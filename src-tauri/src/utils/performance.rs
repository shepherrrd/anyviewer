use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub encode_time_ms: f64,
    pub network_latency_ms: f64,
    pub cpu_usage: f64,
    pub memory_usage_mb: f64,
    pub bandwidth_kbps: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            encode_time_ms: 0.0,
            network_latency_ms: 0.0,
            cpu_usage: 0.0,
            memory_usage_mb: 0.0,
            bandwidth_kbps: 0.0,
        }
    }
}

pub struct PerformanceMonitor {
    frame_times: Arc<Mutex<VecDeque<Duration>>>,
    encode_times: Arc<Mutex<VecDeque<Duration>>>,
    network_latencies: Arc<Mutex<VecDeque<Duration>>>,
    bandwidth_samples: Arc<Mutex<VecDeque<u64>>>,
    last_frame_time: Arc<Mutex<Option<Instant>>>,
    max_samples: usize,
}

impl PerformanceMonitor {
    pub fn new(max_samples: usize) -> Self {
        Self {
            frame_times: Arc::new(Mutex::new(VecDeque::with_capacity(max_samples))),
            encode_times: Arc::new(Mutex::new(VecDeque::with_capacity(max_samples))),
            network_latencies: Arc::new(Mutex::new(VecDeque::with_capacity(max_samples))),
            bandwidth_samples: Arc::new(Mutex::new(VecDeque::with_capacity(max_samples))),
            last_frame_time: Arc::new(Mutex::new(None)),
            max_samples,
        }
    }
    
    pub fn record_frame(&self) {
        let now = Instant::now();
        let mut last_frame_time = self.last_frame_time.lock().unwrap();
        
        if let Some(last_time) = *last_frame_time {
            let frame_time = now.duration_since(last_time);
            
            let mut frame_times = self.frame_times.lock().unwrap();
            if frame_times.len() >= self.max_samples {
                frame_times.pop_front();
            }
            frame_times.push_back(frame_time);
        }
        
        *last_frame_time = Some(now);
    }
    
    pub fn record_encode_time(&self, duration: Duration) {
        let mut encode_times = self.encode_times.lock().unwrap();
        if encode_times.len() >= self.max_samples {
            encode_times.pop_front();
        }
        encode_times.push_back(duration);
    }
    
    pub fn record_network_latency(&self, duration: Duration) {
        let mut network_latencies = self.network_latencies.lock().unwrap();
        if network_latencies.len() >= self.max_samples {
            network_latencies.pop_front();
        }
        network_latencies.push_back(duration);
    }
    
    pub fn record_bandwidth(&self, bytes: u64) {
        let mut bandwidth_samples = self.bandwidth_samples.lock().unwrap();
        if bandwidth_samples.len() >= self.max_samples {
            bandwidth_samples.pop_front();
        }
        bandwidth_samples.push_back(bytes);
    }
    
    pub fn get_metrics(&self) -> PerformanceMetrics {
        let fps = self.calculate_fps();
        let frame_time_ms = self.calculate_average_frame_time();
        let encode_time_ms = self.calculate_average_encode_time();
        let network_latency_ms = self.calculate_average_network_latency();
        let bandwidth_kbps = self.calculate_bandwidth();
        
        PerformanceMetrics {
            fps,
            frame_time_ms,
            encode_time_ms,
            network_latency_ms,
            cpu_usage: self.get_cpu_usage(),
            memory_usage_mb: self.get_memory_usage(),
            bandwidth_kbps,
        }
    }
    
    fn calculate_fps(&self) -> f64 {
        let frame_times = self.frame_times.lock().unwrap();
        if frame_times.is_empty() {
            return 0.0;
        }
        
        let avg_frame_time: Duration = frame_times.iter().sum::<Duration>() / frame_times.len() as u32;
        if avg_frame_time.is_zero() {
            return 0.0;
        }
        
        1000.0 / avg_frame_time.as_millis() as f64
    }
    
    fn calculate_average_frame_time(&self) -> f64 {
        let frame_times = self.frame_times.lock().unwrap();
        if frame_times.is_empty() {
            return 0.0;
        }
        
        let avg_frame_time: Duration = frame_times.iter().sum::<Duration>() / frame_times.len() as u32;
        avg_frame_time.as_millis() as f64
    }
    
    fn calculate_average_encode_time(&self) -> f64 {
        let encode_times = self.encode_times.lock().unwrap();
        if encode_times.is_empty() {
            return 0.0;
        }
        
        let avg_encode_time: Duration = encode_times.iter().sum::<Duration>() / encode_times.len() as u32;
        avg_encode_time.as_millis() as f64
    }
    
    fn calculate_average_network_latency(&self) -> f64 {
        let network_latencies = self.network_latencies.lock().unwrap();
        if network_latencies.is_empty() {
            return 0.0;
        }
        
        let avg_latency: Duration = network_latencies.iter().sum::<Duration>() / network_latencies.len() as u32;
        avg_latency.as_millis() as f64
    }
    
    fn calculate_bandwidth(&self) -> f64 {
        let bandwidth_samples = self.bandwidth_samples.lock().unwrap();
        if bandwidth_samples.len() < 2 {
            return 0.0;
        }
        
        let total_bytes: u64 = bandwidth_samples.iter().sum();
        let time_span = Duration::from_secs(bandwidth_samples.len() as u64); // Assuming 1 sample per second
        
        if time_span.is_zero() {
            return 0.0;
        }
        
        (total_bytes as f64 * 8.0) / (time_span.as_secs() as f64 * 1000.0) // Convert to kbps
    }
    
