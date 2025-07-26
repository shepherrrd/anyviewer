use anyhow::Result;
use config::{Config, File, FileFormat};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub client: ClientConfig,
    pub security: SecurityConfig,
    pub capture: CaptureConfig,
    pub codec: CodecConfig,
    pub network: NetworkConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub max_connections: usize,
    pub enable_discovery: bool,
    pub discovery_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub auto_connect: bool,
    pub preferred_quality: String,
    pub fullscreen_on_connect: bool,
    pub show_connection_info: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_encryption: bool,
    pub require_authentication: bool,
    pub session_timeout_minutes: u32,
    pub max_failed_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub fps: u32,
    pub quality: u8,
    pub capture_cursor: bool,
    pub monitor_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecConfig {
    pub format: String,
    pub quality: u8,
    pub enable_hardware_acceleration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub buffer_size: usize,
    pub connection_timeout_seconds: u32,
    pub heartbeat_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                port: 7878,
                max_connections: 10,
                enable_discovery: true,
                discovery_port: 7879,
            },
            client: ClientConfig {
                auto_connect: false,
                preferred_quality: "high".to_string(),
                fullscreen_on_connect: false,
                show_connection_info: true,
            },
            security: SecurityConfig {
                enable_encryption: true,
                require_authentication: true,
                session_timeout_minutes: 60,
                max_failed_attempts: 5,
            },
            capture: CaptureConfig {
                fps: 30,
                quality: 80,
                capture_cursor: true,
                monitor_index: 0,
            },
            codec: CodecConfig {
                format: "jpeg".to_string(),
                quality: 80,
                enable_hardware_acceleration: true,
            },
            network: NetworkConfig {
                buffer_size: 65536,
                connection_timeout_seconds: 30,
                heartbeat_interval_seconds: 30,
            },
            ui: UiConfig {
                theme: "system".to_string(),
                window_width: 1200,
                window_height: 800,
                minimize_to_tray: true,
                start_minimized: false,
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            info!("Config file not found, creating default configuration");
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }
        
        info!("Loading configuration from: {}", config_path.display());
        
        let settings = Config::builder()
            .add_source(File::from(config_path).format(FileFormat::Toml))
            .build()?;
        
        let config: AppConfig = settings.try_deserialize()?;
        
        info!("Configuration loaded successfully");
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        
        std::fs::write(&config_path, toml_string)?;
        
        info!("Configuration saved to: {}", config_path.display());
        Ok(())
    }
    
    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        
        Ok(config_dir.join("anyviewer").join("config.toml"))
    }
    
    pub fn get_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?;
        
        let anyviewer_dir = data_dir.join("anyviewer");
        
        if !anyviewer_dir.exists() {
            std::fs::create_dir_all(&anyviewer_dir)?;
        }
        
        Ok(anyviewer_dir)
    }
    
    pub fn get_log_dir() -> Result<PathBuf> {
        let log_dir = Self::get_data_dir()?.join("logs");
        
        if !log_dir.exists() {
            std::fs::create_dir_all(&log_dir)?;
        }
        
        Ok(log_dir)
    }
    
    pub fn validate(&self) -> Result<()> {
        // Validate server config
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("Server port cannot be 0"));
        }
        
        if self.server.max_connections == 0 {
            return Err(anyhow::anyhow!("Max connections must be greater than 0"));
        }
        
        if self.server.port == self.server.discovery_port {
            return Err(anyhow::anyhow!("Server port and discovery port cannot be the same"));
        }
        
        // Validate capture config
        if self.capture.fps == 0 || self.capture.fps > 120 {
            return Err(anyhow::anyhow!("FPS must be between 1 and 120"));
        }
        
        if self.capture.quality == 0 || self.capture.quality > 100 {
            return Err(anyhow::anyhow!("Quality must be between 1 and 100"));
        }
        
        // Validate codec config
        if self.codec.quality == 0 || self.codec.quality > 100 {
            return Err(anyhow::anyhow!("Codec quality must be between 1 and 100"));
        }
        
        let valid_formats = ["jpeg", "png", "webp"];
        if !valid_formats.contains(&self.codec.format.as_str()) {
            return Err(anyhow::anyhow!("Invalid codec format: {}", self.codec.format));
        }
        
        // Validate security config
        if self.security.session_timeout_minutes == 0 {
            return Err(anyhow::anyhow!("Session timeout must be greater than 0"));
        }
        
        // Validate network config
        if self.network.connection_timeout_seconds == 0 {
            return Err(anyhow::anyhow!("Connection timeout must be greater than 0"));
        }
        
        if self.network.heartbeat_interval_seconds == 0 {
            return Err(anyhow::anyhow!("Heartbeat interval must be greater than 0"));
        }
        
        // Validate UI config
        if self.ui.window_width < 400 || self.ui.window_height < 300 {
            return Err(anyhow::anyhow!("Window size must be at least 400x300"));
        }
        
        let valid_themes = ["light", "dark", "system"];
        if !valid_themes.contains(&self.ui.theme.as_str()) {
            return Err(anyhow::anyhow!("Invalid theme: {}", self.ui.theme));
        }
        
        info!("Configuration validation passed");
        Ok(())
    }
    
    pub fn reload(&mut self) -> Result<()> {
        *self = Self::load()?;
        info!("Configuration reloaded");
        Ok(())
    }
    
    pub fn reset_to_defaults(&mut self) -> Result<()> {
        *self = Self::default();
        self.save()?;
        info!("Configuration reset to defaults");
        Ok(())
    }
    
    pub fn export_to_file(&self, path: &str) -> Result<()> {
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        
        std::fs::write(path, toml_string)?;
        info!("Configuration exported to: {}", path);
        Ok(())
    }
    
    pub fn import_from_file(&mut self, path: &str) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        
        *self = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;
        
        self.validate()?;
        self.save()?;
        
        info!("Configuration imported from: {}", path);
        Ok(())
    }
}