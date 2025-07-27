
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::{info, error, debug, warn};
use tauri::{CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem};

// Import modules
mod capture;
mod network;
mod codec;
mod input;
mod security;
mod config;
mod utils;
mod streaming;
mod permissions;
mod metrics;
mod testing;

use capture::ScreenCaptureManager;
use network::{NetworkManager, ConnectionRequest as NetworkConnectionRequest, ConnectionResponse, DiscoveredDevice, IncomingConnectionRequest};
use network::connection_manager::{ConnectionManager, ConnectionConfig, ConnectionStatus, ConnectionType};
use input::InputManager;
use security::SecurityManager;
use config::AppConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

// Global network manager state
static GLOBAL_NETWORK_MANAGER: tokio::sync::OnceCell<Arc<Mutex<NetworkManager>>> = tokio::sync::OnceCell::const_new();

async fn get_global_network_manager() -> Arc<Mutex<NetworkManager>> {
    GLOBAL_NETWORK_MANAGER.get_or_init(|| async {
        Arc::new(Mutex::new(NetworkManager::new()))
    }).await.clone()
}

use streaming::{StreamingManager, StreamingConfig, StreamingStats};
use permissions::{PermissionManager, PermissionConfig, Permission, PermissionResponse, DeviceInfo as PermissionDeviceInfo};
use metrics::{MetricsCollector, ConnectionMetrics, SystemMetrics, QualityMetrics, AlertThresholds};
use testing::{PerformanceTester, PerformanceTestConfig, PerformanceTestResult};

// Tauri commands
#[tauri::command]
async fn start_host_session() -> Result<String, String> {
    info!("Starting host session");
    
    // Initialize screen capture
    let capture_manager = ScreenCaptureManager::new().map_err(|e| e.to_string())?;
    
    // Generate unique session ID
    let session_id = uuid::Uuid::new_v4().to_string();
    
    // Start network server
    let network_manager = NetworkManager::new();
    network_manager.start_host_server().await.map_err(|e| e.to_string())?;
    
    info!("Host session started with ID: {}", session_id);
    Ok(session_id)
}

#[tauri::command]
async fn connect_to_session(session_id: String) -> Result<ConnectionResponse, String> {
    info!("Connecting to session: {}", session_id);
    
    let network_manager = NetworkManager::new();
    let request = NetworkConnectionRequest { session_id: session_id.clone() };
    
    let response = network_manager.connect_to_host(request).await.map_err(|e| e.to_string())?;
    
    info!("Successfully connected to session: {}", session_id);
    Ok(response)
}

#[tauri::command]
async fn capture_screen() -> Result<Vec<u8>, String> {
    debug!("Capturing screen");
    
    let capture_manager = ScreenCaptureManager::new().map_err(|e| e.to_string())?;
    let screen_data = capture_manager.capture_primary_screen().await.map_err(|e| e.to_string())?;
    
    Ok(screen_data)
}

#[tauri::command]
async fn send_input_event(x: i32, y: i32, event_type: String, data: String) -> Result<(), String> {
    debug!("Sending input event: {} at ({}, {})", event_type, x, y);
    
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        rt.block_on(async {
            let mut input_manager = InputManager::new();
            input_manager.send_input(x, y, event_type, data).await.map_err(|e| e.to_string())
        })
    }).await.map_err(|e| e.to_string())??;
    
    Ok(())
}

#[tauri::command]
async fn get_system_info() -> Result<serde_json::Value, String> {
    debug!("Getting system information");
    
    let system_info = utils::system::get_system_info().await.map_err(|e| e.to_string())?;
    Ok(system_info)
}

#[tauri::command]
async fn generate_session_id() -> Result<String, String> {
    debug!("Generating new session ID");
    
    let id_generator = utils::id_generator::IdGenerator::new();
    let connection_id = id_generator.generate_connection_id().map_err(|e| e.to_string())?;
    
    info!("Generated session ID: {}", connection_id.formatted_id);
    Ok(connection_id.formatted_id)
}

#[tauri::command]
async fn initialize_security() -> Result<String, String> {
    info!("Initializing security subsystem");
    
    let security_manager = SecurityManager::new().map_err(|e| e.to_string())?;
    let public_key = security_manager.get_public_key().map_err(|e| e.to_string())?;
    
    Ok(public_key)
}

