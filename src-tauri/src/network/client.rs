use anyhow::Result;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use log::{info, error, debug, warn};
use serde_json;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use url::Url;
use uuid::Uuid;

use super::protocol::{ProtocolMessage, MessageType, ScreenFrame, InputEvent};

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub server_url: String,
    pub auth_token: Option<String>,
    pub auto_reconnect: bool,
    pub heartbeat_interval: u64, // seconds
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "ws://127.0.0.1:7878".to_string(),
            auth_token: None,
            auto_reconnect: true,
            heartbeat_interval: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected,
    Disconnected,
    AuthenticationSuccess,
    AuthenticationFailed(String),
    ScreenFrameReceived(Vec<u8>),
    InputEventSent,
    Error(String),
}

pub struct RemoteDesktopClient {
    config: Arc<RwLock<ClientConfig>>,
    ws_stream: Option<WebSocket>,
    event_tx: Option<mpsc::UnboundedSender<ClientEvent>>,
    is_connected: Arc<RwLock<bool>>,
    is_authenticated: Arc<RwLock<bool>>,
}

impl RemoteDesktopClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            ws_stream: None,
            event_tx: None,
            is_connected: Arc::new(RwLock::new(false)),
            is_authenticated: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn connect(&mut self) -> Result<mpsc::UnboundedReceiver<ClientEvent>> {
        let config = self.config.read().await;
        let url = Url::parse(&config.server_url)?;
        drop(config);
        
        info!("Connecting to remote desktop server: {}", url);
        
        let (ws_stream, _) = connect_async(url).await?;
        self.ws_stream = Some(ws_stream);
        
        let (event_tx, event_rx) = mpsc::unbounded_channel::<ClientEvent>();
        self.event_tx = Some(event_tx.clone());
        
        // Set connected status
        *self.is_connected.write().await = true;
        
        // Notify connection
        let _ = event_tx.send(ClientEvent::Connected);
        
        // Start message handling
        if let Some(ws_stream) = self.ws_stream.take() {
            let is_connected = self.is_connected.clone();
            let is_authenticated = self.is_authenticated.clone();
            let config = self.config.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_messages(
                    ws_stream,
                    event_tx,
                    is_connected,
                    is_authenticated,
                    config,
                ).await {
                    error!("Message handling error: {}", e);
                }
            });
        }
        
        // Start authentication
        self.authenticate().await?;
        
        Ok(event_rx)
    }
    
    async fn handle_messages(
        mut ws_stream: WebSocket,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
        is_connected: Arc<RwLock<bool>>,
        is_authenticated: Arc<RwLock<bool>>,
        config: Arc<RwLock<ClientConfig>>,
    ) -> Result<()> {
        // Split the WebSocket stream for concurrent read/write
        let (mut ws_sink, mut ws_stream_read) = ws_stream.split();
        
        // Create channel for sending messages from reader to writer
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Message>();
        
        // Start heartbeat and message writer with the sink
        let heartbeat_tx = event_tx.clone();
        let heartbeat_config = config.clone();
        tokio::spawn(async move {
            let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(30)); // Default interval
            
            loop {
                tokio::select! {
                    // Handle heartbeat
                    _ = heartbeat_interval.tick() => {
                        // Update interval if config changed
                        let new_interval = {
                            let config = heartbeat_config.read().await;
                            config.heartbeat_interval
                        };
                        
                        let heartbeat_msg = ProtocolMessage {
                            id: Uuid::new_v4().to_string(),
                            message_type: MessageType::Heartbeat,
                            data: serde_json::json!({"timestamp": chrono::Utc::now()}),
                            timestamp: chrono::Utc::now(),
                        };
                        
                        if let Ok(msg_text) = serde_json::to_string(&heartbeat_msg) {
                            if ws_sink.send(Message::Text(msg_text)).await.is_err() {
                                let _ = heartbeat_tx.send(ClientEvent::Error("Heartbeat failed".to_string()));
                                break;
                            }
                        }
                    },
                    // Handle messages from reader
                    Some(msg) = write_rx.recv() => {
                        if ws_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                    else => break,
                }
            }
        });
        
        while let Some(msg) = ws_stream_read.next().await {
            match msg? {
                Message::Text(text) => {
                    debug!("Received text message: {}", text);
                    
                    if let Ok(protocol_msg) = serde_json::from_str::<ProtocolMessage>(&text) {
                        Self::handle_protocol_message(
                            protocol_msg,
                            &event_tx,
                            &is_authenticated,
                        ).await?;
                    } else {
                        warn!("Invalid protocol message: {}", text);
                    }
                }
                Message::Binary(data) => {
                    debug!("Received binary message ({} bytes)", data.len());
                    let _ = event_tx.send(ClientEvent::ScreenFrameReceived(data));
                }
                Message::Ping(payload) => {
                    debug!("Received ping");
                    let _ = write_tx.send(Message::Pong(payload));
                }
                Message::Pong(_) => {
                    debug!("Received pong");
                }
                Message::Close(_) => {
                    info!("Server closed connection");
                    *is_connected.write().await = false;
                    let _ = event_tx.send(ClientEvent::Disconnected);
                    break;
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    async fn handle_protocol_message(
        message: ProtocolMessage,
        event_tx: &mpsc::UnboundedSender<ClientEvent>,
        is_authenticated: &Arc<RwLock<bool>>,
    ) -> Result<()> {
        match message.message_type {
            MessageType::AuthResponse => {
                debug!("Received auth response");
                
                if let Ok(success) = message.data.get("success").and_then(|v| v.as_bool()).ok_or("Missing success field") {
                    if success {
                        *is_authenticated.write().await = true;
                        let _ = event_tx.send(ClientEvent::AuthenticationSuccess);
                        info!("Authentication successful");
                    } else {
                        let error = message.data.get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");
                        let _ = event_tx.send(ClientEvent::AuthenticationFailed(error.to_string()));
                        error!("Authentication failed: {}", error);
                    }
                }
            }
            MessageType::ScreenFrame => {
                debug!("Received screen frame message");
                
                if let Ok(frame) = serde_json::from_value::<ScreenFrame>(message.data) {
                    let _ = event_tx.send(ClientEvent::ScreenFrameReceived(frame.data));
                }
            }
            MessageType::Heartbeat => {
                debug!("Received heartbeat response");
            }
            _ => {
                debug!("Unhandled message type: {:?}", message.message_type);
            }
        }
        
        Ok(())
    }
    
    async fn heartbeat_loop_with_sink(
        mut ws_sink: SplitSink<WebSocket, Message>,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
        config: Arc<RwLock<ClientConfig>>,
    ) {
        loop {
            let interval = {
                let config = config.read().await;
                config.heartbeat_interval
            };
            
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            
            let heartbeat_msg = ProtocolMessage {
                id: Uuid::new_v4().to_string(),
                message_type: MessageType::Heartbeat,
                data: serde_json::json!({"timestamp": chrono::Utc::now()}),
                timestamp: chrono::Utc::now(),
            };
            
            if let Ok(msg_text) = serde_json::to_string(&heartbeat_msg) {
                if ws_sink.send(Message::Text(msg_text)).await.is_err() {
                    let _ = event_tx.send(ClientEvent::Error("Heartbeat failed".to_string()));
                    break;
                }
            }
        }
    }

    async fn heartbeat_loop(
        mut ws_stream: WebSocket,
        event_tx: mpsc::UnboundedSender<ClientEvent>,
        config: Arc<RwLock<ClientConfig>>,
    ) {
        loop {
            let interval = {
                let config = config.read().await;
                config.heartbeat_interval
            };
            
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            
            let heartbeat_msg = ProtocolMessage {
                id: Uuid::new_v4().to_string(),
                message_type: MessageType::Heartbeat,
                data: serde_json::json!({"timestamp": chrono::Utc::now()}),
                timestamp: chrono::Utc::now(),
            };
            
            if let Ok(msg_text) = serde_json::to_string(&heartbeat_msg) {
                if ws_stream.send(Message::Text(msg_text)).await.is_err() {
                    let _ = event_tx.send(ClientEvent::Error("Heartbeat failed".to_string()));
                    break;
                }
            }
        }
    }
    
    pub async fn authenticate(&self) -> Result<()> {
        if let Some(ref event_tx) = self.event_tx {
            let auth_msg = ProtocolMessage {
                id: Uuid::new_v4().to_string(),
                message_type: MessageType::AuthRequest,
                data: serde_json::json!({
                    "token": self.config.read().await.auth_token
                }),
                timestamp: chrono::Utc::now(),
            };
            
            let msg_text = serde_json::to_string(&auth_msg)?;
            
            // In a real implementation, send this through the WebSocket
            debug!("Sending authentication request");
        }
        
        Ok(())
    }
    
    pub async fn request_screen_frame(&self) -> Result<()> {
        if !*self.is_authenticated.read().await {
            return Err(anyhow::anyhow!("Not authenticated"));
        }
        
        let request_msg = ProtocolMessage {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::ScreenFrameRequest,
            data: serde_json::json!({}),
            timestamp: chrono::Utc::now(),
        };
        
        let msg_text = serde_json::to_string(&request_msg)?;
        
        // In a real implementation, send this through the WebSocket
        debug!("Requesting screen frame");
        
        if let Some(ref event_tx) = self.event_tx {
            let _ = event_tx.send(ClientEvent::InputEventSent);
        }
        
        Ok(())
    }
    
    pub async fn send_input_event(&self, input_event: InputEvent) -> Result<()> {
        if !*self.is_authenticated.read().await {
            return Err(anyhow::anyhow!("Not authenticated"));
        }
        
        let input_msg = ProtocolMessage {
            id: Uuid::new_v4().to_string(),
            message_type: MessageType::InputEvent,
            data: serde_json::to_value(input_event)?,
            timestamp: chrono::Utc::now(),
        };
        
        let msg_text = serde_json::to_string(&input_msg)?;
        
        // In a real implementation, send this through the WebSocket
        debug!("Sending input event");
        
        if let Some(ref event_tx) = self.event_tx {
            let _ = event_tx.send(ClientEvent::InputEventSent);
        }
        
        Ok(())
    }
    
    pub async fn disconnect(&mut self) -> Result<()> {
        *self.is_connected.write().await = false;
        *self.is_authenticated.write().await = false;
        
        if let Some(ref mut ws_stream) = self.ws_stream {
            ws_stream.close(None).await?;
        }
        
        info!("Disconnected from remote desktop server");
        Ok(())
    }
    
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }
    
    pub async fn is_authenticated(&self) -> bool {
        *self.is_authenticated.read().await
    }
}