// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod sip;

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;

// SIP State Management
struct SipState {
    initialized: bool,
    registered: bool,
    current_call: Option<String>,
}

impl Default for SipState {
    fn default() -> Self {
        Self {
            initialized: false,
            registered: false,
            current_call: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct SipEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

// Initialize SIP stack
#[tauri::command]
async fn init_sip(
    state: tauri::State<'_, Mutex<SipState>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    println!("Initializing SIP stack...");
    
    // Initialize SIP with rsipstack
    sip::init_pjsip().await?;
    
    let mut sip_state = state.lock().unwrap();
    sip_state.initialized = true;
    
    app_handle.emit_all("sip-event", SipEvent {
        event_type: "initialized".to_string(),
        registered: None,
        state: Some("INITIALIZED".to_string()),
        message: Some("SIP stack initialized".to_string()),
    }).map_err(|e| e.to_string())?;
    
    Ok("SIP stack initialized".to_string())
}

// Register SIP account
#[tauri::command]
async fn register_account(
    server: String,
    user: String,
    password: String,
    state: tauri::State<'_, Mutex<SipState>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    println!("Registering account: {}@{}", user, server);
    
    // Register with rsipstack
    sip::register_account(&server, &user, &password).await?;
    
    // Wait a bit for registration to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let mut sip_state = state.lock().unwrap();
    sip_state.registered = true;
    
    app_handle.emit_all("sip-event", SipEvent {
        event_type: "registration_state".to_string(),
        registered: Some(true),
        state: Some("REGISTERED".to_string()),
        message: Some(format!("Registered as {}@{}", user, server)),
    }).map_err(|e| e.to_string())?;
    
    Ok("Registration successful".to_string())
}

// Make outbound call
#[tauri::command]
async fn make_call(
    number: String,
    state: tauri::State<'_, Mutex<SipState>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    println!("Making call to: {}", number);
    
    // Check registration status
    let is_registered = {
        let sip_state = state.lock().unwrap();
        sip_state.registered
    };
    
    if !is_registered {
        return Err("Not registered".to_string());
    }
    
    // Make call with rsipstack
    sip::make_call(&number).await?;
    
    // Update state
    {
        let mut sip_state = state.lock().unwrap();
        sip_state.current_call = Some(number.clone());
    }
    
    app_handle.emit_all("sip-event", SipEvent {
        event_type: "call_state".to_string(),
        registered: None,
        state: Some("OUTGOING".to_string()),
        message: Some(format!("Calling {}", number)),
    }).map_err(|e| e.to_string())?;
    
    Ok("Call initiated".to_string())
}

// Answer incoming call
#[tauri::command]
async fn answer_call(
    _state: tauri::State<'_, Mutex<SipState>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    println!("Answering call");
    
    // Answer with rsipstack
    sip::answer_call().await?;
    
    app_handle.emit_all("sip-event", SipEvent {
        event_type: "call_state".to_string(),
        registered: None,
        state: Some("ACTIVE".to_string()),
        message: Some("Call answered".to_string()),
    }).map_err(|e| e.to_string())?;
    
    Ok("Call answered".to_string())
}

// Hangup call
#[tauri::command]
async fn hangup_call(
    state: tauri::State<'_, Mutex<SipState>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    println!("Hanging up call");
    
    // Hangup with rsipstack
    sip::hangup_call().await?;
    
    // Update state
    {
        let mut sip_state = state.lock().unwrap();
        sip_state.current_call = None;
    }
    
    app_handle.emit_all("sip-event", SipEvent {
        event_type: "call_state".to_string(),
        registered: None,
        state: Some("REGISTERED".to_string()),
        message: Some("Call ended".to_string()),
    }).map_err(|e| e.to_string())?;
    
    Ok("Call ended".to_string())
}

// Unregister (de-register) from SIP server
#[tauri::command]
async fn unregister() -> Result<String, String> {
    println!("Unregistering from SIP server...");
    
    // Unregister from server
    sip::unregister().await?;
    
    Ok("Unregistered successfully".to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(SipState::default()))
        .invoke_handler(tauri::generate_handler![
            init_sip,
            register_account,
            make_call,
            answer_call,
            hangup_call,
            unregister
        ])
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                // Prevent default close behavior
                api.prevent_close();
                
                println!("App closing, cleaning up SIP...");
                
                let _app_handle = event.window().app_handle();
                
                // Spawn async task to unregister
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = sip::unregister().await {
                        eprintln!("Error during unregister: {}", e);
                    } else {
                        println!("SIP cleanup completed");
                    }
                    sip::shutdown().await;
                    
                    // Now exit the app
                    std::process::exit(0);
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
