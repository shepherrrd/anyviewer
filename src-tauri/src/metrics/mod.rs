use anyhow::Result;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant, interval};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetrics {
    pub connection_id: String,
    pub connection_type: ConnectionType,
    pub latency_ms: f32,
    pub bandwidth_mbps: f32,
    pub packet_loss_percent: f32,
    pub jitter_ms: f32,
    pub quality_score: f32, // 0-100
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    P2P,
    Relay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_usage_percent: f32,
    pub network_rx_mbps: f32,
    pub network_tx_mbps: f32,
    pub screen_capture_fps: f32,
    pub encoding_fps: f32,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub video_quality_score: f32,     // 0-100
    pub input_responsiveness_ms: f32,
    pub frame_drops_per_second: f32,
    pub compression_efficiency: f32,   // 0-1 (higher = better compression)
    pub user_satisfaction_score: f32, // 0-100 (based on various factors)
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    pub id: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub metric_value: f32,
    pub threshold: f32,
    pub connection_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    HighLatency,
    PacketLoss,
    LowBandwidth,
    HighCpuUsage,
    HighMemoryUsage,
    LowQualityScore,
    ConnectionUnstable,
    FrameDrops,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
struct MetricHistory<T> {
    values: VecDeque<(Instant, T)>,
    max_size: usize,
}

impl<T> MetricHistory<T> {
    fn new(max_size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    fn add(&mut self, value: T) {
        let now = Instant::now();
        self.values.push_back((now, value));
        
        if self.values.len() > self.max_size {
            self.values.pop_front();
        }
    }
    
    fn get_recent(&self, duration: Duration) -> Vec<&T> {
        let cutoff = Instant::now() - duration;
        self.values.iter()
            .filter(|(timestamp, _)| *timestamp >= cutoff)
            .map(|(_, value)| value)
            .collect()
    }
    
    fn latest(&self) -> Option<&T> {
        self.values.back().map(|(_, value)| value)
    }
}

pub struct MetricsCollector {
    connection_metrics: Arc<RwLock<HashMap<String, MetricHistory<ConnectionMetrics>>>>,
    system_metrics: Arc<RwLock<MetricHistory<SystemMetrics>>>,
    quality_metrics: Arc<RwLock<MetricHistory<QualityMetrics>>>,
    alerts: Arc<RwLock<Vec<PerformanceAlert>>>,
    alert_thresholds: Arc<RwLock<AlertThresholds>>,
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub max_latency_ms: f32,
    pub max_packet_loss_percent: f32,
    pub min_bandwidth_mbps: f32,
    pub max_cpu_usage_percent: f32,
    pub max_memory_usage_percent: f32,
    pub min_quality_score: f32,
    pub max_frame_drops_per_second: f32,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_latency_ms: 100.0,
            max_packet_loss_percent: 5.0,
            min_bandwidth_mbps: 1.0,
            max_cpu_usage_percent: 80.0,
            max_memory_usage_percent: 90.0,
            min_quality_score: 70.0,
            max_frame_drops_per_second: 2.0,
        }
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            connection_metrics: Arc::new(RwLock::new(HashMap::new())),
            system_metrics: Arc::new(RwLock::new(MetricHistory::new(300))), // 5 minutes of data
            quality_metrics: Arc::new(RwLock::new(MetricHistory::new(300))),
            alerts: Arc::new(RwLock::new(Vec::new())),
            alert_thresholds: Arc::new(RwLock::new(AlertThresholds::default())),
        }
    }
    
    pub async fn start_collection(&self) -> Result<()> {
        info!("Starting metrics collection");
        
        // Start system metrics collection
        let system_metrics = self.system_metrics.clone();
        let quality_metrics = self.quality_metrics.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                // Collect system metrics
                if let Ok(sys_metrics) = collect_system_metrics().await {
                    let mut metrics = system_metrics.write().await;
                    metrics.add(sys_metrics);
                }
                
                // Collect quality metrics
                if let Ok(qual_metrics) = collect_quality_metrics().await {
                    let mut metrics = quality_metrics.write().await;
                    metrics.add(qual_metrics);
                }
            }
        });
        
        // Start alert monitoring
        self.start_alert_monitoring().await;
        
        info!("Metrics collection started");
        Ok(())
    }
    
    pub async fn record_connection_metrics(&self, metrics: ConnectionMetrics) {
        let connection_id = metrics.connection_id.clone();
        
        let mut conn_metrics = self.connection_metrics.write().await;
        let history = conn_metrics.entry(connection_id)
            .or_insert_with(|| MetricHistory::new(300));
        history.add(metrics.clone());
        
        // Check for alerts
        self.check_connection_alerts(&metrics).await;
    }
    
    pub async fn get_connection_metrics(&self, connection_id: &str) -> Option<ConnectionMetrics> {
        let conn_metrics = self.connection_metrics.read().await;
        conn_metrics.get(connection_id)
            .and_then(|history| history.latest())
            .cloned()
    }
    
    pub async fn get_all_connection_metrics(&self) -> HashMap<String, ConnectionMetrics> {
        let conn_metrics = self.connection_metrics.read().await;
        conn_metrics.iter()
            .filter_map(|(id, history)| {
                history.latest().map(|metrics| (id.clone(), metrics.clone()))
            })
            .collect()
    }
    
    pub async fn get_system_metrics(&self) -> Option<SystemMetrics> {
        let sys_metrics = self.system_metrics.read().await;
        sys_metrics.latest().cloned()
    }
    
    pub async fn get_quality_metrics(&self) -> Option<QualityMetrics> {
        let qual_metrics = self.quality_metrics.read().await;
        qual_metrics.latest().cloned()
    }
    
    pub async fn get_connection_history(&self, connection_id: &str, duration: Duration) -> Vec<ConnectionMetrics> {
        let conn_metrics = self.connection_metrics.read().await;
        if let Some(history) = conn_metrics.get(connection_id) {
            history.get_recent(duration).into_iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
    
    pub async fn get_system_history(&self, duration: Duration) -> Vec<SystemMetrics> {
        let sys_metrics = self.system_metrics.read().await;
        sys_metrics.get_recent(duration).into_iter().cloned().collect()
    }
    
    pub async fn get_quality_history(&self, duration: Duration) -> Vec<QualityMetrics> {
        let qual_metrics = self.quality_metrics.read().await;
        qual_metrics.get_recent(duration).into_iter().cloned().collect()
    }
    
    pub async fn get_alerts(&self) -> Vec<PerformanceAlert> {
        let alerts = self.alerts.read().await;
        alerts.clone()
    }
    
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            info!("Acknowledged alert: {}", alert_id);
        }
        Ok(())
    }
    
    pub async fn clear_acknowledged_alerts(&self) {
        let mut alerts = self.alerts.write().await;
        alerts.retain(|alert| !alert.acknowledged);
    }
    
    pub async fn update_alert_thresholds(&self, thresholds: AlertThresholds) {
        let mut current_thresholds = self.alert_thresholds.write().await;
        *current_thresholds = thresholds;
        info!("Updated alert thresholds");
    }
    
    async fn start_alert_monitoring(&self) {
        let system_metrics = self.system_metrics.clone();
        let quality_metrics = self.quality_metrics.clone();
        let alerts = self.alerts.clone();
        let thresholds = self.alert_thresholds.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let thresholds = thresholds.read().await.clone();
                
                // Check system metrics alerts
                if let Some(sys_metrics) = system_metrics.read().await.latest() {
                    let mut new_alerts = Vec::new();
                    
                    if sys_metrics.cpu_usage_percent > thresholds.max_cpu_usage_percent {
                        new_alerts.push(create_alert(
                            AlertType::HighCpuUsage,
                            AlertSeverity::High,
                            format!("High CPU usage: {:.1}%", sys_metrics.cpu_usage_percent),
                            sys_metrics.cpu_usage_percent,
                            thresholds.max_cpu_usage_percent,
                            None,
                        ));
                    }
                    
                    if sys_metrics.memory_usage_percent > thresholds.max_memory_usage_percent {
                        new_alerts.push(create_alert(
                            AlertType::HighMemoryUsage,
                            AlertSeverity::High,
                            format!("High memory usage: {:.1}%", sys_metrics.memory_usage_percent),
                            sys_metrics.memory_usage_percent,
                            thresholds.max_memory_usage_percent,
                            None,
                        ));
                    }
                    
                    // Add new alerts
                    if !new_alerts.is_empty() {
                        let mut alerts_lock = alerts.write().await;
                        alerts_lock.extend(new_alerts);
                    }
                }
                
                // Check quality metrics alerts
                if let Some(qual_metrics) = quality_metrics.read().await.latest() {
                    let mut new_alerts = Vec::new();
                    
                    if qual_metrics.video_quality_score < thresholds.min_quality_score {
                        new_alerts.push(create_alert(
                            AlertType::LowQualityScore,
                            AlertSeverity::Medium,
                            format!("Low video quality: {:.1}", qual_metrics.video_quality_score),
                            qual_metrics.video_quality_score,
                            thresholds.min_quality_score,
                            None,
                        ));
                    }
                    
                    if qual_metrics.frame_drops_per_second > thresholds.max_frame_drops_per_second {
                        new_alerts.push(create_alert(
                            AlertType::FrameDrops,
                            AlertSeverity::Medium,
                            format!("High frame drops: {:.1}/sec", qual_metrics.frame_drops_per_second),
                            qual_metrics.frame_drops_per_second,
                            thresholds.max_frame_drops_per_second,
                            None,
                        ));
                    }
                    
                    // Add new alerts
                    if !new_alerts.is_empty() {
                        let mut alerts_lock = alerts.write().await;
                        alerts_lock.extend(new_alerts);
                    }
                }
            }
        });
    }
    
    async fn check_connection_alerts(&self, metrics: &ConnectionMetrics) {
        let thresholds = self.alert_thresholds.read().await.clone();
        let mut new_alerts = Vec::new();
        
        if metrics.latency_ms > thresholds.max_latency_ms {
            new_alerts.push(create_alert(
                AlertType::HighLatency,
                AlertSeverity::Medium,
                format!("High latency: {:.1}ms", metrics.latency_ms),
                metrics.latency_ms,
                thresholds.max_latency_ms,
                Some(metrics.connection_id.clone()),
            ));
        }
        
        if metrics.packet_loss_percent > thresholds.max_packet_loss_percent {
            new_alerts.push(create_alert(
                AlertType::PacketLoss,
                AlertSeverity::High,
                format!("Packet loss: {:.1}%", metrics.packet_loss_percent),
                metrics.packet_loss_percent,
                thresholds.max_packet_loss_percent,
                Some(metrics.connection_id.clone()),
            ));
        }
        
        if metrics.bandwidth_mbps < thresholds.min_bandwidth_mbps {
            new_alerts.push(create_alert(
                AlertType::LowBandwidth,
                AlertSeverity::Medium,
                format!("Low bandwidth: {:.1} Mbps", metrics.bandwidth_mbps),
                metrics.bandwidth_mbps,
                thresholds.min_bandwidth_mbps,
                Some(metrics.connection_id.clone()),
            ));
        }
        
        if metrics.quality_score < thresholds.min_quality_score {
            new_alerts.push(create_alert(
                AlertType::LowQualityScore,
                AlertSeverity::Medium,
                format!("Low connection quality: {:.1}", metrics.quality_score),
                metrics.quality_score,
                thresholds.min_quality_score,
                Some(metrics.connection_id.clone()),
            ));
        }
        
        if !new_alerts.is_empty() {
            let mut alerts = self.alerts.write().await;
            alerts.extend(new_alerts);
        }
    }
}

