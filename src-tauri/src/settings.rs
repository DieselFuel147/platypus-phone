use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipCredentials {
    pub server: String,
    pub username: String,
    #[serde(default)]
    pub password_encrypted: String,
}

impl Default for SipCredentials {
    fn default() -> Self {
        Self {
            server: String::new(),
            username: String::new(),
            password_encrypted: String::new(),
        }
    }
}

/// Simple XOR-based obfuscation for password storage
/// Note: This is NOT cryptographically secure, but provides basic obfuscation
/// to prevent casual viewing of the password in the config file
fn obfuscate_password(password: &str) -> String {
    const KEY: &[u8] = b"PlatypusPhoneKey2024"; // Simple key for XOR
    
    let bytes: Vec<u8> = password
        .bytes()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    
    // Encode as hex string
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

fn deobfuscate_password(encrypted: &str) -> Result<String, String> {
    const KEY: &[u8] = b"PlatypusPhoneKey2024";
    
    // Decode from hex
    let bytes: Result<Vec<u8>, _> = (0..encrypted.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encrypted[i..i + 2], 16))
        .collect();
    
    let bytes = bytes.map_err(|e| format!("Failed to decode password: {}", e))?;
    
    // XOR decrypt
    let decrypted: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    
    String::from_utf8(decrypted).map_err(|e| format!("Invalid UTF-8: {}", e))
}

/// Get the path to the settings file
fn get_settings_path() -> Result<PathBuf, String> {
    // Get the app data directory
    let app_dir = tauri::api::path::app_data_dir(&tauri::Config::default())
        .ok_or_else(|| "Failed to get app data directory".to_string())?;
    
    // Create directory if it doesn't exist
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app directory: {}", e))?;
    
    Ok(app_dir.join("settings.json"))
}

/// Save SIP credentials to disk
pub fn save_credentials(server: &str, username: &str, password: &str) -> Result<(), String> {
    let credentials = SipCredentials {
        server: server.to_string(),
        username: username.to_string(),
        password_encrypted: obfuscate_password(password),
    };
    
    let settings_path = get_settings_path()?;
    let json = serde_json::to_string_pretty(&credentials)
        .map_err(|e| format!("Failed to serialize credentials: {}", e))?;
    
    fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;
    
    tracing::info!("Saved credentials to: {}", settings_path.display());
    Ok(())
}

/// Load SIP credentials from disk
pub fn load_credentials() -> Result<(String, String, String), String> {
    let settings_path = get_settings_path()?;
    
    if !settings_path.exists() {
        return Ok((String::new(), String::new(), String::new()));
    }
    
    let json = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings file: {}", e))?;
    
    let credentials: SipCredentials = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse settings file: {}", e))?;
    
    let password = if credentials.password_encrypted.is_empty() {
        String::new()
    } else {
        deobfuscate_password(&credentials.password_encrypted)?
    };
    
    tracing::info!("Loaded credentials from: {}", settings_path.display());
    Ok((credentials.server, credentials.username, password))
}

/// Clear saved credentials
pub fn clear_credentials() -> Result<(), String> {
    let settings_path = get_settings_path()?;
    
    if settings_path.exists() {
        fs::remove_file(&settings_path)
            .map_err(|e| format!("Failed to delete settings file: {}", e))?;
        tracing::info!("Cleared credentials");
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_obfuscation() {
        let password = "MySecretPassword123!";
        let encrypted = obfuscate_password(password);
        
        // Should not be the same as original
        assert_ne!(encrypted, password);
        
        // Should be able to decrypt
        let decrypted = deobfuscate_password(&encrypted).unwrap();
        assert_eq!(decrypted, password);
    }

    #[test]
    fn test_empty_password() {
        let password = "";
        let encrypted = obfuscate_password(password);
        let decrypted = deobfuscate_password(&encrypted).unwrap();
        assert_eq!(decrypted, password);
    }
}