// Connection request commands
#[tauri::command]
async fn initialize_connection_requests() -> Result<(), String> {
    info!("Initializing connection request manager");
    
    let manager = get_global_network_manager().await;
    let mut network_manager = manager.lock().await;
    let (_request_rx, _response_rx) = network_manager.initialize_connection_requests().await.map_err(|e| e.to_string())?;
    
    info!("Connection request manager initialized");
    Ok(())
}

#[tauri::command]
async fn create_connection_request(
    requester_device_id: String,
    requester_name: String,
    requester_ip: String,
    requested_permissions: Vec<String>,
    message: Option<String>,
) -> Result<String, String> {
    info!("Creating connection request from {}", requester_name);
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    let request_id = network_manager
        .create_connection_request(
            requester_device_id,
            requester_name,
            requester_ip,
            requested_permissions,
            message,
        )
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(request_id)
}

#[tauri::command]
async fn respond_to_connection_request(
    request_id: String,
    accepted: bool,
    granted_permissions: Vec<String>,
    session_duration_minutes: Option<u32>,
    denial_reason: Option<String>,
) -> Result<(), String> {
    info!("Responding to connection request {}: accepted={}", request_id, accepted);
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    network_manager
        .respond_to_connection_request(
            request_id,
            accepted,
            granted_permissions,
            session_duration_minutes,
            denial_reason,
        )
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn get_pending_connection_requests() -> Result<Vec<IncomingConnectionRequest>, String> {
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    let requests = network_manager.get_pending_connection_requests().await;
    
    Ok(requests)
}

#[tauri::command]
async fn get_accepted_connection_requests() -> Result<Vec<serde_json::Value>, String> {
    // This would return requests that have been accepted and need screen sharing to start
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    
    // For now return empty array - this would be implemented to track accepted requests
    // that are waiting for screen sharing to begin
    let accepted_requests: Vec<serde_json::Value> = vec![];
    
    Ok(accepted_requests)
}

#[tauri::command]
async fn start_screen_sharing_for_request(request_id: String) -> Result<(), String> {
    info!("Starting screen sharing for request: {}", request_id);
    
    // Initialize screen capture and streaming
    let capture_manager = ScreenCaptureManager::new().map_err(|e| e.to_string())?;
    
    // Start streaming manager
    let streaming_manager = StreamingManager::new();
    let _event_receiver = streaming_manager.initialize().await.map_err(|e| e.to_string())?;
    
    streaming_manager.start_streaming().await.map_err(|e| e.to_string())?;
    
    info!("Screen sharing started for request: {}", request_id);
    Ok(())
}

#[tauri::command]
async fn cancel_connection_request(request_id: String) -> Result<(), String> {
    info!("Cancelling connection request: {}", request_id);
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    network_manager.cancel_connection_request(&request_id).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn test_create_connection_request() -> Result<String, String> {
    info!("Creating test connection request");
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    
    let request_id = network_manager
        .create_connection_request(
            "test-device-123".to_string(),
            "Test Windows PC".to_string(),
            "192.168.1.100".to_string(),
            vec!["screen_capture".to_string(), "input_forwarding".to_string()],
            Some("Test connection request from Windows PC".to_string()),
        )
        .await
        .map_err(|e| e.to_string())?;
    
    info!("Test connection request created with ID: {}", request_id);
    Ok(request_id)
}

#[tauri::command]
async fn test_udp_broadcast() -> Result<String, String> {
    use std::net::UdpSocket;
    
    info!("Testing UDP broadcast functionality");
    
    // Instead of trying to bind to 7879 (which discovery service already uses),
    // let's test broadcasting capability using ephemeral port
    
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(test_socket) => {
            if let Err(e) = test_socket.set_broadcast(true) {
                return Ok(format!("❌ Failed to enable broadcast: {}", e));
            }
            
            // Create a test discovery message
            let test_message = "TEST_BROADCAST_FROM_ANYVIEWER";
            let broadcast_addr = "255.255.255.255:7879";
            
            match test_socket.send_to(test_message.as_bytes(), broadcast_addr) {
                Ok(_) => {
                    let result = format!("✅ UDP Test successful!\n📡 Test broadcast sent successfully to {}\n🔍 Discovery service should be running on port 7879", broadcast_addr);
                    info!("{}", result);
                    Ok(result)
                }
                Err(e) => {
                    let result = format!("⚠️  Failed to send test broadcast: {}", e);
                    warn!("{}", result);
                    Ok(result)
                }
            }
        }
        Err(e) => {
            Ok(format!("❌ Failed to create test socket: {}", e))
        }
    }
}

// Network discovery commands
#[tauri::command]
async fn start_network_discovery(device_name: String) -> Result<(), String> {
    info!("Starting network discovery with device name: {}", device_name);
    
    let manager = get_global_network_manager().await;
    let mut network_manager = manager.lock().await;
    let _device_updates_rx = network_manager.start_discovery(device_name).await.map_err(|e| e.to_string())?;
    
    info!("Network discovery started");
    Ok(())
}

#[tauri::command]
async fn stop_network_discovery() -> Result<(), String> {
    info!("Stopping network discovery");
    
    let manager = get_global_network_manager().await;
    let mut network_manager = manager.lock().await;
    network_manager.stop_discovery().await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn get_discovered_devices() -> Result<Vec<DiscoveredDevice>, String> {
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    let devices = network_manager.get_discovered_devices().await;
    
    Ok(devices)
}

#[tauri::command]
async fn connect_to_discovered_device(device_id: String) -> Result<ConnectionResponse, String> {
    info!("Connecting to discovered device: {}", device_id);
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    let response = network_manager.connect_to_discovered_device(&device_id).await.map_err(|e| e.to_string())?;
    
    info!("Successfully connected to discovered device: {}", device_id);
    Ok(response)
}

#[tauri::command]
async fn connect_to_ip(ip_address: String, port: Option<String>) -> Result<ConnectionResponse, String> {
    info!("Connecting to IP address: {}:{}", ip_address, port.as_deref().unwrap_or("7878"));
    
    let actual_port = port.unwrap_or_else(|| "7878".to_string());
    let full_address = if ip_address.contains(':') {
        ip_address
    } else {
        format!("{}:{}", ip_address, actual_port)
    };

    // For IP connections, we'll create a direct connection request
    let session_id = uuid::Uuid::new_v4().to_string();
    let request = NetworkConnectionRequest { session_id: session_id.clone() };
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    
    // In a real implementation, this would establish a direct TCP connection
    // For now, we'll simulate the response
    let response = ConnectionResponse {
        success: true,
        session_id,
        server_info: Some(network::ServerInfo {
            version: "1.0.0".to_string(),
            capabilities: vec![
                "screen_capture".to_string(),
                "input_forwarding".to_string(),
                "file_transfer".to_string(),
            ],
            encryption_enabled: true,
        }),
        error: None,
    };

    info!("Successfully connected to IP: {}", full_address);
    Ok(response)
}

#[tauri::command]
async fn send_connection_request_to_device(
    device_id: String,
    requester_name: String,
    requester_ip: String,
    requested_permissions: Vec<String>,
    message: Option<String>,
) -> Result<String, String> {
    info!("Sending connection request to device: {}", device_id);
    
    let manager = get_global_network_manager().await;
    let network_manager = manager.lock().await;
    
    let request_id = network_manager
        .send_connection_request_to_device(
            &device_id,
            requester_name,
            requester_ip,
            requested_permissions,
            message,
        )
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(request_id)
}

// New connection manager commands
#[tauri::command]
async fn initialize_connection_manager() -> Result<String, String> {
    info!("Initializing connection manager");
    
    let connection_manager = ConnectionManager::new();
    let _event_receiver = connection_manager.initialize().await.map_err(|e| e.to_string())?;
    
    // Get the generated connection ID
    let connection_id = connection_manager.get_connection_id().await
        .ok_or_else(|| "Failed to generate connection ID".to_string())?;
    
    info!("Connection manager initialized with ID: {}", connection_id);
    Ok(connection_id)
}

#[tauri::command]
async fn start_hosting_with_fallback() -> Result<String, String> {
    info!("Starting hosting with P2P/Relay fallback");
    
    let connection_manager = ConnectionManager::new();
    let _event_receiver = connection_manager.initialize().await.map_err(|e| e.to_string())?;
    
    let connection_id = connection_manager.start_hosting().await.map_err(|e| e.to_string())?;
    
    info!("Hosting started with connection ID: {}", connection_id);
    Ok(connection_id)
}

#[tauri::command]
async fn connect_to_host_with_fallback(target_id: String) -> Result<(), String> {
    info!("Connecting to host with P2P/Relay fallback: {}", target_id);
    
    let connection_manager = ConnectionManager::new();
    let _event_receiver = connection_manager.initialize().await.map_err(|e| e.to_string())?;
    
    connection_manager.connect_to_host(target_id.clone()).await.map_err(|e| e.to_string())?;
    
    info!("Successfully connected to host: {}", target_id);
    Ok(())
}

#[tauri::command]
async fn get_connection_status() -> Result<String, String> {
    let connection_manager = ConnectionManager::new();
    let status = connection_manager.get_connection_status().await;
    
    let status_str = match status {
        ConnectionStatus::Disconnected => "disconnected".to_string(),
        ConnectionStatus::Connecting => "connecting".to_string(),
        ConnectionStatus::Connected(conn_type) => match conn_type {
            network::connection_manager::ConnectionType::P2P => "connected_p2p".to_string(),
            network::connection_manager::ConnectionType::Relay => "connected_relay".to_string(),
        },
        ConnectionStatus::Failed(error) => format!("failed:{}", error),
    };
    
    Ok(status_str)
}

#[tauri::command]
async fn get_available_peers() -> Result<Vec<String>, String> {
    let connection_manager = ConnectionManager::new();
    let peers = connection_manager.get_available_peers().await;
    
    Ok(peers)
}

#[tauri::command]
async fn disconnect_connection() -> Result<(), String> {
    info!("Disconnecting current connection");
    
    let connection_manager = ConnectionManager::new();
    connection_manager.disconnect().await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn update_connection_config(
    p2p_enabled: bool,
    relay_enabled: bool,
    auto_fallback: bool,
    relay_server_url: String,
) -> Result<(), String> {
    info!("Updating connection configuration");
    
    let connection_manager = ConnectionManager::new();
    
    let mut relay_config = network::relay_client::RelayConfig::default();
    relay_config.server_url = relay_server_url;
    relay_config.enabled = relay_enabled;
    
    let new_config = ConnectionConfig {
        p2p_enabled,
        relay_enabled,
        auto_fallback_to_relay: auto_fallback,
        connection_timeout_seconds: 30,
        relay_config,
    };
    
    connection_manager.update_config(new_config).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

// Streaming commands
#[tauri::command]
async fn initialize_streaming() -> Result<(), String> {
    info!("Initializing streaming manager");
    
    let streaming_manager = StreamingManager::new();
    let _event_receiver = streaming_manager.initialize().await.map_err(|e| e.to_string())?;
    
    info!("Streaming manager initialized");
    Ok(())
}

#[tauri::command]
async fn start_screen_streaming() -> Result<(), String> {
    info!("Starting screen streaming");
    
    let streaming_manager = StreamingManager::new();
    let _event_receiver = streaming_manager.initialize().await.map_err(|e| e.to_string())?;
    
    streaming_manager.start_streaming().await.map_err(|e| e.to_string())?;
    
    info!("Screen streaming started");
    Ok(())
}

#[tauri::command]
async fn stop_screen_streaming() -> Result<(), String> {
    info!("Stopping screen streaming");
    
    let streaming_manager = StreamingManager::new();
    streaming_manager.stop_streaming().await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn get_streaming_stats() -> Result<StreamingStats, String> {
    let streaming_manager = StreamingManager::new();
    let stats = streaming_manager.get_stats().await;
    
    Ok(stats)
}

#[tauri::command]
async fn adjust_streaming_quality(quality: u8) -> Result<(), String> {
    info!("Adjusting streaming quality to {}", quality);
    
    let streaming_manager = StreamingManager::new();
    streaming_manager.adjust_quality(quality).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn update_streaming_config(
    target_fps: u32,
    quality: u8,
    compression_type: String,
    adaptive_quality: bool,
    max_bandwidth_mbps: f32,
    enable_delta_compression: bool,
) -> Result<(), String> {
    info!("Updating streaming configuration");
    
    let compression_type = match compression_type.as_str() {
        "jpeg" => streaming::CompressionType::JPEG,
        "webp" => streaming::CompressionType::WebP,
        "h264" => streaming::CompressionType::H264,
        "vp8" => streaming::CompressionType::VP8,
        "av1" => streaming::CompressionType::AV1,
        _ => streaming::CompressionType::JPEG,
    };
    
    let new_config = StreamingConfig {
        target_fps,
        quality,
        compression_type,
        adaptive_quality,
        max_bandwidth_mbps,
        enable_delta_compression,
        buffer_size: 3,
    };
    
    let streaming_manager = StreamingManager::new();
    streaming_manager.update_config(new_config).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn get_latest_frame() -> Result<Option<Vec<u8>>, String> {
    let streaming_manager = StreamingManager::new();
    let frame = streaming_manager.get_latest_frame().await;
    
    Ok(frame)
}

// Permission commands
#[tauri::command]
async fn initialize_permissions() -> Result<(), String> {
    info!("Initializing permission manager");
    
    let permission_manager = PermissionManager::new();
    let _event_receiver = permission_manager.initialize().await.map_err(|e| e.to_string())?;
    
    info!("Permission manager initialized");
    Ok(())
}

#[tauri::command]
async fn request_permission(
    connection_id: String,
    device_name: String,
    device_os: String,
    device_version: String,
    ip_address: Option<String>,
    permissions: Vec<String>,
) -> Result<String, String> {
    info!("Permission request from {}: {:?}", device_name, permissions);
    
    let device_info = PermissionDeviceInfo {
        name: device_name,
        os: device_os,
        version: device_version,
        ip_address,
    };
    
    let requested_permissions: Vec<Permission> = permissions.iter()
        .filter_map(|p| match p.as_str() {
            "screen_view" => Some(Permission::ScreenView),
            "input_control" => Some(Permission::InputControl),
            "file_transfer" => Some(Permission::FileTransfer),
            "clipboard" => Some(Permission::Clipboard),
            "audio_access" => Some(Permission::AudioAccess),
            "system_info" => Some(Permission::SystemInfo),
            _ => None,
        })
        .collect();
    
    let permission_manager = PermissionManager::new();
    let _event_receiver = permission_manager.initialize().await.map_err(|e| e.to_string())?;
    
    let request_id = permission_manager
        .request_permission(connection_id, device_info, requested_permissions)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(request_id)
}

#[tauri::command]
async fn respond_to_permission_request(
    request_id: String,
    granted: bool,
    permissions: Vec<String>,
    duration_minutes: Option<u32>,
    deny_reason: Option<String>,
) -> Result<(), String> {
    info!("Responding to permission request {}: granted={}", request_id, granted);
    
    let response = if granted {
        let granted_permissions: Vec<Permission> = permissions.iter()
            .filter_map(|p| match p.as_str() {
                "screen_view" => Some(Permission::ScreenView),
                "input_control" => Some(Permission::InputControl),
                "file_transfer" => Some(Permission::FileTransfer),
                "clipboard" => Some(Permission::Clipboard),
                "audio_access" => Some(Permission::AudioAccess),
                "system_info" => Some(Permission::SystemInfo),
                _ => None,
            })
            .collect();
        
        PermissionResponse::Granted {
            permissions: granted_permissions,
            duration_minutes,
        }
    } else {
        PermissionResponse::Denied {
            reason: deny_reason.unwrap_or_else(|| "Access denied".to_string()),
        }
    };
    
    let permission_manager = PermissionManager::new();
    permission_manager
        .respond_to_request(request_id, response)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn check_permission(connection_id: String, permission: String) -> Result<bool, String> {
    let permission_enum = match permission.as_str() {
        "screen_view" => Permission::ScreenView,
        "input_control" => Permission::InputControl,
        "file_transfer" => Permission::FileTransfer,
        "clipboard" => Permission::Clipboard,
        "audio_access" => Permission::AudioAccess,
        "system_info" => Permission::SystemInfo,
        _ => return Ok(false),
    };
    
    let permission_manager = PermissionManager::new();
    let has_permission = permission_manager
        .check_permission(&connection_id, &permission_enum)
        .await;
    
    Ok(has_permission)
}

#[tauri::command]
async fn revoke_permissions(
    connection_id: String,
    permissions: Option<Vec<String>>,
) -> Result<(), String> {
    info!("Revoking permissions for connection: {}", connection_id);
    
    let revoke_permissions = permissions.map(|perms| {
        perms.iter()
            .filter_map(|p| match p.as_str() {
                "screen_view" => Some(Permission::ScreenView),
                "input_control" => Some(Permission::InputControl),
                "file_transfer" => Some(Permission::FileTransfer),
                "clipboard" => Some(Permission::Clipboard),
                "audio_access" => Some(Permission::AudioAccess),
                "system_info" => Some(Permission::SystemInfo),
                _ => None,
            })
            .collect()
    });
    
    let permission_manager = PermissionManager::new();
    permission_manager
        .revoke_permissions(&connection_id, revoke_permissions)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn get_active_permissions() -> Result<Vec<serde_json::Value>, String> {
    let permission_manager = PermissionManager::new();
    let grants = permission_manager.get_active_grants().await;
    
    let grants_json: Vec<serde_json::Value> = grants.iter()
        .map(|grant| serde_json::to_value(grant).unwrap_or_default())
        .collect();
    
    Ok(grants_json)
}

#[tauri::command]
async fn get_pending_permission_requests() -> Result<Vec<serde_json::Value>, String> {
    let permission_manager = PermissionManager::new();
    let requests = permission_manager.get_pending_requests().await;
    
    let requests_json: Vec<serde_json::Value> = requests.iter()
        .map(|request| serde_json::to_value(request).unwrap_or_default())
        .collect();
    
    Ok(requests_json)
}

#[tauri::command]
async fn update_permission_config(
    require_screen_view: bool,
    require_input_control: bool,
    require_file_transfer: bool,
    auto_deny_minutes: u32,
    max_connections: usize,
    enable_whitelist: bool,
    whitelisted_devices: Vec<String>,
    default_session_minutes: Option<u32>,
) -> Result<(), String> {
    info!("Updating permission configuration");
    
    let new_config = PermissionConfig {
        require_permission_for_screen_view: require_screen_view,
        require_permission_for_input_control: require_input_control,
        require_permission_for_file_transfer: require_file_transfer,
        auto_deny_after_minutes: auto_deny_minutes,
        max_concurrent_connections: max_connections,
        enable_whitelist,
        whitelisted_devices,
        default_session_duration_minutes: default_session_minutes,
    };
    
    let permission_manager = PermissionManager::new();
    permission_manager
        .update_config(new_config)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

// Metrics and monitoring commands
#[tauri::command]
async fn initialize_metrics() -> Result<(), String> {
    info!("Initializing metrics collector");
    
    let metrics_collector = MetricsCollector::new();
    metrics_collector.start_collection().await.map_err(|e| e.to_string())?;
    
    info!("Metrics collector initialized");
    Ok(())
}

#[tauri::command]
async fn get_connection_metrics(connection_id: String) -> Result<Option<ConnectionMetrics>, String> {
    let metrics_collector = MetricsCollector::new();
    let metrics = metrics_collector.get_connection_metrics(&connection_id).await;
    
    Ok(metrics)
}

#[tauri::command]
async fn get_all_connection_metrics() -> Result<Vec<ConnectionMetrics>, String> {
    let metrics_collector = MetricsCollector::new();
    let all_metrics = metrics_collector.get_all_connection_metrics().await;
    
    Ok(all_metrics.into_values().collect())
}

#[tauri::command]
async fn get_system_metrics() -> Result<Option<SystemMetrics>, String> {
    let metrics_collector = MetricsCollector::new();
    let metrics = metrics_collector.get_system_metrics().await;
    
    Ok(metrics)
}

#[tauri::command]
async fn get_quality_metrics() -> Result<Option<QualityMetrics>, String> {
    let metrics_collector = MetricsCollector::new();
    let metrics = metrics_collector.get_quality_metrics().await;
    
    Ok(metrics)
}

#[tauri::command]
async fn get_performance_alerts() -> Result<Vec<serde_json::Value>, String> {
    let metrics_collector = MetricsCollector::new();
    let alerts = metrics_collector.get_alerts().await;
    
    let alerts_json: Vec<serde_json::Value> = alerts.iter()
        .map(|alert| serde_json::to_value(alert).unwrap_or_default())
        .collect();
    
    Ok(alerts_json)
}

#[tauri::command]
async fn acknowledge_alert(alert_id: String) -> Result<(), String> {
    info!("Acknowledging alert: {}", alert_id);
    
    let metrics_collector = MetricsCollector::new();
    metrics_collector.acknowledge_alert(&alert_id).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn clear_acknowledged_alerts() -> Result<(), String> {
    info!("Clearing acknowledged alerts");
    
    let metrics_collector = MetricsCollector::new();
    metrics_collector.clear_acknowledged_alerts().await;
    
    Ok(())
}

#[tauri::command]
async fn update_alert_thresholds(
    max_latency_ms: f32,
    max_packet_loss_percent: f32,
    min_bandwidth_mbps: f32,
    max_cpu_usage_percent: f32,
    max_memory_usage_percent: f32,
    min_quality_score: f32,
    max_frame_drops_per_second: f32,
) -> Result<(), String> {
    info!("Updating alert thresholds");
    
    let thresholds = AlertThresholds {
        max_latency_ms,
        max_packet_loss_percent,
        min_bandwidth_mbps,
        max_cpu_usage_percent,
        max_memory_usage_percent,
        min_quality_score,
        max_frame_drops_per_second,
    };
    
    let metrics_collector = MetricsCollector::new();
    metrics_collector.update_alert_thresholds(thresholds).await;
    
    Ok(())
}

#[tauri::command]
async fn record_connection_metrics(
    connection_id: String,
    connection_type: String,
    latency_ms: f32,
    bandwidth_mbps: f32,
    packet_loss_percent: f32,
    jitter_ms: f32,
    quality_score: f32,
) -> Result<(), String> {
    let conn_type = match connection_type.as_str() {
        "p2p" => metrics::ConnectionType::P2P,
        "relay" => metrics::ConnectionType::Relay,
        _ => metrics::ConnectionType::P2P,
    };
    
    let metrics = ConnectionMetrics {
        connection_id,
        connection_type: conn_type,
        latency_ms,
        bandwidth_mbps,
        packet_loss_percent,
        jitter_ms,
        quality_score,
        last_updated: chrono::Utc::now(),
    };
    
    let metrics_collector = MetricsCollector::new();
    metrics_collector.record_connection_metrics(metrics).await;
    
    Ok(())
}

// Performance testing commands
#[tauri::command]
async fn run_performance_tests(
    test_duration_seconds: u64,
    target_fps: u32,
    benchmark_iterations: u32,
) -> Result<Vec<PerformanceTestResult>, String> {
    info!("Starting performance tests");
    
    let test_config = PerformanceTestConfig {
        test_duration_seconds,
        target_fps,
        test_compression_types: vec![
            streaming::CompressionType::JPEG,
            streaming::CompressionType::WebP,
        ],
        test_quality_levels: vec![50, 75, 90],
        connection_timeout_seconds: 10,
        benchmark_iterations,
    };
    
    let mut tester = PerformanceTester::new(test_config);
    let results = tester.run_comprehensive_tests().await.map_err(|e| e.to_string())?;
    
    info!("Performance tests completed with {} results", results.len());
    Ok(results)
}

#[tauri::command]
async fn run_p2p_vs_relay_test(test_duration_seconds: u64) -> Result<Vec<PerformanceTestResult>, String> {
    info!("Running P2P vs Relay comparison test");
    
    let test_config = PerformanceTestConfig {
        test_duration_seconds,
        target_fps: 30,
        test_compression_types: vec![streaming::CompressionType::JPEG],
        test_quality_levels: vec![75],
        connection_timeout_seconds: 10,
        benchmark_iterations: 1,
    };
    
    let mut tester = PerformanceTester::new(test_config);
    tester.connection_manager.initialize().await.map_err(|e| e.to_string())?;
    tester.streaming_manager.initialize().await.map_err(|e| e.to_string())?;
    
    // Run P2P test
    let p2p_result = tester.run_connection_test(ConnectionType::P2P).await.map_err(|e| e.to_string())?;
    
    // Run Relay test  
    let relay_result = tester.run_connection_test(ConnectionType::Relay).await.map_err(|e| e.to_string())?;
    
    let results = vec![p2p_result, relay_result];
    
    info!("P2P vs Relay test completed");
    Ok(results)
}

#[tauri::command]
async fn generate_performance_report(results: Vec<PerformanceTestResult>) -> Result<String, String> {
    info!("Generating performance report");
    
    let test_config = PerformanceTestConfig::default();
    let mut tester = PerformanceTester::new(test_config);
    
    // Set results in tester (this is a simplified approach)
    // In a real implementation, we'd store results persistently
    
    let report = tester.generate_report().await.map_err(|e| e.to_string())?;
    
    Ok(report)
}

#[tauri::command]
async fn benchmark_compression_algorithms() -> Result<Vec<PerformanceTestResult>, String> {
    info!("Benchmarking compression algorithms");
    
    let test_config = PerformanceTestConfig {
        test_duration_seconds: 15,
        target_fps: 30,
        test_compression_types: vec![
            streaming::CompressionType::JPEG,
            streaming::CompressionType::WebP,
            streaming::CompressionType::H264,
        ],
        test_quality_levels: vec![75],
        connection_timeout_seconds: 10,
        benchmark_iterations: 3,
    };
    
    let mut tester = PerformanceTester::new(test_config);
    tester.streaming_manager.initialize().await.map_err(|e| e.to_string())?;
    
    let results = tester.test_compression_algorithms().await.map_err(|e| e.to_string())?;
    
    info!("Compression benchmark completed with {} results", results.len());
    Ok(results)
}

#[tauri::command]
async fn test_quality_levels() -> Result<Vec<PerformanceTestResult>, String> {
    info!("Testing different quality levels");
    
    let test_config = PerformanceTestConfig {
        test_duration_seconds: 10,
        target_fps: 30,
        test_compression_types: vec![streaming::CompressionType::JPEG],
        test_quality_levels: vec![25, 50, 75, 90, 95],
        connection_timeout_seconds: 10,
        benchmark_iterations: 1,
    };
    
    let mut tester = PerformanceTester::new(test_config);
    tester.streaming_manager.initialize().await.map_err(|e| e.to_string())?;
    
    let results = tester.test_quality_levels().await.map_err(|e| e.to_string())?;
    
    info!("Quality level testing completed with {} results", results.len());
    Ok(results)
}

fn create_system_tray() -> SystemTray {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let show = CustomMenuItem::new("show".to_string(), "Show");
    let start_host = CustomMenuItem::new("start_host".to_string(), "Start Hosting");
    let stop_host = CustomMenuItem::new("stop_host".to_string(), "Stop Hosting");
    
    let tray_menu = SystemTrayMenu::new()
        .add_item(show)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(start_host)
        .add_item(stop_host)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);
    
    SystemTray::new().with_menu(tray_menu)
}

fn handle_system_tray_event(app: &tauri::AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick { .. } => {
            let window = app.get_window("main").unwrap();
            window.show().unwrap();
            window.set_focus().unwrap();
        }
        SystemTrayEvent::MenuItemClick { id, .. } => {
            match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "start_host" => {
                    // Emit event to frontend to start hosting
                    let window = app.get_window("main").unwrap();
                    window.emit("start-host-requested", {}).unwrap();
                }
                "stop_host" => {
                    // Emit event to frontend to stop hosting
                    let window = app.get_window("main").unwrap();
                    window.emit("stop-host-requested", {}).unwrap();
                }
                _ => {}
            }
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    info!("Starting AnyViewer application");
    
    // Load configuration
    let _config = AppConfig::load().unwrap_or_else(|e| {
        error!("Failed to load config: {}", e);
        AppConfig::default()
    });
    
    let system_tray = create_system_tray();
    
    tauri::Builder::default()
        .system_tray(system_tray)
        .on_system_tray_event(handle_system_tray_event)
        .invoke_handler(tauri::generate_handler![
            start_host_session,
            connect_to_session,
            capture_screen,
            send_input_event,
            get_system_info,
            generate_session_id,
            initialize_security,
            initialize_connection_requests,
            create_connection_request,
            respond_to_connection_request,
            get_pending_connection_requests,
            get_accepted_connection_requests,
            start_screen_sharing_for_request,
            cancel_connection_request,
            test_create_connection_request,
            test_udp_broadcast,
            start_network_discovery,
            stop_network_discovery,
            get_discovered_devices,
            connect_to_discovered_device,
            send_connection_request_to_device,
            connect_to_ip,
            initialize_connection_manager,
            start_hosting_with_fallback,
            connect_to_host_with_fallback,
            get_connection_status,
            get_available_peers,
            disconnect_connection,
            update_connection_config,
            initialize_streaming,
            start_screen_streaming,
            stop_screen_streaming,
            get_streaming_stats,
            adjust_streaming_quality,
            update_streaming_config,
            get_latest_frame,
            initialize_permissions,
            request_permission,
            respond_to_permission_request,
            check_permission,
            revoke_permissions,
            get_active_permissions,
            get_pending_permission_requests,
            update_permission_config,
            initialize_metrics,
            get_connection_metrics,
            get_all_connection_metrics,
            get_system_metrics,
            get_quality_metrics,
            get_performance_alerts,
            acknowledge_alert,
            clear_acknowledged_alerts,
            update_alert_thresholds,
            record_connection_metrics,
            run_performance_tests,
            run_p2p_vs_relay_test,
            generate_performance_report,
            benchmark_compression_algorithms,
            test_quality_levels
        ])
        .setup(|_app| {
            info!("AnyViewer application setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}