fn create_alert(
    alert_type: AlertType,
    severity: AlertSeverity,
    message: String,
    metric_value: f32,
    threshold: f32,
    connection_id: Option<String>,
) -> PerformanceAlert {
    PerformanceAlert {
        id: uuid::Uuid::new_v4().to_string(),
        alert_type,
        severity,
        message,
        metric_value,
        threshold,
        connection_id,
        created_at: chrono::Utc::now(),
        acknowledged: false,
    }
}

async fn collect_system_metrics() -> Result<SystemMetrics> {
    // Simplified system metrics collection
    // In production, would use proper system monitoring libraries
    
    let cpu_usage = get_cpu_usage().await.unwrap_or(0.0);
    let (memory_used, memory_total) = get_memory_usage().await;
    let memory_usage_percent = if memory_total > 0 {
        (memory_used as f32 / memory_total as f32) * 100.0
    } else {
        0.0
    };
    
    Ok(SystemMetrics {
        cpu_usage_percent: cpu_usage,
        memory_usage_percent,
        memory_used_mb: memory_used / 1024 / 1024,
        memory_total_mb: memory_total / 1024 / 1024,
        disk_usage_percent: 0.0, // Would implement
        network_rx_mbps: 0.0,    // Would implement
        network_tx_mbps: 0.0,    // Would implement
        screen_capture_fps: 0.0, // Would get from streaming manager
        encoding_fps: 0.0,       // Would get from streaming manager
        last_updated: chrono::Utc::now(),
    })
}

async fn collect_quality_metrics() -> Result<QualityMetrics> {
    // Simplified quality metrics collection
    // In production, would collect from various subsystems
    
    Ok(QualityMetrics {
        video_quality_score: 85.0, // Would calculate based on various factors
        input_responsiveness_ms: 25.0,
        frame_drops_per_second: 0.5,
        compression_efficiency: 0.75,
        user_satisfaction_score: 90.0,
        last_updated: chrono::Utc::now(),
    })
}

#[cfg(target_os = "macos")]
async fn get_cpu_usage() -> Option<f32> {
    // macOS-specific CPU usage implementation
    // This is a simplified version
    Some(10.0) // Placeholder
}

#[cfg(target_os = "windows")]
async fn get_cpu_usage() -> Option<f32> {
    // Windows-specific CPU usage implementation
    Some(10.0) // Placeholder
}

#[cfg(target_os = "linux")]
async fn get_cpu_usage() -> Option<f32> {
    // Linux-specific CPU usage implementation
    Some(10.0) // Placeholder
}

async fn get_memory_usage() -> (u64, u64) {
    // Simplified memory usage
    // Would use proper system APIs
    (1_000_000_000, 8_000_000_000) // 1GB used, 8GB total
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}