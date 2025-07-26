use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct FrameData {
    pub data: Vec<u8>,
    pub timestamp: Instant,
    pub frame_id: u64,
    pub size_bytes: usize,
}

pub struct FrameBuffer {
    buffer: Arc<RwLock<VecDeque<FrameData>>>,
    max_size: usize,
    frame_counter: Arc<RwLock<u64>>,
    total_bytes: Arc<RwLock<usize>>,
}

impl FrameBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
            frame_counter: Arc::new(RwLock::new(0)),
            total_bytes: Arc::new(RwLock::new(0)),
        }
    }
    
    pub async fn add_frame(&self, data: Vec<u8>) {
        let frame_id = {
            let mut counter = self.frame_counter.write().await;
            *counter += 1;
            *counter
        };
        
        let frame_data = FrameData {
            size_bytes: data.len(),
            data,
            timestamp: Instant::now(),
            frame_id,
        };
        
        let mut buffer = self.buffer.write().await;
        
        // Remove oldest frame if buffer is full
        if buffer.len() >= self.max_size {
            if let Some(old_frame) = buffer.pop_front() {
                let mut total_bytes = self.total_bytes.write().await;
                *total_bytes = total_bytes.saturating_sub(old_frame.size_bytes);
            }
        }
        
        // Add new frame
        {
            let mut total_bytes = self.total_bytes.write().await;
            *total_bytes += frame_data.size_bytes;
        }
        
        buffer.push_back(frame_data);
    }
    
    pub async fn get_latest_frame(&self) -> Option<Vec<u8>> {
        let buffer = self.buffer.read().await;
        buffer.back().map(|frame| frame.data.clone())
    }
    
    pub async fn get_frame_by_id(&self, frame_id: u64) -> Option<Vec<u8>> {
        let buffer = self.buffer.read().await;
        buffer.iter()
            .find(|frame| frame.frame_id == frame_id)
            .map(|frame| frame.data.clone())
    }
    
    pub async fn get_frames_since(&self, since: Instant) -> Vec<FrameData> {
        let buffer = self.buffer.read().await;
        buffer.iter()
            .filter(|frame| frame.timestamp >= since)
            .cloned()
            .collect()
    }
    
    pub async fn get_buffer_size(&self) -> usize {
        let buffer = self.buffer.read().await;
        buffer.len()
    }
    
    pub async fn get_total_bytes(&self) -> usize {
        *self.total_bytes.read().await
    }
    
    pub async fn clear(&self) {
        let mut buffer = self.buffer.write().await;
        buffer.clear();
        
        let mut total_bytes = self.total_bytes.write().await;
        *total_bytes = 0;
    }
    
    pub async fn get_buffer_stats(&self) -> FrameBufferStats {
        let buffer = self.buffer.read().await;
        let total_bytes = *self.total_bytes.read().await;
        let frame_count = *self.frame_counter.read().await;
        
        let oldest_timestamp = buffer.front().map(|f| f.timestamp);
        let newest_timestamp = buffer.back().map(|f| f.timestamp);
        
        let buffer_duration = if let (Some(oldest), Some(newest)) = (oldest_timestamp, newest_timestamp) {
            newest.duration_since(oldest)
        } else {
            Duration::from_secs(0)
        };
        
        let average_frame_size = if !buffer.is_empty() {
            total_bytes / buffer.len()
        } else {
            0
        };
        
        FrameBufferStats {
            current_frames: buffer.len(),
            max_frames: self.max_size,
            total_bytes,
            average_frame_size_bytes: average_frame_size,
            buffer_duration_ms: buffer_duration.as_millis() as u64,
            total_frames_processed: frame_count,
        }
    }
    
    pub async fn cleanup_old_frames(&self, max_age: Duration) {
        let cutoff_time = Instant::now() - max_age;
        let mut buffer = self.buffer.write().await;
        let mut total_bytes = self.total_bytes.write().await;
        
        while let Some(frame) = buffer.front() {
            if frame.timestamp < cutoff_time {
                let old_frame = buffer.pop_front().unwrap();
                *total_bytes = total_bytes.saturating_sub(old_frame.size_bytes);
            } else {
                break;
            }
        }
    }
    
    pub async fn set_max_size(&self, new_max_size: usize) {
        let mut buffer = self.buffer.write().await;
        let mut total_bytes = self.total_bytes.write().await;
        
        // Remove excess frames if new size is smaller
        while buffer.len() > new_max_size {
            if let Some(old_frame) = buffer.pop_front() {
                *total_bytes = total_bytes.saturating_sub(old_frame.size_bytes);
            }
        }
        
        // Update max size
        // Note: We can't directly change the VecDeque capacity, but this affects future additions
    }
    
    pub async fn get_frame_rate(&self, duration: Duration) -> f32 {
        let since = Instant::now() - duration;
        let buffer = self.buffer.read().await;
        
        let recent_frames = buffer.iter()
            .filter(|frame| frame.timestamp >= since)
            .count();
        
        recent_frames as f32 / duration.as_secs_f32()
    }
    
    pub async fn get_bandwidth_usage(&self, duration: Duration) -> f32 {
        let since = Instant::now() - duration;
        let buffer = self.buffer.read().await;
        
        let recent_bytes: usize = buffer.iter()
            .filter(|frame| frame.timestamp >= since)
            .map(|frame| frame.size_bytes)
            .sum();
        
        // Return bandwidth in Mbps
        (recent_bytes as f32 * 8.0) / (duration.as_secs_f32() * 1_000_000.0)
    }
}

#[derive(Debug, Clone)]
pub struct FrameBufferStats {
    pub current_frames: usize,
    pub max_frames: usize,
    pub total_bytes: usize,
    pub average_frame_size_bytes: usize,
    pub buffer_duration_ms: u64,
    pub total_frames_processed: u64,
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new(3)
    }
}