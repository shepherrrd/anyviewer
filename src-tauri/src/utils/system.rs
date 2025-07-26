use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;

pub async fn get_system_info() -> Result<Value> {
    let mut info = HashMap::new();
    
    // Basic system information
    info.insert("os", json!(std::env::consts::OS));
    info.insert("arch", json!(std::env::consts::ARCH));
    info.insert("family", json!(std::env::consts::FAMILY));
    
    // Get hostname
    if let Ok(hostname) = gethostname::gethostname().into_string() {
        info.insert("hostname", json!(hostname));
    }
    
    // Get current user
    if let Ok(username) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        info.insert("username", json!(username));
    }
    
    // Memory information (simplified)
    #[cfg(target_os = "macos")]
    {
        if let Ok(memory_info) = get_macos_memory_info() {
            info.insert("memory", memory_info);
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        if let Ok(memory_info) = get_windows_memory_info() {
            info.insert("memory", memory_info);
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        if let Ok(memory_info) = get_linux_memory_info() {
            info.insert("memory", memory_info);
        }
    }
    
    // CPU information
    info.insert("cpu_count", json!(num_cpus::get()));
    
    // Screen information
    if let Ok(screens) = get_screen_info().await {
        info.insert("screens", screens);
    }
    
    // Network interfaces
    if let Ok(interfaces) = get_network_interfaces() {
        info.insert("network_interfaces", interfaces);
    }
    
    // Application information
    info.insert("app_name", json!("AnyViewer"));
    info.insert("app_version", json!(env!("CARGO_PKG_VERSION")));
    info.insert("build_timestamp", json!(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()));
    
    Ok(json!(info))
}

#[cfg(target_os = "macos")]
fn get_macos_memory_info() -> Result<Value> {
    // Use system_profiler or sysctl to get memory info
    let output = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()?;
    
    if output.status.success() {
        let memory_bytes = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()?;
        
        return Ok(json!({
            "total_bytes": memory_bytes,
            "total_gb": memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        }));
    }
    
    Ok(json!({}))
}

#[cfg(target_os = "windows")]
fn get_windows_memory_info() -> Result<Value> {
    // Use wmic or PowerShell to get memory info
    let output = std::process::Command::new("wmic")
        .args(["computersystem", "get", "TotalPhysicalMemory", "/value"])
        .output()?;
    
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.starts_with("TotalPhysicalMemory=") {
                if let Ok(memory_bytes) = line
                    .strip_prefix("TotalPhysicalMemory=")
                    .unwrap_or("0")
                    .parse::<u64>()
                {
                    return Ok(json!({
                        "total_bytes": memory_bytes,
                        "total_gb": memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    }));
                }
            }
        }
    }
    
    Ok(json!({}))
}

#[cfg(target_os = "linux")]
fn get_linux_memory_info() -> Result<Value> {
    let meminfo = std::fs::read_to_string("/proc/meminfo")?;
    let mut total_kb = 0u64;
    let mut available_kb = 0u64;
    
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(value) = line.split_whitespace().nth(1) {
                total_kb = value.parse().unwrap_or(0);
            }
        } else if line.starts_with("MemAvailable:") {
            if let Some(value) = line.split_whitespace().nth(1) {
                available_kb = value.parse().unwrap_or(0);
            }
        }
    }
    
    let total_bytes = total_kb * 1024;
    let available_bytes = available_kb * 1024;
    
    Ok(json!({
        "total_bytes": total_bytes,
        "available_bytes": available_bytes,
        "total_gb": total_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        "available_gb": available_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }))
}

async fn get_screen_info() -> Result<Value> {
    let screens = screenshots::Screen::all()?;
    let mut screen_info = Vec::new();
    
    for (index, screen) in screens.iter().enumerate() {
        let display = &screen.display_info;
        screen_info.push(json!({
            "index": index,
            "width": display.width,
            "height": display.height,
            "x": display.x,
            "y": display.y,
            "scale_factor": display.scale_factor,
            "is_primary": index == 0 // Assume first screen is primary
        }));
    }
    
    Ok(json!(screen_info))
}

