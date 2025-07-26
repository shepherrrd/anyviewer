use anyhow::Result;
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
use log::{info, error, debug, warn};
use rand::RngCore;
use rsa::{RsaPrivateKey, RsaPublicKey, Pkcs1v15Encrypt, pkcs1::EncodeRsaPublicKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_encryption: bool,
    pub key_size: usize,
    pub session_timeout: u64, // seconds
    pub max_failed_attempts: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_encryption: true,
            key_size: 2048,
            session_timeout: 3600, // 1 hour
            max_failed_attempts: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionKey {
    pub id: String,
    pub aes_key: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthAttempt {
    pub client_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
}

pub struct SecurityManager {
    config: Arc<RwLock<SecurityConfig>>,
    rsa_private_key: RsaPrivateKey,
    rsa_public_key: RsaPublicKey,
    session_keys: Arc<RwLock<HashMap<String, SessionKey>>>,
    auth_attempts: Arc<RwLock<Vec<AuthAttempt>>>,
}

impl SecurityManager {
    pub fn new() -> Result<Self> {
        info!("Initializing security manager");
        
        let config = SecurityConfig::default();
        
        // Generate RSA key pair
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, config.key_size)?;
        let public_key = RsaPublicKey::from(&private_key);
        
        info!("Generated RSA key pair (size: {})", config.key_size);
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            rsa_private_key: private_key,
            rsa_public_key: public_key,
            session_keys: Arc::new(RwLock::new(HashMap::new())),
            auth_attempts: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    pub fn get_public_key(&self) -> Result<String> {
        let public_key_pem = self.rsa_public_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)?;
        Ok(public_key_pem)
    }
    
    pub async fn create_session_key(&self, client_id: &str) -> Result<String> {
        debug!("Creating session key for client: {}", client_id);
        
        // Generate AES-256 key
        let mut aes_key = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut aes_key);
        
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_key = SessionKey {
            id: session_id.clone(),
            aes_key,
            created_at: chrono::Utc::now(),
            last_used: chrono::Utc::now(),
        };
        
        self.session_keys.write().await.insert(session_id.clone(), session_key);
        
        info!("Created session key for client {}: {}", client_id, session_id);
        Ok(session_id)
    }
    
    pub async fn encrypt_data(&self, session_id: &str, data: &[u8]) -> Result<Vec<u8>> {
        let session_keys = self.session_keys.read().await;
        let session_key = session_keys.get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session key not found: {}", session_id))?;
        
        let key = Key::<Aes256Gcm>::from_slice(&session_key.aes_key);
        let cipher = Aes256Gcm::new(key);
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let encrypted = cipher.encrypt(nonce, data)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
        // Prepend nonce to encrypted data
        let mut result = nonce_bytes.to_vec();
        result.extend(encrypted);
        
        debug!("Encrypted {} bytes to {} bytes", data.len(), result.len());
        Ok(result)
    }
    
    pub async fn decrypt_data(&self, session_id: &str, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            return Err(anyhow::anyhow!("Invalid encrypted data length"));
        }
        
        let session_keys = self.session_keys.read().await;
        let session_key = session_keys.get(session_id)
            .ok_or_else(|| anyhow::anyhow!("Session key not found: {}", session_id))?;
        
        let key = Key::<Aes256Gcm>::from_slice(&session_key.aes_key);
        let cipher = Aes256Gcm::new(key);
        
        // Extract nonce and ciphertext
        let nonce = Nonce::from_slice(&encrypted_data[..12]);
        let ciphertext = &encrypted_data[12..];
        
        let decrypted = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
        
        debug!("Decrypted {} bytes to {} bytes", encrypted_data.len(), decrypted.len());
        Ok(decrypted)
    }
    
    pub fn encrypt_with_rsa(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut rng = rand::thread_rng();
        let encrypted = self.rsa_public_key.encrypt(&mut rng, Pkcs1v15Encrypt, data)?;
        Ok(encrypted)
    }
    
    pub fn decrypt_with_rsa(&self, encrypted_data: &[u8]) -> Result<Vec<u8>> {
        let decrypted = self.rsa_private_key.decrypt(Pkcs1v15Encrypt, encrypted_data)?;
        Ok(decrypted)
    }
    
    pub async fn authenticate_client(&self, client_id: &str, credentials: &ClientCredentials) -> Result<bool> {
        debug!("Authenticating client: {}", client_id);
        
        // Check for rate limiting
        if self.is_rate_limited(client_id).await {
            error!("Rate limit exceeded for client: {}", client_id);
            self.record_auth_attempt(client_id, false).await;
            return Ok(false);
        }
        
        // Simple authentication logic - in a real app, you'd validate against a database
        let is_valid = match credentials {
            ClientCredentials::Password { username, password } => {
                // For demo purposes, accept any non-empty credentials
                !username.is_empty() && !password.is_empty()
            }
            ClientCredentials::Token { token } => {
                // For demo purposes, accept any non-empty token
                !token.is_empty()
            }
        };
        
        self.record_auth_attempt(client_id, is_valid).await;
        
        if is_valid {
            info!("Client {} authenticated successfully", client_id);
        } else {
            warn!("Authentication failed for client: {}", client_id);
        }
        
        Ok(is_valid)
    }
    
    async fn is_rate_limited(&self, client_id: &str) -> bool {
        let attempts = self.auth_attempts.read().await;
        let config = self.config.read().await;
        
        let recent_failed_attempts = attempts
            .iter()
            .filter(|attempt| {
                attempt.client_id == client_id &&
                !attempt.success &&
                attempt.timestamp > chrono::Utc::now() - chrono::Duration::minutes(15)
            })
            .count();
        
        recent_failed_attempts >= config.max_failed_attempts as usize
    }
    
    async fn record_auth_attempt(&self, client_id: &str, success: bool) {
        let attempt = AuthAttempt {
            client_id: client_id.to_string(),
            timestamp: chrono::Utc::now(),
            success,
        };
        
        self.auth_attempts.write().await.push(attempt);
        
        // Clean up old attempts (keep only last 1000)
        let mut attempts = self.auth_attempts.write().await;
        if attempts.len() > 1000 {
            let drain_count = attempts.len() - 1000;
            attempts.drain(..drain_count);
        }
    }
    
    pub async fn cleanup_expired_sessions(&self) -> Result<()> {
        let config = self.config.read().await;
        let timeout = chrono::Duration::seconds(config.session_timeout as i64);
        let cutoff_time = chrono::Utc::now() - timeout;
        drop(config);
        
        let mut session_keys = self.session_keys.write().await;
        let initial_count = session_keys.len();
        
        session_keys.retain(|_, session| session.last_used > cutoff_time);
        
        let removed_count = initial_count - session_keys.len();
        if removed_count > 0 {
            info!("Cleaned up {} expired sessions", removed_count);
        }
        
        Ok(())
    }
    
    pub async fn update_session_activity(&self, session_id: &str) -> Result<()> {
        let mut session_keys = self.session_keys.write().await;
        if let Some(session) = session_keys.get_mut(session_id) {
            session.last_used = chrono::Utc::now();
        }
        Ok(())
    }
    
    pub async fn revoke_session(&self, session_id: &str) -> Result<()> {
        let mut session_keys = self.session_keys.write().await;
        if session_keys.remove(session_id).is_some() {
            info!("Revoked session: {}", session_id);
        }
        Ok(())
    }
    
    pub async fn get_security_stats(&self) -> SecurityStats {
        let session_keys = self.session_keys.read().await;
        let auth_attempts = self.auth_attempts.read().await;
        
        let successful_auths = auth_attempts.iter().filter(|a| a.success).count();
        let failed_auths = auth_attempts.iter().filter(|a| !a.success).count();
        
        SecurityStats {
            active_sessions: session_keys.len(),
            total_auth_attempts: auth_attempts.len(),
            successful_authentications: successful_auths,
            failed_authentications: failed_auths,
            encryption_enabled: self.config.read().await.enable_encryption,
        }
    }
    
    pub async fn update_config(&self, new_config: SecurityConfig) -> Result<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Updated security configuration");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientCredentials {
    Password { username: String, password: String },
    Token { token: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityStats {
    pub active_sessions: usize,
    pub total_auth_attempts: usize,
    pub successful_authentications: usize,
    pub failed_authentications: usize,
    pub encryption_enabled: bool,
}