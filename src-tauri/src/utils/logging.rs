use anyhow::Result;
use log::LevelFilter;
use std::io::Write;
use std::path::PathBuf;

pub struct LoggingConfig {
    pub level: LevelFilter,
    pub log_to_file: bool,
    pub log_to_console: bool,
    pub log_file_path: Option<PathBuf>,
    pub max_file_size: u64,
    pub max_files: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LevelFilter::Info,
            log_to_file: true,
            log_to_console: true,
            log_file_path: None,
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 5,
        }
    }
}

pub fn init_logging(config: LoggingConfig) -> Result<()> {
    let mut builder = env_logger::Builder::new();
    
    // Set log level
    builder.filter_level(config.level);
    
    // Configure format
    builder.format(|buf, record| {
        writeln!(
            buf,
            "{} [{}] [{}:{}] - {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level(),
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.args()
        )
    });
    
    // Configure target
    if config.log_to_console && config.log_to_file {
        builder.target(env_logger::Target::Stdout);
    } else if config.log_to_console {
        builder.target(env_logger::Target::Stdout);
    } else if config.log_to_file {
        builder.target(env_logger::Target::Stdout); // env_logger doesn't directly support file output
    }
    
    builder.init();
    
    // If logging to file is enabled, set up file logging
    if config.log_to_file {
        setup_file_logging(config)?;
    }
    
    log::info!("Logging initialized");
    Ok(())
}

fn setup_file_logging(config: LoggingConfig) -> Result<()> {
    let log_file_path = if let Some(path) = config.log_file_path {
        path
    } else {
        let log_dir = crate::config::AppConfig::get_log_dir()?;
        log_dir.join("anyviewer.log")
    };
    
    // Create parent directories
    if let Some(parent) = log_file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Rotate logs if file is too large
    if log_file_path.exists() {
        let metadata = std::fs::metadata(&log_file_path)?;
        if metadata.len() > config.max_file_size {
            rotate_log_files(&log_file_path, config.max_files)?;
        }
    }
    
    log::info!("File logging enabled: {}", log_file_path.display());
    Ok(())
}

fn rotate_log_files(log_file_path: &PathBuf, max_files: usize) -> Result<()> {
    let base_name = log_file_path.file_stem().unwrap_or_default();
    let extension = log_file_path.extension().unwrap_or_default();
    let parent = log_file_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    
    // Rotate existing files
    for i in (1..max_files).rev() {
        let old_file = parent.join(format!(
            "{}.{}.{}",
            base_name.to_string_lossy(),
            i,
            extension.to_string_lossy()
        ));
        let new_file = parent.join(format!(
            "{}.{}.{}",
            base_name.to_string_lossy(),
            i + 1,
            extension.to_string_lossy()
        ));
        
        if old_file.exists() {
            std::fs::rename(old_file, new_file)?;
        }
    }
    
    // Move current log to .1
    let first_backup = parent.join(format!(
        "{}.1.{}",
        base_name.to_string_lossy(),
        extension.to_string_lossy()
    ));
    
    if log_file_path.exists() {
        std::fs::rename(log_file_path, first_backup)?;
    }
    
    log::info!("Log files rotated");
    Ok(())
}

pub fn log_system_info() {
    log::info!("=== AnyViewer System Information ===");
    log::info!("Version: {}", env!("CARGO_PKG_VERSION"));
    log::info!("OS: {}", std::env::consts::OS);
    log::info!("Architecture: {}", std::env::consts::ARCH);
    log::info!("CPU cores: {}", num_cpus::get());
    
    if let Ok(hostname) = gethostname::gethostname().into_string() {
        log::info!("Hostname: {}", hostname);
    }
    
    if let Ok(username) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        log::info!("Username: {}", username);
    }
    
    log::info!("Working directory: {:?}", std::env::current_dir());
    log::info!("======================================");
}

pub struct LogCapture {
    entries: Vec<LogEntry>,
    max_entries: usize,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: String,
    pub message: String,
    pub module: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl LogCapture {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }
    
    pub fn add_entry(&mut self, entry: LogEntry) {
        self.entries.push(entry);
        
        // Keep only the most recent entries
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }
    
    pub fn get_entries(&self) -> &[LogEntry] {
        &self.entries
    }
    
    pub fn get_entries_by_level(&self, level: &str) -> Vec<&LogEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.level == level)
            .collect()
    }
    
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    
    pub fn export_to_string(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                format!(
                    "{} [{}] {} - {}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                    entry.level,
                    entry.module.as_deref().unwrap_or("unknown"),
                    entry.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = self.export_to_string();
        std::fs::write(path, content)?;
        Ok(())
    }
}

// Performance logging utilities
pub fn log_performance<F, R>(operation_name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let result = f();
    let duration = start.elapsed();
    
    log::debug!(
        "Performance: {} took {:.2}ms",
        operation_name,
        duration.as_millis()
    );
    
    result
}

pub async fn log_async_performance<F, Fut, R>(operation_name: &str, f: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let start = std::time::Instant::now();
    let result = f().await;
    let duration = start.elapsed();
    
    log::debug!(
        "Async Performance: {} took {:.2}ms",
        operation_name,
        duration.as_millis()
    );
    
    result
}

#[macro_export]
macro_rules! perf_log {
    ($operation:expr, $block:block) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        log::debug!("Performance: {} took {:.2}ms", $operation, duration.as_millis());
        result
    }};
}

#[macro_export]
macro_rules! debug_time {
    ($($arg:tt)*) => {
        log::debug!("[{}] {}", chrono::Utc::now().format("%H:%M:%S%.3f"), format!($($arg)*));
    };
}