fn get_network_interfaces() -> Result<Value> {
    let mut interfaces = Vec::new();
    
    // Get network interfaces using system commands
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("ifconfig").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                interfaces = parse_ifconfig_output(&output_str);
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("ip").args(["addr", "show"]).output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                interfaces = parse_ip_addr_output(&output_str);
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("ipconfig").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                interfaces = parse_ipconfig_output(&output_str);
            }
        }
    }
    
    Ok(json!(interfaces))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_ifconfig_output(output: &str) -> Vec<Value> {
    let mut interfaces = Vec::new();
    let mut current_interface: Option<Value> = None;
    
    for line in output.lines() {
        let line = line.trim();
        
        if !line.starts_with(' ') && line.contains(':') {
            // New interface
            if let Some(interface) = current_interface.take() {
                interfaces.push(interface);
            }
            
            let interface_name = line.split(':').next().unwrap_or("unknown").to_string();
            current_interface = Some(json!({
                "name": interface_name,
                "addresses": []
            }));
        } else if line.starts_with("inet ") && current_interface.is_some() {
            // IP address line
            if let Some(addr) = line.split_whitespace().nth(1) {
                if let Some(ref mut interface) = current_interface {
                    if let Some(addresses) = interface["addresses"].as_array_mut() {
                        addresses.push(json!(addr));
                    }
                }
            }
        }
    }
    
    if let Some(interface) = current_interface {
        interfaces.push(interface);
    }
    
    interfaces
}

#[cfg(target_os = "linux")]
fn parse_ip_addr_output(output: &str) -> Vec<Value> {
    let mut interfaces = Vec::new();
    let mut current_interface: Option<Value> = None;
    
    for line in output.lines() {
        let line = line.trim();
        
        if line.starts_with(char::is_numeric) {
            // New interface
            if let Some(interface) = current_interface.take() {
                interfaces.push(interface);
            }
            
            if let Some(name_part) = line.split(':').nth(1) {
                let interface_name = name_part.trim().split_whitespace().next().unwrap_or("unknown");
                current_interface = Some(json!({
                    "name": interface_name,
                    "addresses": []
                }));
            }
        } else if line.starts_with("inet ") && current_interface.is_some() {
            // IP address line
            if let Some(addr_with_prefix) = line.split_whitespace().nth(1) {
                let addr = addr_with_prefix.split('/').next().unwrap_or(addr_with_prefix);
                if let Some(ref mut interface) = current_interface {
                    if let Some(addresses) = interface["addresses"].as_array_mut() {
                        addresses.push(json!(addr));
                    }
                }
            }
        }
    }
    
    if let Some(interface) = current_interface {
        interfaces.push(interface);
    }
    
    interfaces
}

#[cfg(target_os = "windows")]
fn parse_ipconfig_output(output: &str) -> Vec<Value> {
    let mut interfaces = Vec::new();
    let mut current_interface: Option<Value> = None;
    
    for line in output.lines() {
        let line = line.trim();
        
        if line.ends_with(':') && !line.starts_with(' ') {
            // New interface
            if let Some(interface) = current_interface.take() {
                interfaces.push(interface);
            }
            
            let interface_name = line.trim_end_matches(':').to_string();
            current_interface = Some(json!({
                "name": interface_name,
                "addresses": []
            }));
        } else if (line.contains("IPv4 Address") || line.contains("IP Address")) && current_interface.is_some() {
            // IP address line
            if let Some(addr) = line.split(':').nth(1) {
                let addr = addr.trim();
                if let Some(ref mut interface) = current_interface {
                    if let Some(addresses) = interface["addresses"].as_array_mut() {
                        addresses.push(json!(addr));
                    }
                }
            }
        }
    }
    
    if let Some(interface) = current_interface {
        interfaces.push(interface);
    }
    
    interfaces
}

pub fn get_platform_info() -> Value {
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY,
        "pointer_width": std::mem::size_of::<usize>() * 8,
        "endian": if cfg!(target_endian = "little") { "little" } else { "big" }
    })
}

pub fn is_admin() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Check if running as administrator on Windows
        use std::process::Command;
        if let Ok(output) = Command::new("net").args(["session"]).output() {
            return output.status.success();
        }
    }
    
    #[cfg(unix)]
    {
        // Check if running as root on Unix-like systems
        unsafe {
            return libc::geteuid() == 0;
        }
    }
    
    false
}