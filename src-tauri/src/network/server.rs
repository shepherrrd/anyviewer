use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use log::{info, error, debug, warn};
use serde_json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

use super::protocol::{ProtocolMessage, MessageType, InputEvent};

type ClientId = String;
type WebSocket = WebSocketStream<TcpStream>;

pub struct RemoteDesktopServer {
    port: u16,
    clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
    message_tx: Option<mpsc::UnboundedSender<ServerMessage>>,
}

#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub id: ClientId,
    pub address: SocketAddr,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub authenticated: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ServerMessage {
    ClientConnected(ClientId, SocketAddr),
    ClientDisconnected(ClientId),
    ScreenFrameRequest(ClientId),
    InputEvent(ClientId, InputEvent),
    BroadcastFrame(Vec<u8>),
}

impl RemoteDesktopServer {
    pub async fn new(port: u16) -> Result<Self> {
        Ok(Self {
            port,
            clients: Arc::new(RwLock::new(HashMap::new())),
            message_tx: None,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Remote desktop server listening on {}", addr);
        
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<ServerMessage>();
        
        // Spawn message handler
        let clients_clone = self.clients.clone();
        tokio::spawn(async move {
            while let Some(message) = message_rx.recv().await {
                Self::handle_server_message(message, &clients_clone).await;
            }
        });
        
        // Accept connections
        while let Ok((stream, addr)) = listener.accept().await {
            info!("New connection from {}", addr);
            
            let clients = self.clients.clone();
            let message_tx = message_tx.clone();
            
            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, addr, clients, message_tx).await {
                    error!("Connection error for {}: {}", addr, e);
                }
            });
        }
        
        Ok(())
    }
    
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
        message_tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<()> {
        let ws_stream = accept_async(stream).await?;
        let client_id = Uuid::new_v4().to_string();
        
        debug!("WebSocket connection established for client {}", client_id);
        
        // Register client
        let client_connection = ClientConnection {
            id: client_id.clone(),
            address: addr,
            connected_at: chrono::Utc::now(),
            authenticated: false,
            capabilities: vec![],
        };
        
        clients.write().await.insert(client_id.clone(), client_connection);
        
        // Notify about new connection
        let _ = message_tx.send(ServerMessage::ClientConnected(client_id.clone(), addr));
        
        // Handle WebSocket messages
        let result = Self::handle_websocket(ws_stream, client_id.clone(), message_tx.clone()).await;
        
        // Cleanup on disconnect
        clients.write().await.remove(&client_id);
        let _ = message_tx.send(ServerMessage::ClientDisconnected(client_id));
        
        result
    }
    
    async fn handle_websocket(
        mut ws_stream: WebSocket,
        client_id: ClientId,
        message_tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<()> {
        while let Some(msg) = ws_stream.next().await {
            match msg? {
                Message::Text(text) => {
                    debug!("Received text message from {}: {}", client_id, text);
                    
                    if let Ok(protocol_msg) = serde_json::from_str::<ProtocolMessage>(&text) {
                        Self::handle_protocol_message(protocol_msg, &client_id, &message_tx, &mut ws_stream).await?;
                    } else {
                        warn!("Invalid protocol message from {}: {}", client_id, text);
                    }
                }
                Message::Binary(data) => {
                    debug!("Received binary message from {} ({} bytes)", client_id, data.len());
                    // Handle binary data (e.g., compressed screen frames)
                }
                Message::Ping(payload) => {
                    debug!("Received ping from {}", client_id);
                    ws_stream.send(Message::Pong(payload)).await?;
                }
                Message::Pong(_) => {
                    debug!("Received pong from {}", client_id);
                }
                Message::Close(_) => {
                    info!("Client {} closed connection", client_id);
                    break;
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    async fn handle_protocol_message(
        message: ProtocolMessage,
        client_id: &str,
        message_tx: &mpsc::UnboundedSender<ServerMessage>,
        ws_stream: &mut WebSocket,
    ) -> Result<()> {
        match message.message_type {
            MessageType::AuthRequest => {
                debug!("Auth request from client {}", client_id);
                
                // Simple authentication (in real app, use proper auth)
                let auth_response = ProtocolMessage {
                    id: Uuid::new_v4().to_string(),
                    message_type: MessageType::AuthResponse,
                    data: serde_json::json!({
                        "success": true,
                        "capabilities": ["screen_capture", "input_forwarding"]
                    }),
                    timestamp: chrono::Utc::now(),
                };
                
                let response_text = serde_json::to_string(&auth_response)?;
                ws_stream.send(Message::Text(response_text)).await?;
            }
            MessageType::ScreenFrameRequest => {
                debug!("Screen frame request from client {}", client_id);
                let _ = message_tx.send(ServerMessage::ScreenFrameRequest(client_id.to_string()));
            }
            MessageType::InputEvent => {
                if let Ok(input_event) = serde_json::from_value::<InputEvent>(message.data) {
                    debug!("Input event from client {}: {:?}", client_id, input_event);
                    let _ = message_tx.send(ServerMessage::InputEvent(client_id.to_string(), input_event));
                }
            }
            MessageType::Heartbeat => {
                debug!("Heartbeat from client {}", client_id);
                
                let heartbeat_response = ProtocolMessage {
                    id: Uuid::new_v4().to_string(),
                    message_type: MessageType::Heartbeat,
                    data: serde_json::json!({"status": "ok"}),
                    timestamp: chrono::Utc::now(),
                };
                
                let response_text = serde_json::to_string(&heartbeat_response)?;
                ws_stream.send(Message::Text(response_text)).await?;
            }
            _ => {
                debug!("Unhandled message type from client {}: {:?}", client_id, message.message_type);
            }
        }
        
        Ok(())
    }
    
    async fn handle_server_message(
        message: ServerMessage,
        clients: &Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
    ) {
        match message {
            ServerMessage::ClientConnected(client_id, addr) => {
                info!("Client {} connected from {}", client_id, addr);
            }
            ServerMessage::ClientDisconnected(client_id) => {
                info!("Client {} disconnected", client_id);
            }
            ServerMessage::ScreenFrameRequest(client_id) => {
                debug!("Processing screen frame request from {}", client_id);
                // In a real implementation, this would trigger screen capture
                // and send the frame back to the client
            }
            ServerMessage::InputEvent(client_id, input_event) => {
                debug!("Processing input event from {}: {:?}", client_id, input_event);
                // In a real implementation, this would inject the input into the system
            }
            ServerMessage::BroadcastFrame(frame_data) => {
                debug!("Broadcasting frame to all clients ({} bytes)", frame_data.len());
                // Broadcast to all connected clients
                let clients_read = clients.read().await;
                for client in clients_read.values() {
                    debug!("Sending frame to client {}", client.id);
                    // In a real implementation, send frame to each client's WebSocket
                }
            }
        }
    }
    
    pub async fn broadcast_screen_frame(&self, frame_data: Vec<u8>) -> Result<()> {
        if let Some(ref tx) = self.message_tx {
            let _ = tx.send(ServerMessage::BroadcastFrame(frame_data));
        }
        Ok(())
    }
    
    pub async fn get_connected_clients(&self) -> Vec<ClientConnection> {
        self.clients.read().await.values().cloned().collect()
    }
}