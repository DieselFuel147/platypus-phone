use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub server: String,
    pub username: String,
    #[serde(default)]
    pub password_encrypted: String,
    #[serde(default)]
    pub audio_input_device: String,
    #[serde(default)]
    pub audio_output_device: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            server: String::new(),
            username: String::new(),
            password_encrypted: String::new(),
            audio_input_device: String::new(),
            audio_output_device: String::new(),
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

/// Load all settings from disk
fn load_settings() -> Result<AppSettings, String> {
    let settings_path = get_settings_path()?;
    
    if !settings_path.exists() {
        return Ok(AppSettings::default());
    }
    
    let json = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings file: {}", e))?;
    
    let settings: AppSettings = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse settings file: {}", e))?;
    
    tracing::info!("Loaded settings from: {}", settings_path.display());
    Ok(settings)
}

/// Save all settings to disk
fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let settings_path = get_settings_path()?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    
    fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;
    
    tracing::info!("Saved settings to: {}", settings_path.display());
    Ok(())
}

/// Save SIP credentials to disk
pub fn save_credentials(server: &str, username: &str, password: &str) -> Result<(), String> {
    let mut settings = load_settings()?;
    
    settings.server = server.to_string();
    settings.username = username.to_string();
    settings.password_encrypted = obfuscate_password(password);
    
    save_settings(&settings)
}

/// Load SIP credentials from disk
pub fn load_credentials() -> Result<(String, String, String), String> {
    let settings = load_settings()?;
    
    let password = if settings.password_encrypted.is_empty() {
        String::new()
    } else {
        deobfuscate_password(&settings.password_encrypted)?
    };
    
    Ok((settings.server, settings.username, password))
}

/// Save audio device preferences
pub fn save_audio_devices(input_device: &str, output_device: &str) -> Result<(), String> {
    let mut settings = load_settings()?;
    
    settings.audio_input_device = input_device.to_string();
    settings.audio_output_device = output_device.to_string();
    
    save_settings(&settings)
}

/// Load audio device preferences
pub fn load_audio_devices() -> Result<(String, String), String> {
    let settings = load_settings()?;
    Ok((settings.audio_input_device, settings.audio_output_device))
}

/// Clear all saved settings
pub fn clear_settings() -> Result<(), String> {
    let settings_path = get_settings_path()?;
    
    if settings_path.exists() {
        fs::remove_file(&settings_path)
            .map_err(|e| format!("Failed to delete settings file: {}", e))?;
        tracing::info!("Cleared all settings");
    }
    
    Ok(())
}

// Keep old function name for backward compatibility
pub fn clear_credentials() -> Result<(), String> {
    clear_settings()
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
