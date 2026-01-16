use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
    AuthorizationCode, TokenResponse,
};
use keyring::Entry;
use anyhow::{Result, anyhow};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use oauth2::url::Url;

use crate::core::config::{ProviderType, OAuth2Config};

pub struct AuthManager {
    service_name: String,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            service_name: "ganesha-ai".to_string(),
        }
    }

    pub async fn login(&self, provider: ProviderType) -> Result<String> {
        let config = match provider {
            ProviderType::OpenAI => OAuth2Config::openai(),
            ProviderType::Google => OAuth2Config::google(),
            ProviderType::Anthropic => OAuth2Config::anthropic(),
            _ => return Err(anyhow!("OAuth2 not supported for this provider")),
        };

        let client = BasicClient::new(
            ClientId::new(config.client_id),
            config.client_secret.map(ClientSecret::new),
            AuthUrl::new(config.auth_url)?,
            Some(TokenUrl::new(config.token_url)?),
        )
        .set_redirect_uri(RedirectUrl::new(config.redirect_uri.clone())?);

        // Generate authorization URL
        let (auth_url, _csrf_token) = client
            .authorize_url(oauth2::CsrfToken::new_random)
            .add_scopes(config.scopes.into_iter().map(oauth2::Scope::new))
            .url();

        println!("Open this URL in your browser:\n\n{}\n", auth_url);

        // Start local server to receive callback
        let code = self.wait_for_callback(&config.redirect_uri)?;

        // Exchange code for token
        let token_result = client
            .exchange_code(AuthorizationCode::new(code))
            .request_async(oauth2::reqwest::async_http_client)
            .await?;

        let access_token = token_result.access_token().secret().to_string();

        // Store token in keyring
        self.store_token(&provider.to_string(), &access_token)?;

        Ok(access_token)
    }

    fn wait_for_callback(&self, redirect_uri: &str) -> Result<String> {
        let url = Url::parse(redirect_uri)?;
        let host = url.host_str().unwrap_or("127.0.0.1");
        let port = url.port().unwrap_or(8420);

        let listener = TcpListener::bind(format!("{}:{}", host, port))?;
        let mut socket = listener.accept()?.0;
        let mut reader = BufReader::new(&mut socket);

        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;

        // Parse code from request
        let code = request_line
            .split_whitespace()
            .nth(1)
            .and_then(|path| Url::parse(&format!("http://localhost{}", path)).ok())
            .and_then(|u: Url| u.query_pairs().find(|(k, _): &(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)| k == "code").map(|(_, v)| v.into_owned()))
            .ok_or_else(|| anyhow!("No code found in callback"))?;

        // Send response
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Login Successful!</h1><p>You can close this window and return to the terminal.</p></body></html>";
        socket.write_all(response.as_bytes())?;

        Ok(code)
    }

    pub fn store_token(&self, provider: &str, token: &str) -> Result<()> {
        let entry = Entry::new(&self.service_name, provider)?;
        entry.set_password(token)?;
        Ok(())
    }

    pub fn get_token(&self, provider: &str) -> Result<String> {
        let entry = Entry::new(&self.service_name, provider)?;
        entry.get_password().map_err(|e| anyhow!("Failed to get token: {}", e))
    }

    pub fn delete_token(&self, provider: &str) -> Result<()> {
        let entry = Entry::new(&self.service_name, provider)?;
        entry.delete_password().map_err(|e| anyhow!("Failed to delete token: {}", e))
    }
}
