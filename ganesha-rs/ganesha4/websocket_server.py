#!/usr/bin/env python3

"""
A simple WebSocket echo server using websockets library.
"""

import asyncio
import logging
import websockets

# Configure basic logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

async def echo(websocket, path):
    """
    Handler for each connected client.
    Echoes back any message received.
    """
    logger.info(f"Client connected: {websocket.remote_address}")
    try:
        async for message in websocket:
            logger.debug(f"Received message from {websocket.remote_address}: {message}")
            await websocket.send(message)
    except websockets.exceptions.ConnectionClosed as e:
        logger.warning(f"Connection closed: {e.code} - {e.reason}")
    finally:
        logger.info(f"Client disconnected: {websocket.remote_address}")

async def main():
    """
    Starts the WebSocket server on localhost:8765.
    """
    async with websockets.serve(echo, "localhost", 8765):
        logger.info("WebSocket server started at ws://localhost:8765")
        await asyncio.Future()  # run forever

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
