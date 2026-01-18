//! # MCP Transports
//!
//! Transport layer implementations for MCP communication.

use crate::types::{JsonRpcRequest, JsonRpcResponse, Result};
use crate::McpProtocolError;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{debug, error};

/// Transport trait for MCP communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a request and wait for response
    async fn request(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse>;

    /// Send a notification (no response expected)
    async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()>;

    /// Close the transport
    async fn close(&self) -> Result<()>;

    /// Check if transport is connected
    fn is_connected(&self) -> bool;
}

/// Stdio transport for local MCP servers
pub struct StdioTransport {
    child: tokio::sync::Mutex<Option<Child>>,
    stdin_tx: mpsc::Sender<String>,
    response_rx: tokio::sync::Mutex<mpsc::Receiver<JsonRpcResponse>>,
    connected: std::sync::atomic::AtomicBool,
}

impl StdioTransport {
    /// Create a new stdio transport by spawning a process
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
        working_dir: Option<&str>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(env);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| {
            McpProtocolError::ProcessError(format!("Failed to spawn {}: {}", command, e))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpProtocolError::ProcessError("Failed to get stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpProtocolError::ProcessError("Failed to get stdout".to_string())
        })?;

        // Channel for sending to stdin
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);

        // Channel for receiving responses
        let (response_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(32);

        // Task to write to stdin
        let mut stdin_writer = stdin;
        tokio::spawn(async move {
            while let Some(msg) = stdin_rx.recv().await {
                if let Err(e) = stdin_writer.write_all(msg.as_bytes()).await {
                    error!("Failed to write to stdin: {}", e);
                    break;
                }
                if let Err(e) = stdin_writer.write_all(b"\n").await {
                    error!("Failed to write newline: {}", e);
                    break;
                }
                if let Err(e) = stdin_writer.flush().await {
                    error!("Failed to flush stdin: {}", e);
                    break;
                }
            }
        });

        // Task to read from stdout
        let mut reader = BufReader::new(stdout).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        if response_tx.send(response).await.is_err() {
                            break;
                        }
                    }
                    Err(_e) => {
                        debug!("Non-JSON line from server: {}", line);
                    }
                }
            }
        });

        Ok(Self {
            child: tokio::sync::Mutex::new(Some(child)),
            stdin_tx,
            response_rx: tokio::sync::Mutex::new(response_rx),
            connected: std::sync::atomic::AtomicBool::new(true),
        })
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn request(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let json = serde_json::to_string(&req)?;
        debug!("Sending request: {}", json);

        self.stdin_tx
            .send(json)
            .await
            .map_err(|e| McpProtocolError::TransportError(format!("Send failed: {}", e)))?;

        // Wait for response with timeout
        let mut rx = self.response_rx.lock().await;
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
            Ok(Some(response)) => Ok(response),
            Ok(None) => Err(McpProtocolError::TransportError(
                "Connection closed".to_string(),
            )),
            Err(_) => Err(McpProtocolError::Timeout("Request timeout".to_string())),
        }
    }

    async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = crate::types::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        let json = serde_json::to_string(&notification)?;

        self.stdin_tx
            .send(json)
            .await
            .map_err(|e| McpProtocolError::TransportError(format!("Send failed: {}", e)))?;

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.connected
            .store(false, std::sync::atomic::Ordering::SeqCst);

        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// SSE (Server-Sent Events) transport for remote MCP servers
pub struct SseTransport {
    url: String,
    client: reqwest::Client,
    connected: std::sync::atomic::AtomicBool,
}

impl SseTransport {
    /// Create a new SSE transport
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
            connected: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Create with authentication header
    pub fn with_auth(url: impl Into<String>, auth_header: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    auth_header.into().parse().unwrap(),
                );
                headers
            })
            .build()
            .unwrap();

        Self {
            url: url.into(),
            client,
            connected: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

#[async_trait]
impl Transport for SseTransport {
    async fn request(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let response = self
            .client
            .post(&self.url)
            .json(&req)
            .send()
            .await
            .map_err(|e| McpProtocolError::TransportError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(McpProtocolError::TransportError(format!(
                "HTTP {} from server",
                response.status()
            )));
        }

        let json_response: JsonRpcResponse = response.json().await.map_err(|e| {
            McpProtocolError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        Ok(json_response)
    }

    async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = crate::types::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let _ = self
            .client
            .post(&self.url)
            .json(&notification)
            .send()
            .await
            .map_err(|e| McpProtocolError::TransportError(format!("HTTP error: {}", e)))?;

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.connected
            .store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// HTTP transport for stateless MCP servers
pub struct HttpTransport {
    url: String,
    client: reqwest::Client,
}

impl HttpTransport {
    /// Create a new HTTP transport
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn request(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let response = self
            .client
            .post(&self.url)
            .json(&req)
            .send()
            .await
            .map_err(|e| McpProtocolError::TransportError(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(McpProtocolError::TransportError(format!(
                "HTTP {} from server",
                response.status()
            )));
        }

        let json_response: JsonRpcResponse = response.json().await.map_err(|e| {
            McpProtocolError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        Ok(json_response)
    }

    async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = crate::types::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let _ = self
            .client
            .post(&self.url)
            .json(&notification)
            .send()
            .await
            .map_err(|e| McpProtocolError::TransportError(format!("HTTP error: {}", e)))?;

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true // HTTP is stateless
    }
}
