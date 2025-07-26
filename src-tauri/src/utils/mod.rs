pub mod system;
pub mod logging;
pub mod performance;
pub mod id_generator;
pub mod file_transfer;

use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn get_current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;
    
    if bytes < THRESHOLD {
        return format!("{} B", bytes);
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }
    
    format!("{:.1} {}", size, UNITS[unit_index])
}

pub fn format_duration(duration_ms: u64) -> String {
    if duration_ms < 1000 {
        return format!("{}ms", duration_ms);
    }
    
    let seconds = duration_ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes % 60, seconds % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

pub fn validate_network_address(address: &str) -> bool {
    // Simple validation for IP:PORT format
    if let Some((ip, port)) = address.split_once(':') {
        // Validate IP address
        if ip.parse::<std::net::IpAddr>().is_err() && ip != "localhost" {
            return false;
        }
        
        // Validate port
        if let Ok(port_num) = port.parse::<u16>() {
            port_num > 0
        } else {
            false
        }
    } else {
        false
    }
}

pub fn generate_session_id() -> String {
    use rand::Rng;
    
    let mut rng = rand::thread_rng();
    let id: u32 = rng.gen_range(100_000_000..=999_999_999);
    format!("{:03}-{:03}-{:03}", id / 1_000_000, (id / 1_000) % 1_000, id % 1_000)
}

pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn create_error_response(code: u32, message: &str) -> Value {
    json!({
        "error": {
            "code": code,
            "message": message,
            "timestamp": get_current_timestamp()
        }
    })
}

pub fn create_success_response(data: Value) -> Value {
    json!({
        "success": true,
        "data": data,
        "timestamp": get_current_timestamp()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1s");
        assert_eq!(format_duration(65000), "1m 5s");
        assert_eq!(format_duration(3665000), "1h 1m 5s");
    }
    
    #[test]
    fn test_validate_network_address() {
        assert!(validate_network_address("127.0.0.1:8080"));
        assert!(validate_network_address("localhost:3000"));
        assert!(validate_network_address("192.168.1.1:22"));
        assert!(!validate_network_address("invalid"));
        assert!(!validate_network_address("127.0.0.1"));
        assert!(!validate_network_address("127.0.0.1:70000"));
    }
    
    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test file.txt"), "test_file.txt");
        assert_eq!(sanitize_filename("file@name#.log"), "file_name_.log");
        assert_eq!(sanitize_filename("normal-file_name.dat"), "normal-file_name.dat");
    }
    
    #[test]
    fn test_generate_session_id() {
        let id = generate_session_id();
        assert_eq!(id.len(), 11); // XXX-XXX-XXX format
        assert_eq!(id.chars().nth(3), Some('-'));
        assert_eq!(id.chars().nth(7), Some('-'));
    }
}