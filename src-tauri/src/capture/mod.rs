pub mod screen_capture;

use anyhow::Result;
use log::{debug, error, info};
use screenshots::Screen;
use std::sync::Arc;
use tokio::sync::RwLock;


#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub fps: u32,
    pub quality: u8,
    pub monitor_index: usize,
    pub capture_cursor: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            quality: 80,
            monitor_index: 0,
            capture_cursor: true,
        }
    }
}

pub struct ScreenCaptureManager {
    config: Arc<RwLock<CaptureConfig>>,
    screens: Vec<Screen>,
    is_capturing: Arc<RwLock<bool>>,
}

impl ScreenCaptureManager {
    pub fn new() -> Result<Self> {
        let screens = Screen::all()?;
        
        if screens.is_empty() {
            return Err(anyhow::anyhow!("No screens found"));
        }
        
        info!("Found {} screen(s)", screens.len());
        for (i, screen) in screens.iter().enumerate() {
            debug!("Screen {}: {}x{}", i, screen.display_info.width, screen.display_info.height);
        }
        
        Ok(Self {
            config: Arc::new(RwLock::new(CaptureConfig::default())),
            screens,
            is_capturing: Arc::new(RwLock::new(false)),
        })
    }
    
    pub async fn capture_primary_screen(&self) -> Result<Vec<u8>> {
        let config = self.config.read().await;
        let screen = &self.screens[config.monitor_index.min(self.screens.len() - 1)];
        
        debug!("Capturing screen {} ({}x{})", 
               config.monitor_index, 
               screen.display_info.width, 
               screen.display_info.height);
        
        let image = screen.capture()?;
        
        // Convert to JPEG for transmission
        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        
        // TODO: Implement proper image conversion to JPEG
        // image.save_to(&mut cursor, image::ImageFormat::Jpeg)?;
        
        debug!("Captured screen data: {} bytes", buffer.len());
        Ok(buffer)
    }
    
    pub async fn capture_all_screens(&self) -> Result<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        
        for (i, screen) in self.screens.iter().enumerate() {
            debug!("Capturing screen {}", i);
            match screen.capture() {
                Ok(image) => {
                    let mut buffer = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut buffer);
                    
                    // TODO: Fix image conversion
                    // if let Err(e) = image.save_to(&mut cursor, image::ImageFormat::Jpeg) {
                        // error!("Failed to encode screen {} image: {}", i, e);
                        // continue;
                    // }
                    
                    results.push(buffer);
                }
                Err(e) => {
                    error!("Failed to capture screen {}: {}", i, e);
                }
            }
        }
        
        Ok(results)
    }
    
    pub async fn get_screen_info(&self) -> Vec<ScreenInfo> {
        self.screens
            .iter()
            .enumerate()
            .map(|(i, screen)| ScreenInfo {
                index: i,
                width: screen.display_info.width as u32,
                height: screen.display_info.height as u32,
                scale_factor: screen.display_info.scale_factor as f64,
                is_primary: i == 0, // Assume first screen is primary
            })
            .collect()
    }
    
    pub async fn update_config(&self, new_config: CaptureConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated capture configuration");
        Ok(())
    }
    
    pub async fn start_continuous_capture<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let mut is_capturing = self.is_capturing.write().await;
        if *is_capturing {
            return Err(anyhow::anyhow!("Already capturing"));
        }
        *is_capturing = true;
        
        let config = self.config.clone();
        let screens = self.screens.clone();
        let is_capturing_clone = self.is_capturing.clone();
        
        tokio::spawn(async move {
            info!("Starting continuous screen capture");
            
            while *is_capturing_clone.read().await {
                let config_read = config.read().await;
                let fps = config_read.fps;
                let monitor_index = config_read.monitor_index.min(screens.len() - 1);
                drop(config_read);
                
                let frame_duration = std::time::Duration::from_millis(1000 / fps as u64);
                
                if let Ok(image) = screens[monitor_index].capture() {
                    let mut buffer = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut buffer);
                    
                    // TODO: Fix image conversion
                    // if image.save_to(&mut cursor, image::ImageFormat::Jpeg).is_ok() {
                    if true { // Temporary placeholder
                        callback(buffer);
                    }
                }
                
                tokio::time::sleep(frame_duration).await;
            }
            
            info!("Stopped continuous screen capture");
        });
        
        Ok(())
    }
    
    pub async fn stop_continuous_capture(&self) {
        let mut is_capturing = self.is_capturing.write().await;
        *is_capturing = false;
        info!("Stopping continuous screen capture");
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScreenInfo {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}