    fn get_cpu_usage(&self) -> f64 {
        // Simplified CPU usage calculation
        // In a real implementation, you'd use platform-specific APIs
        #[cfg(target_os = "linux")]
        {
            if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
                if let Some(cpu_line) = stat.lines().next() {
                    // Parse CPU usage from /proc/stat
                    // This is a simplified implementation
                    return self.parse_cpu_usage(&cpu_line);
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            // Use system_profiler or top command
            if let Ok(output) = std::process::Command::new("top")
                .args(["-l", "1", "-n", "0"])
                .output()
            {
                if output.status.success() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    return self.parse_macos_cpu_usage(&output_str);
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Use wmic or typeperf
            if let Ok(output) = std::process::Command::new("wmic")
                .args(["cpu", "get", "loadpercentage", "/value"])
                .output()
            {
                if output.status.success() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    return self.parse_windows_cpu_usage(&output_str);
                }
            }
        }
        
        0.0
    }
    
    #[cfg(target_os = "linux")]
    fn parse_cpu_usage(&self, cpu_line: &str) -> f64 {
        let values: Vec<u64> = cpu_line
            .split_whitespace()
            .skip(1) // Skip "cpu"
            .take(7) // Take first 7 values
            .filter_map(|s| s.parse().ok())
            .collect();
        
        if values.len() >= 4 {
            let idle = values[3];
            let total: u64 = values.iter().sum();
            if total > 0 {
                return ((total - idle) as f64 / total as f64) * 100.0;
            }
        }
        
        0.0
    }
    
    #[cfg(target_os = "macos")]
    fn parse_macos_cpu_usage(&self, output: &str) -> f64 {
        for line in output.lines() {
            if line.contains("CPU usage:") {
                // Parse line like "CPU usage: 12.34% user, 5.67% sys, 81.99% idle"
                if let Some(user_part) = line.split(',').next() {
                    if let Some(percentage_str) = user_part.split_whitespace().nth(2) {
                        if let Ok(percentage) = percentage_str.trim_end_matches('%').parse::<f64>() {
                            return percentage;
                        }
                    }
                }
            }
        }
        0.0
    }
    
    #[cfg(target_os = "windows")]
    fn parse_windows_cpu_usage(&self, output: &str) -> f64 {
        for line in output.lines() {
            if line.starts_with("LoadPercentage=") {
                if let Ok(percentage) = line
                    .strip_prefix("LoadPercentage=")
                    .unwrap_or("0")
                    .parse::<f64>()
                {
                    return percentage;
                }
            }
        }
        0.0
    }
    
    fn get_memory_usage(&self) -> f64 {
        // Get current process memory usage
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<f64>() {
                                return kb / 1024.0; // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
            {
                if output.status.success() {
                    if let Ok(kb) = String::from_utf8_lossy(&output.stdout).trim().parse::<f64>() {
                        return kb / 1024.0; // Convert KB to MB
                    }
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Use tasklist or wmic
            if let Ok(output) = std::process::Command::new("tasklist")
                .args(["/fi", &format!("PID eq {}", std::process::id()), "/fo", "csv"])
                .output()
            {
                if output.status.success() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    // Parse CSV output to get memory usage
                    // This is a simplified implementation
                    return self.parse_windows_memory_usage(&output_str);
                }
            }
        }
        
        0.0
    }
    
    #[cfg(target_os = "windows")]
    fn parse_windows_memory_usage(&self, output: &str) -> f64 {
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() >= 2 {
            let data_line = lines[1];
            let fields: Vec<&str> = data_line.split(',').collect();
            if fields.len() >= 5 {
                let memory_str = fields[4].trim_matches('"').replace(",", "");
                if let Some(memory_part) = memory_str.split_whitespace().next() {
                    if let Ok(kb) = memory_part.parse::<f64>() {
                        return kb / 1024.0; // Convert KB to MB
                    }
                }
            }
        }
        0.0
    }
    
    pub fn reset(&self) {
        self.frame_times.lock().unwrap().clear();
        self.encode_times.lock().unwrap().clear();
        self.network_latencies.lock().unwrap().clear();
        self.bandwidth_samples.lock().unwrap().clear();
        *self.last_frame_time.lock().unwrap() = None;
    }
    
    pub fn export_metrics_csv(&self) -> String {
        let metrics = self.get_metrics();
        format!(
            "timestamp,fps,frame_time_ms,encode_time_ms,network_latency_ms,cpu_usage,memory_usage_mb,bandwidth_kbps\n{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            metrics.fps,
            metrics.frame_time_ms,
            metrics.encode_time_ms,
            metrics.network_latency_ms,
            metrics.cpu_usage,
            metrics.memory_usage_mb,
            metrics.bandwidth_kbps
        )
    }
}

pub struct PerformanceTimer {
    name: String,
    start_time: Instant,
}

impl PerformanceTimer {
    pub fn start(name: &str) -> Self {
        Self {
            name: name.to_string(),
            start_time: Instant::now(),
        }
    }
    
    pub fn stop(self) -> Duration {
        let duration = self.start_time.elapsed();
        log::debug!("Timer '{}': {:.2}ms", self.name, duration.as_millis());
        duration
    }
}

#[macro_export]
macro_rules! perf_timer {
    ($name:expr, $block:block) => {{
        let timer = $crate::utils::performance::PerformanceTimer::start($name);
        let result = $block;
        timer.stop();
        result
    }};
}