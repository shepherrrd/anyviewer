use anyhow::Result;
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionId {
    pub id: String,
    pub numeric_id: u32,
    pub formatted_id: String, // XXX XXX XXX format
}

pub struct IdGenerator {
    used_ids: Arc<RwLock<HashMap<u32, bool>>>,
    id_to_session: Arc<RwLock<HashMap<String, String>>>, // formatted_id -> session_id
}

impl IdGenerator {
    pub fn new() -> Self {
        Self {
            used_ids: Arc::new(RwLock::new(HashMap::new())),
            id_to_session: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Generate a unique 7-digit ID (e.g., 1704456)
    pub fn generate_connection_id(&self) -> Result<ConnectionId> {
        let mut rng = thread_rng();
        let mut attempts = 0;
        const MAX_ATTEMPTS: usize = 1000;
        
        loop {
            if attempts >= MAX_ATTEMPTS {
                return Err(anyhow::anyhow!("Failed to generate unique ID after {} attempts", MAX_ATTEMPTS));
            }
            
            // Generate a random 7-digit number (1,000,000 to 9,999,999)
            let numeric_id: u32 = rng.gen_range(1_000_000..10_000_000);
            
            // Check if this ID is already in use
            {
                let used_ids = self.used_ids.read().unwrap();
                if used_ids.contains_key(&numeric_id) {
                    attempts += 1;
                    continue;
                }
            }
            
            // Format the ID as simple 7-digit number 
            let formatted_id = numeric_id.to_string();
            let id_string = numeric_id.to_string();
            
            // Reserve this ID
            {
                let mut used_ids = self.used_ids.write().unwrap();
                used_ids.insert(numeric_id, true);
            }
            
            let connection_id = ConnectionId {
                id: id_string,
                numeric_id,
                formatted_id,
            };
            
            log::info!("Generated connection ID: {}", connection_id.formatted_id);
            return Ok(connection_id);
        }
    }
    
    /// Parse a formatted ID back to numeric form
    pub fn parse_connection_id(&self, formatted_id: &str) -> Result<u32> {
        // Remove any spaces and parse
        let cleaned = formatted_id.replace(" ", "").replace("-", "");
        
        if cleaned.len() != 7 {
            return Err(anyhow::anyhow!("Invalid ID format. Expected 7 digits, got {}", cleaned.len()));
        }
        
        cleaned.parse::<u32>()
            .map_err(|e| anyhow::anyhow!("Failed to parse ID: {}", e))
    }
    
    /// Validate if an ID format is correct
    pub fn validate_id_format(&self, id: &str) -> bool {
        // Accept 7-digit format: "1704456"
        let cleaned = id.replace(" ", "").replace("-", "");
        
        if cleaned.len() != 7 {
            return false;
        }
        
        cleaned.chars().all(|c| c.is_ascii_digit())
    }
    
    /// Register a session with a connection ID
    pub fn register_session(&self, connection_id: &ConnectionId, session_id: String) -> Result<()> {
        let mut id_to_session = self.id_to_session.write().unwrap();
        id_to_session.insert(connection_id.formatted_id.clone(), session_id);
        log::debug!("Registered session for ID: {}", connection_id.formatted_id);
        Ok(())
    }
    
    /// Get session ID by connection ID
    pub fn get_session_by_id(&self, formatted_id: &str) -> Option<String> {
        let id_to_session = self.id_to_session.read().unwrap();
        id_to_session.get(formatted_id).cloned()
    }
    
    /// Release a connection ID
    pub fn release_id(&self, connection_id: &ConnectionId) -> Result<()> {
        {
            let mut used_ids = self.used_ids.write().unwrap();
            used_ids.remove(&connection_id.numeric_id);
        }
        
        {
            let mut id_to_session = self.id_to_session.write().unwrap();
            id_to_session.remove(&connection_id.formatted_id);
        }
        
        log::info!("Released connection ID: {}", connection_id.formatted_id);
        Ok(())
    }
    
    /// Get all active connection IDs
    pub fn get_active_ids(&self) -> Vec<u32> {
        let used_ids = self.used_ids.read().unwrap();
        used_ids.keys().cloned().collect()
    }
    
    /// Check if an ID is currently in use
    pub fn is_id_in_use(&self, numeric_id: u32) -> bool {
        let used_ids = self.used_ids.read().unwrap();
        used_ids.contains_key(&numeric_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_id_generation() {
        let generator = IdGenerator::new();
        let id = generator.generate_connection_id().unwrap();
        
        assert_eq!(id.id.len(), 7);
        assert!(id.numeric_id >= 1_000_000);
        assert!(id.numeric_id < 10_000_000);
        assert_eq!(id.formatted_id.len(), 7); // "1704456" = 7 chars
    }
    
    #[test]
    fn test_id_parsing() {
        let generator = IdGenerator::new();
        
        assert_eq!(generator.parse_connection_id("1704456").unwrap(), 1704456);
        assert_eq!(generator.parse_connection_id("1234567").unwrap(), 1234567);
    }
    
    #[test]
    fn test_id_validation() {
        let generator = IdGenerator::new();
        
        assert!(generator.validate_id_format("1704456"));
        assert!(generator.validate_id_format("1234567"));
        assert!(!generator.validate_id_format("123456")); // Too short
        assert!(!generator.validate_id_format("12345678")); // Too long
        assert!(!generator.validate_id_format("170445a")); // Invalid character
    }
    
    #[test]
    fn test_unique_id_generation() {
        let generator = IdGenerator::new();
        let id1 = generator.generate_connection_id().unwrap();
        let id2 = generator.generate_connection_id().unwrap();
        
        assert_ne!(id1.numeric_id, id2.numeric_id);
        assert_ne!(id1.formatted_id, id2.formatted_id);
    }
}