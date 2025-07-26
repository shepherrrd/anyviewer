use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub id: String,
    pub message_type: MessageType,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    // Authentication
    AuthRequest,
    AuthResponse,
    
    // Screen capture
    ScreenFrameRequest,
    ScreenFrame,
    ScreenInfo,
    
    // Input events
    InputEvent,
    InputAck,
    
    // Control messages
    Heartbeat,
    ConnectionStatus,
    Error,
    
    // File transfer (future feature)
    FileTransferRequest,
    FileTransferData,
    FileTransferComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenFrame {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,
    pub is_keyframe: bool,
    pub changed_regions: Option<Vec<Region>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Raw,
    H264,
    H265,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    pub event_type: InputEventType,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub button: Option<MouseButton>,
    pub key: Option<String>,
    pub modifiers: Option<Vec<KeyModifier>>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputEventType {
    MouseMove,
    MouseClick,
    MouseRelease,
    MouseScroll,
    KeyPress,
    KeyRelease,
    KeyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyModifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
    Super,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub error: Option<String>,
    pub session_token: Option<String>,
    pub server_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub authenticated: bool,
    pub session_id: String,
    pub connection_quality: ConnectionQuality,
    pub latency_ms: Option<u32>,
    pub bandwidth_kbps: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: u32,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

// Protocol constants
pub const PROTOCOL_VERSION: &str = "1.0.0";
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 30;
pub const CONNECTION_TIMEOUT_SECONDS: u64 = 60;

// Error codes
pub const ERROR_AUTHENTICATION_FAILED: u32 = 1001;
pub const ERROR_UNAUTHORIZED: u32 = 1002;
pub const ERROR_INVALID_MESSAGE: u32 = 2001;
pub const ERROR_SCREEN_CAPTURE_FAILED: u32 = 3001;
pub const ERROR_INPUT_INJECTION_FAILED: u32 = 3002;
pub const ERROR_NETWORK_ERROR: u32 = 4001;
pub const ERROR_INTERNAL_ERROR: u32 = 5001;

impl ProtocolMessage {
    pub fn new(message_type: MessageType, data: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            message_type,
            data,
            timestamp: Utc::now(),
        }
    }
    
    pub fn auth_request(username: Option<String>, password: Option<String>, token: Option<String>) -> Self {
        let auth_request = AuthRequest {
            username,
            password,
            token,
            client_info: ClientInfo {
                name: "AnyViewer".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                platform: std::env::consts::OS.to_string(),
                capabilities: vec![
                    "screen_capture".to_string(),
                    "input_forwarding".to_string(),
                    "file_transfer".to_string(),
                ],
            },
        };
        
        Self::new(MessageType::AuthRequest, serde_json::to_value(auth_request).unwrap())
    }
    
    pub fn auth_response(success: bool, error: Option<String>, session_token: Option<String>) -> Self {
        let auth_response = AuthResponse {
            success,
            error,
            session_token,
            server_capabilities: vec![
                "screen_capture".to_string(),
                "input_forwarding".to_string(),
                "file_transfer".to_string(),
            ],
        };
        
        Self::new(MessageType::AuthResponse, serde_json::to_value(auth_response).unwrap())
    }
    
    pub fn screen_frame(frame: ScreenFrame) -> Self {
        Self::new(MessageType::ScreenFrame, serde_json::to_value(frame).unwrap())
    }
    
    pub fn input_event(event: InputEvent) -> Self {
        Self::new(MessageType::InputEvent, serde_json::to_value(event).unwrap())
    }
    
    pub fn error(code: u32, message: String, details: Option<serde_json::Value>) -> Self {
        let error_msg = ErrorMessage {
            code,
            message,
            details,
        };
        
        Self::new(MessageType::Error, serde_json::to_value(error_msg).unwrap())
    }
    
    pub fn heartbeat() -> Self {
        Self::new(MessageType::Heartbeat, serde_json::json!({
            "timestamp": Utc::now(),
            "version": PROTOCOL_VERSION
        }))
    }
}

impl InputEvent {
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self {
            event_type: InputEventType::MouseMove,
            x: Some(x),
            y: Some(y),
            button: None,
            key: None,
            modifiers: None,
            timestamp: Utc::now(),
        }
    }
    
    pub fn mouse_click(x: i32, y: i32, button: MouseButton) -> Self {
        Self {
            event_type: InputEventType::MouseClick,
            x: Some(x),
            y: Some(y),
            button: Some(button),
            key: None,
            modifiers: None,
            timestamp: Utc::now(),
        }
    }
    
    pub fn key_press(key: String, modifiers: Option<Vec<KeyModifier>>) -> Self {
        Self {
            event_type: InputEventType::KeyPress,
            x: None,
            y: None,
            button: None,
            key: Some(key),
            modifiers,
            timestamp: Utc::now(),
        }
    }
    
    pub fn key_type(text: String) -> Self {
        Self {
            event_type: InputEventType::KeyType,
            x: None,
            y: None,
            button: None,
            key: Some(text),
            modifiers: None,
            timestamp: Utc::now(),
        }
    }
}