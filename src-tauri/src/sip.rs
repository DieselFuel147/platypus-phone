use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::UdpSocket;
use md5::compute as md5_compute;
use crate::rtp::{RtpSession, g711, parse_sdp};
use crate::audio::AudioManager;

// Dialog state for active calls
#[derive(Clone, Debug)]
pub struct Dialog {
    call_id: String,
    from_tag: String,
    to_tag: Option<String>,
    cseq: u32,
    remote_uri: String,
    local_uri: String,
    state: CallState,
    // RTP session (Arc makes it cloneable)
    rtp_session: Option<Arc<RtpSession>>,
    // Task handles for cleanup (not cloned)
    audio_tx_task: Option<Arc<tokio::task::JoinHandle<()>>>,
    audio_rx_task: Option<Arc<tokio::task::JoinHandle<()>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallState {
    Idle,
    Calling,
    Ringing,
    Confirmed,
    Terminated,
}

pub struct SipEngine {
    socket: Option<Arc<UdpSocket>>,
    server: String,
    user: String,
    password: String,
    registered: bool,
    local_addr: String,
    active_dialog: Option<Dialog>,
}

impl Default for SipEngine {
    fn default() -> Self {
        Self {
            socket: None,
            server: String::new(),
            user: String::new(),
            password: String::new(),
            registered: false,
            local_addr: String::new(),
            active_dialog: None,
        }
    }
}

static SIP_ENGINE: Lazy<Arc<Mutex<SipEngine>>> =
    Lazy::new(|| Arc::new(Mutex::new(SipEngine::default())));

pub async fn init_pjsip() -> Result<(), String> {
    let mut engine = SIP_ENGINE.lock().await;

    if engine.socket.is_some() {
        return Ok(());
    }

    println!("[SIP] Initializing SIP stack");

    // Create UDP socket on ephemeral port
    let socket = UdpSocket::bind("0.0.0.0:0").await
        .map_err(|e| format!("Failed to create UDP socket: {}", e))?;

    let actual_local_addr = socket.local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?;

    // Get the actual local IP address by connecting to a public DNS server
    let local_ip = match std::net::UdpSocket::bind("0.0.0.0:0") {
        Ok(test_socket) => {
            match test_socket.connect("8.8.8.8:80") {
                Ok(_) => {
                    test_socket.local_addr()
                        .map(|addr| addr.ip().to_string())
                        .unwrap_or_else(|_| "127.0.0.1".to_string())
                }
                Err(_) => "127.0.0.1".to_string()
            }
        }
        Err(_) => "127.0.0.1".to_string()
    };
    
    let local_addr = format!("{}:{}", local_ip, actual_local_addr.port());

    println!("[SIP] UDP socket created");
    println!("[SIP] Actual bind address: {}", actual_local_addr);
    println!("[SIP] Advertised address: {}", local_addr);

    engine.socket = Some(Arc::new(socket));
    engine.local_addr = local_addr;

    println!("[SIP] SIP stack initialized successfully");

    Ok(())
}

pub async fn register_account(
    server: &str,
    user: &str,
    password: &str,
) -> Result<(), String> {
    let mut engine = SIP_ENGINE.lock().await;

    let socket = engine
        .socket
        .as_ref()
        .ok_or("SIP not initialized")?
        .clone();

    println!("[SIP] Registering account:");
    println!("  Server: {}", server);
    println!("  User: {}", user);

    // Store credentials
    engine.server = server.to_string();
    engine.user = user.to_string();
    engine.password = password.to_string();

    let local_addr = engine.local_addr.clone();
    
    // Release the lock before async operations
    drop(engine);

    // Build initial REGISTER message (without auth)
    let from_uri = format!("sip:{}@{}", user, server);
    let to_uri = from_uri.clone();
    let contact_uri = format!("sip:{}@{}", user, local_addr);
    let call_id = uuid::Uuid::new_v4().to_string();
    let branch = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
    let tag = uuid::Uuid::new_v4().simple().to_string();

    // Build raw SIP REGISTER message
    let register_msg = format!(
        "REGISTER sip:{} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {};branch={}\r\n\
         From: <{}>;tag={}\r\n\
         To: <{}>\r\n\
         Call-ID: {}\r\n\
         CSeq: 1 REGISTER\r\n\
         Contact: <{}>\r\n\
         Max-Forwards: 70\r\n\
         Expires: 3600\r\n\
         User-Agent: Platypus-Phone/0.1.0\r\n\
         Content-Length: 0\r\n\
         \r\n",
        server,
        local_addr,
        branch,
        from_uri,
        tag,
        to_uri,
        call_id,
        contact_uri
    );

    println!("[SIP] Sending initial REGISTER to {}", server);
    println!("[SIP] Message:\n{}", register_msg);

    // Resolve server address (DNS lookup if needed)
    println!("[SIP] Resolving server address: {}", server);
    let server_addr: std::net::SocketAddr = if server.contains(':') {
        // Already has port
        match server.parse() {
            Ok(addr) => addr,
            Err(_e) => {
                println!("[SIP] Failed to parse address directly, trying DNS lookup...");
                // Try DNS lookup
                let parts: Vec<&str> = server.split(':').collect();
                let host = parts[0];
                let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(5060);
                
                let addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await
                    .map_err(|e| format!("DNS lookup failed: {}", e))?;
                
                addrs.into_iter().next()
                    .ok_or_else(|| format!("No addresses found for {}", host))?
            }
        }
    } else {
        // Need to add port and possibly do DNS lookup
        println!("[SIP] Performing DNS lookup for {}...", server);
        let lookup_addr = format!("{}:5060", server);
        
        let addrs = tokio::net::lookup_host(&lookup_addr).await
            .map_err(|e| format!("DNS lookup failed for {}: {}", server, e))?;
        
        let resolved = addrs.into_iter().next()
            .ok_or_else(|| format!("No addresses found for {}", server))?;
        
        println!("[SIP] Resolved {} to {}", server, resolved);
        resolved
    };

    println!("[SIP] Target address: {}", server_addr);
    println!("[SIP] Sending {} bytes...", register_msg.len());

    // Send initial REGISTER request
    match socket.send_to(register_msg.as_bytes(), server_addr).await {
        Ok(sent_bytes) => {
            println!("[SIP] ✓ REGISTER sent successfully ({} bytes to {})", sent_bytes, server_addr);
        }
        Err(_e) => {
            println!("[SIP] ✗ Failed to send REGISTER: {}", _e);
            return Err(format!("Failed to send REGISTER: {}", _e));
        }
    }
    
    println!("[SIP] ✓ REGISTER sent ({} bytes to {})", register_msg.len(), server_addr);
    println!("[SIP] Waiting for server response...");
    
    // Listen for response with timeout
    let mut buf = vec![0u8; 4096];
    let response_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        socket.recv_from(&mut buf)
    ).await;
    
    match response_result {
        Ok(Ok((size, from_addr))) => {
            buf.truncate(size);
            let response_str = String::from_utf8_lossy(&buf);
            println!("[SIP] Received response from {} ({} bytes):", from_addr, size);
            println!("{}", response_str);
            
            // Check response code
            if response_str.contains("SIP/2.0 401") || response_str.contains("SIP/2.0 407") {
                println!("[SIP] Authentication required (401/407)");
                
                // Parse authentication parameters
                let auth_params = parse_auth_header(&response_str)?;
                
                // Calculate digest response
                let auth_header = calculate_digest_response(
                    user,
                    password,
                    "REGISTER",
                    &format!("sip:{}", server),
                    &auth_params,
                )?;
                
                println!("[SIP] Authorization header: {}", auth_header);
                
                // Build authenticated REGISTER with same Call-ID and tag but new branch and CSeq
                let branch2 = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
                let auth_register_msg = format!(
                    "REGISTER sip:{} SIP/2.0\r\n\
                     Via: SIP/2.0/UDP {};branch={}\r\n\
                     From: <{}>;tag={}\r\n\
                     To: <{}>\r\n\
                     Call-ID: {}\r\n\
                     CSeq: 2 REGISTER\r\n\
                     Contact: <{}>\r\n\
                     Max-Forwards: 70\r\n\
                     Expires: 3600\r\n\
                     Authorization: {}\r\n\
                     User-Agent: Platypus-Phone/0.1.0\r\n\
                     Content-Length: 0\r\n\
                     \r\n",
                    server,
                    local_addr,
                    branch2,
                    from_uri,
                    tag,
                    to_uri,
                    call_id,
                    contact_uri,
                    auth_header
                );
                
                println!("[SIP] Sending authenticated REGISTER...");
                
                socket.send_to(auth_register_msg.as_bytes(), server_addr).await
                    .map_err(|e| format!("Failed to send authenticated REGISTER: {}", e))?;
                
                println!("[SIP] ✓ Authenticated REGISTER sent ({} bytes)", auth_register_msg.len());
                println!("[SIP] Waiting for final response...");
                
                // Wait for final response
                let mut final_buf = vec![0u8; 4096];
                let final_response_result = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    socket.recv_from(&mut final_buf)
                ).await;
                
                match final_response_result {
                    Ok(Ok((final_size, final_from))) => {
                        final_buf.truncate(final_size);
                        let final_str = String::from_utf8_lossy(&final_buf);
                        println!("[SIP] Final response from {} ({} bytes):", final_from, final_size);
                        println!("{}", final_str);
                        
                        if final_str.contains("SIP/2.0 200") {
                            println!("[SIP] ✓✓✓ Registration successful! ✓✓✓");
                            let mut engine = SIP_ENGINE.lock().await;
                            engine.registered = true;
                            Ok(())
                        } else {
                            Err(format!("Registration failed: {}", 
                                final_str.lines().next().unwrap_or("Unknown error")))
                        }
                    }
                    Ok(Err(e)) => Err(format!("Error receiving final response: {}", e)),
                    Err(_) => Err("Timeout waiting for final response (10s)".to_string()),
                }
            } else if response_str.contains("SIP/2.0 200") {
                println!("[SIP] ✓✓✓ Registration successful (no auth required)! ✓✓✓");
                let mut engine = SIP_ENGINE.lock().await;
                engine.registered = true;
                Ok(())
            } else {
                Err(format!("Unexpected response: {}", 
                    response_str.lines().next().unwrap_or("Unknown")))
            }
        }
        Ok(Err(e)) => Err(format!("Socket error receiving response: {}", e)),
        Err(_) => {
            println!("[SIP] ✗ Timeout waiting for server response (10s)");
            println!("[SIP] This could mean:");
            println!("  - Server is not responding");
            println!("  - Firewall is blocking UDP port 5060");
            println!("  - Server address is incorrect");
            println!("  - Network connectivity issue");
            Err("Timeout waiting for server response (10s)".to_string())
        }
    }
}

// Parse authentication parameters from WWW-Authenticate header
fn parse_auth_header(response: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let mut params = std::collections::HashMap::new();
    
    // Find WWW-Authenticate or Proxy-Authenticate line
    let auth_line = response
        .lines()
        .find(|line| line.starts_with("WWW-Authenticate:") || line.starts_with("Proxy-Authenticate:"))
        .ok_or("No authentication header found")?;

    println!("[SIP] Auth header: {}", auth_line);

    // Parse Digest parameters
    if let Some(digest_part) = auth_line.split("Digest ").nth(1) {
        for param in digest_part.split(',') {
            let param = param.trim();
            if let Some((key, value)) = param.split_once('=') {
                let value = value.trim_matches('"');
                params.insert(key.trim().to_string(), value.to_string());
            }
        }
    }

    Ok(params)
}

// Calculate MD5 digest response for authentication
fn calculate_digest_response(
    username: &str,
    password: &str,
    method: &str,
    uri: &str,
    params: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let realm = params.get("realm").ok_or("Missing realm")?;
    let nonce = params.get("nonce").ok_or("Missing nonce")?;
    let default_algo = "MD5".to_string();
    let algorithm = params.get("algorithm").unwrap_or(&default_algo);
    let qop = params.get("qop");

    println!("[SIP] Calculating digest:");
    println!("  Realm: {}", realm);
    println!("  Nonce: {}", nonce);
    println!("  Algorithm: {}", algorithm);

    // Calculate HA1 = MD5(username:realm:password)
    let ha1_input = format!("{}:{}:{}", username, realm, password);
    let ha1 = format!("{:x}", md5_compute(ha1_input.as_bytes()));

    // Calculate HA2 = MD5(method:uri)
    let ha2_input = format!("{}:{}", method, uri);
    let ha2 = format!("{:x}", md5_compute(ha2_input.as_bytes()));

    // Calculate response
    let response = if let Some(qop_val) = qop {
        // With qop
        let nc = "00000001";
        let cnonce = format!("{:x}", md5_compute(uuid::Uuid::new_v4().to_string().as_bytes()));
        let response_input = format!("{}:{}:{}:{}:{}:{}", ha1, nonce, nc, cnonce, qop_val, ha2);
        let response = format!("{:x}", md5_compute(response_input.as_bytes()));
        
        format!(
            "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\", algorithm={}, qop={}, nc={}, cnonce=\"{}\"",
            username, realm, nonce, uri, response, algorithm, qop_val, nc, cnonce
        )
    } else {
        // Without qop
        let response_input = format!("{}:{}:{}", ha1, nonce, ha2);
        let response = format!("{:x}", md5_compute(response_input.as_bytes()));
        
        format!(
            "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\", algorithm={}",
            username, realm, nonce, uri, response, algorithm
        )
    };

    Ok(response)
}

// Generic function to send SIP request with automatic auth retry
async fn send_with_auth(
    socket: &UdpSocket,
    initial_request: &str,
    method: &str,
    uri: &str,
    username: &str,
    password: &str,
    server_addr: std::net::SocketAddr,
    timeout_secs: u64,
) -> Result<String, String> {
    // Send initial request
    socket.send_to(initial_request.as_bytes(), server_addr).await
        .map_err(|e| format!("Failed to send {}: {}", method, e))?;

    println!("[SIP] ✓ {} sent ({} bytes)", method, initial_request.len());

    // Wait for responses - may receive 100 Trying before 401
    let mut buf = vec![0u8; 4096];
    let mut auth_challenge: Option<String> = None;
    
    // Keep listening for responses until we get a final response or auth challenge
    loop {
        let response_result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            socket.recv_from(&mut buf)
        ).await;

        match response_result {
            Ok(Ok((size, _))) => {
                buf.truncate(size);
                let response_str = String::from_utf8_lossy(&buf).to_string();
                
                println!("[SIP] Received response: {}", response_str.lines().next().unwrap_or(""));
                
                // Check if this is a provisional response (1xx)
                if response_str.contains("SIP/2.0 100") || 
                   response_str.contains("SIP/2.0 180") || 
                   response_str.contains("SIP/2.0 183") {
                    println!("[SIP] Provisional response, waiting for final response...");
                    buf = vec![0u8; 4096]; // Reset buffer
                    continue; // Keep waiting
                }
                
                // Check if authentication is required
                if response_str.contains("SIP/2.0 401") || response_str.contains("SIP/2.0 407") {
                    println!("[SIP] Authentication required (401/407), retrying with auth...");
                    auth_challenge = Some(response_str);
                    break;
                }
                
                // Any other response (2xx, 4xx, 5xx, 6xx) - return it
                return Ok(response_str);
            }
            Ok(Err(e)) => return Err(format!("Socket error: {}", e)),
            Err(_) => return Err(format!("Timeout waiting for {} response", method)),
        }
    }
    
    // If we got here, we have an auth challenge
    if let Some(challenge) = auth_challenge {
        // Parse auth parameters
        let auth_params = parse_auth_header(&challenge)?;
        
        // Calculate digest
        let auth_header = calculate_digest_response(
            username,
            password,
            method,
            uri,
            &auth_params,
        )?;
        
        // Rebuild request with Authorization header
        // Find where to insert the Authorization header (before Content-Type or Content-Length)
        let auth_request = if let Some(content_pos) = initial_request.find("Content-Type:") {
            format!(
                "{}Authorization: {}\r\n{}",
                &initial_request[..content_pos],
                auth_header,
                &initial_request[content_pos..]
            )
        } else if let Some(content_pos) = initial_request.find("Content-Length:") {
            format!(
                "{}Authorization: {}\r\n{}",
                &initial_request[..content_pos],
                auth_header,
                &initial_request[content_pos..]
            )
        } else if let Some(user_agent_pos) = initial_request.find("User-Agent:") {
            // Insert after User-Agent line
            if let Some(line_end) = initial_request[user_agent_pos..].find("\r\n") {
                let insert_pos = user_agent_pos + line_end + 2;
                format!(
                    "{}Authorization: {}\r\n{}",
                    &initial_request[..insert_pos],
                    auth_header,
                    &initial_request[insert_pos..]
                )
            } else {
                return Err("Failed to parse request for auth insertion".to_string());
            }
        } else {
            return Err("Failed to find insertion point for Authorization header".to_string());
        };
        
        // Also need to update CSeq
        let auth_request = auth_request.replace(
            &format!("CSeq: 1 {}", method),
            &format!("CSeq: 2 {}", method)
        );
        
        // Update branch parameter
        let new_branch = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
        let auth_request = if let Some(via_start) = auth_request.find("Via: ") {
            if let Some(branch_start) = auth_request[via_start..].find("branch=") {
                let abs_branch_start = via_start + branch_start + 7; // 7 = len("branch=")
                if let Some(branch_end) = auth_request[abs_branch_start..].find(|c| c == ';' || c == '\r') {
                    let abs_branch_end = abs_branch_start + branch_end;
                    format!(
                        "{}{}{}",
                        &auth_request[..abs_branch_start],
                        new_branch,
                        &auth_request[abs_branch_end..]
                    )
                } else {
                    auth_request
                }
            } else {
                auth_request
            }
        } else {
            auth_request
        };
        
        println!("[SIP] Sending authenticated {}...", method);
        println!("[SIP] Auth request (first 10 lines):");
        for (i, line) in auth_request.lines().take(10).enumerate() {
            println!("[SIP]   {}: {}", i+1, line);
        }
        
        // Send authenticated request
        socket.send_to(auth_request.as_bytes(), server_addr).await
            .map_err(|e| format!("Failed to send authenticated {}: {}", method, e))?;
        
        println!("[SIP] ✓ Authenticated {} sent ({} bytes)", method, auth_request.len());
        
        // Wait for final response (may get provisional responses again)
        loop {
            let mut final_buf = vec![0u8; 4096];
            let final_result = tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                socket.recv_from(&mut final_buf)
            ).await;
            
            match final_result {
                Ok(Ok((final_size, _))) => {
                    final_buf.truncate(final_size);
                    let final_response = String::from_utf8_lossy(&final_buf).to_string();
                    
                    println!("[SIP] Received response: {}", final_response.lines().next().unwrap_or(""));
                    
                    // Skip provisional responses
                    if final_response.contains("SIP/2.0 100") || 
                       final_response.contains("SIP/2.0 180") || 
                       final_response.contains("SIP/2.0 183") {
                        println!("[SIP] Provisional response, waiting for final response...");
                        continue;
                    }
                    
                    // Return any final response
                    return Ok(final_response);
                }
                Ok(Err(e)) => return Err(format!("Socket error: {}", e)),
                Err(_) => return Err(format!("Timeout waiting for authenticated {} response", method)),
            }
        }
    }
    
    Err("No auth challenge received".to_string())
}

// Start RTP media session after call is established
async fn start_rtp_media(response_sdp: &str, local_port: u16) -> Result<(Arc<RtpSession>, tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>), String> {
tracing::info!("[RTP] Starting RTP media session...");
println!("[RTP] Starting RTP media session...");

// Parse remote SDP
let (remote_ip, remote_port, payload_type) = parse_sdp(response_sdp)?;

tracing::info!("[RTP] Remote endpoint: {}:{}", remote_ip, remote_port);
tracing::info!("[RTP] Payload type: {} ({})", payload_type,
if payload_type == 0 { "PCMU" } else if payload_type == 8 { "PCMA" } else { "Unknown" });

println!("[RTP] Remote endpoint: {}:{}", remote_ip, remote_port);
println!("[RTP] Payload type: {} ({})", payload_type,
if payload_type == 0 { "PCMU" } else if payload_type == 8 { "PCMA" } else { "Unknown" });

// Create remote address
let remote_addr: std::net::SocketAddr = format!("{}:{}", remote_ip, remote_port)
.parse()
.map_err(|e| format!("Invalid remote address: {}", e))?;

// Create RTP session
let rtp_session = Arc::new(
RtpSession::new(local_port, remote_addr, payload_type).await?
);

tracing::info!("[RTP] ✓ RTP session created");
println!("[RTP] ✓ RTP session created");

// Initialize audio manager
tracing::info!("[Audio] Initializing audio devices...");
println!("[Audio] Initializing audio devices...");

let mut audio_manager = match AudioManager::new() {
    Ok(mgr) => {
        tracing::info!("[Audio] ✓ AudioManager created");
        mgr
    }
    Err(e) => {
        tracing::error!("[Audio] ✗ Failed to create AudioManager: {}", e);
        println!("[Audio] ✗ Failed to create AudioManager: {}", e);
        return Err(e);
    }
};

tracing::info!("[Audio] Calling init_input()...");
println!("[Audio] Calling init_input()...");
match audio_manager.init_input() {
    Ok(_) => {
        tracing::info!("[Audio] ✓ Input device initialized");
        println!("[Audio] ✓ Input device initialized");
    }
    Err(e) => {
        tracing::error!("[Audio] ✗ Failed to init input: {}", e);
        println!("[Audio] ✗ Failed to init input: {}", e);
        return Err(e);
    }
}

tracing::info!("[Audio] Calling init_output()...");
match audio_manager.init_output() {
Ok(_) => tracing::info!("[Audio] ✓ Output device initialized"),
Err(e) => {
tracing::error!("[Audio] ✗ Failed to init output: {}", e);
return Err(e);
}
}

// Start audio capture
tracing::info!("[Audio] Starting audio capture...");
let (input_stream, mut audio_rx) = match audio_manager.start_capture() {
Ok(result) => {
tracing::info!("[Audio] ✓ Audio capture started");
result
}
Err(e) => {
tracing::error!("[Audio] ✗ Failed to start capture: {}", e);
return Err(e);
}
};

// Start audio playback
tracing::info!("[Audio] Starting audio playback...");
let (output_stream, audio_tx) = match audio_manager.start_playback() {
Ok(result) => {
tracing::info!("[Audio] ✓ Audio playback started");
result
}
Err(e) => {
tracing::error!("[Audio] ✗ Failed to start playback: {}", e);
return Err(e);
}
};

tracing::info!("[Audio] ✓ Audio devices initialized");
println!("[Audio] ✓ Audio devices initialized");
    
    // Keep streams alive by leaking them (they'll be cleaned up when tasks abort)
    // This is necessary because Stream is not Send and cannot be moved into tokio::spawn
    std::mem::forget(input_stream);
    std::mem::forget(output_stream);
    
    // Spawn TX task: Microphone → Downsample → Encode → RTP → Network
    let rtp_tx = rtp_session.clone();
    let tx_payload_type = payload_type; // Capture for move
    let tx_task = tokio::spawn(async move {
        tracing::info!("[Audio] TX task started (Mic → RTP)");
        println!("[Audio] TX task started (Mic → RTP)");
        let mut packet_count = 0u64;
        
        while let Some(samples) = audio_rx.recv().await {
            tracing::debug!("[Audio] TX: Received {} samples from mic", samples.len());
            
            // Simple downsampling: 48kHz → 8kHz (take every 6th sample)
            // This is crude but will make audio work
            let downsampled: Vec<i16> = samples.iter()
                .step_by(6)
                .copied()
                .collect();
            
            tracing::debug!("[Audio] TX: Downsampled to {} samples", downsampled.len());
            
            // Encode samples to G.711
            let encoded: Vec<u8> = if tx_payload_type == 0 {
                // PCMU (μ-law)
                downsampled.iter().map(|&s| g711::encode_ulaw(s)).collect()
            } else {
                // PCMA (A-law)
                downsampled.iter().map(|&s| g711::encode_alaw(s)).collect()
            };
            
            // Send RTP packet
            if let Err(e) = rtp_tx.send_audio(&encoded).await {
                tracing::error!("[RTP] TX error: {}", e);
                eprintln!("[RTP] TX error: {}", e);
                break;
            }
            
            packet_count += 1;
            if packet_count % 50 == 0 {
                tracing::info!("[RTP] Sent {} packets", packet_count);
                println!("[RTP] Sent {} packets", packet_count);
            }
        }
        
        tracing::info!("[Audio] TX task ended");
        println!("[Audio] TX task ended");
    });
    
    // Spawn RX task: Network → RTP → Decode → Upsample → Speaker
    let rtp_rx = rtp_session.clone();
    let rx_payload_type = payload_type; // Capture for move
    let rx_task = tokio::spawn(async move {
        tracing::info!("[Audio] RX task started (RTP → Speaker)");
        println!("[Audio] RX task started (RTP → Speaker)");
        let mut packet_count = 0u64;
        
        loop {
            match rtp_rx.receive_audio().await {
                Ok(encoded) => {
                    tracing::debug!("[Audio] RX: Received {} encoded bytes", encoded.len());
                    
                    // Decode G.711 to PCM
                    let decoded: Vec<i16> = if rx_payload_type == 0 {
                        // PCMU (μ-law)
                        encoded.iter().map(|&b| g711::decode_ulaw(b)).collect()
                    } else {
                        // PCMA (A-law)
                        encoded.iter().map(|&b| g711::decode_alaw(b)).collect()
                    };
                    
                    tracing::debug!("[Audio] RX: Decoded to {} samples", decoded.len());
                    
                    // Simple upsampling: 8kHz → 48kHz (repeat each sample 6 times)
                    // This is crude but will make audio work
                    let upsampled: Vec<i16> = decoded.iter()
                        .flat_map(|&sample| std::iter::repeat(sample).take(6))
                        .collect();
                    
                    tracing::debug!("[Audio] RX: Upsampled to {} samples", upsampled.len());
                    
                    // Send to speaker
                    if let Err(e) = audio_tx.send(upsampled).await {
                        tracing::error!("[Audio] Playback error: {}", e);
                        eprintln!("[Audio] Playback error: {}", e);
                        break;
                    }
                    
                    packet_count += 1;
                    if packet_count % 50 == 0 {
                        tracing::info!("[RTP] Received {} packets", packet_count);
                        println!("[RTP] Received {} packets", packet_count);
                    }
                }
                Err(e) => {
                    tracing::error!("[RTP] RX error: {}", e);
                    eprintln!("[RTP] RX error: {}", e);
                    break;
                }
            }
        }
        
        tracing::info!("[Audio] RX task ended");
        println!("[Audio] RX task ended");
    });
    
    println!("[RTP] ✓✓✓ RTP media session active! ✓✓✓");
    
    Ok((rtp_session, tx_task, rx_task))
}

pub async fn make_call(number: &str) -> Result<(), String> {
    let mut engine = SIP_ENGINE.lock().await;

    if !engine.registered {
        return Err("Not registered".to_string());
    }

    let socket = engine.socket.as_ref().ok_or("SIP not initialized")?.clone();
    let server = engine.server.clone();
    let user = engine.user.clone();
    let local_addr = engine.local_addr.clone();

    println!("[SIP] Making call to: {}", number);
    println!("[SIP] From: {}@{}", user, server);

    // Build destination URI
    let dest_uri = if number.starts_with("sip:") {
        number.to_string()
    } else {
        format!("sip:{}@{}", number, server)
    };

    println!("[SIP] Destination URI: {}", dest_uri);

    // Create dialog for this call
    let call_id = uuid::Uuid::new_v4().to_string();
    let from_tag = uuid::Uuid::new_v4().simple().to_string();
    let from_uri = format!("sip:{}@{}", user, server);
    
    let dialog = Dialog {
        call_id: call_id.clone(),
        from_tag: from_tag.clone(),
        to_tag: None,
        cseq: 1,
        remote_uri: dest_uri.clone(),
        local_uri: from_uri.clone(),
        state: CallState::Calling,
        rtp_session: None,
        audio_tx_task: None,
        audio_rx_task: None,
    };
    
    engine.active_dialog = Some(dialog);
    drop(engine);

    // Generate SDP (Session Description Protocol)
    let local_ip = local_addr.split(':').next().unwrap_or("127.0.0.1");
    let rtp_port = 10000; // TODO: Allocate actual RTP port
    let session_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let sdp = format!(
        "v=0\r\n\
         o=- {} {} IN IP4 {}\r\n\
         s=Platypus Phone Call\r\n\
         c=IN IP4 {}\r\n\
         t=0 0\r\n\
         m=audio {} RTP/AVP 0 8 101\r\n\
         a=rtpmap:0 PCMU/8000\r\n\
         a=rtpmap:8 PCMA/8000\r\n\
         a=rtpmap:101 telephone-event/8000\r\n\
         a=sendrecv\r\n",
        session_id,
        session_id,
        local_ip,
        local_ip,
        rtp_port
    );

    // Build INVITE request
    let branch = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
    let contact_uri = format!("sip:{}@{}", user, local_addr);
    
    let invite_msg = format!(
        "INVITE {} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {};branch={}\r\n\
         From: <{}>;tag={}\r\n\
         To: <{}>\r\n\
         Call-ID: {}\r\n\
         CSeq: 1 INVITE\r\n\
         Contact: <{}>\r\n\
         Max-Forwards: 70\r\n\
         Content-Type: application/sdp\r\n\
         User-Agent: Platypus-Phone/0.1.0\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        dest_uri,
        local_addr,
        branch,
        from_uri,
        from_tag,
        dest_uri,
        call_id,
        contact_uri,
        sdp.len(),
        sdp
    );

    println!("[SIP] Sending INVITE...");
    println!("[SIP] Message:\n{}", invite_msg);

    // Resolve server address
    let server_addr: std::net::SocketAddr = if server.contains(':') {
        match server.parse() {
            Ok(addr) => addr,
            Err(_) => {
                let parts: Vec<&str> = server.split(':').collect();
                let host = parts[0];
                let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(5060);
                let addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await
                    .map_err(|e| format!("DNS lookup failed: {}", e))?;
                addrs.into_iter().next()
                    .ok_or_else(|| format!("No addresses found for {}", host))?
            }
        }
    } else {
        let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host(format!("{}:5060", server)).await
            .map_err(|e| format!("DNS lookup failed: {}", e))?
            .collect();
        *addrs.first()
            .ok_or_else(|| format!("No addresses found for {}", server))?
    };

    // Get password for auth
    let password = {
        let engine = SIP_ENGINE.lock().await;
        engine.password.clone()
    };

    // Send INVITE with auth handling
    let first_response = send_with_auth(
        &socket,
        &invite_msg,
        "INVITE",
        &dest_uri,
        &user,
        &password,
        server_addr,
        30,
    ).await?;

    println!("[SIP] First response:");
    println!("{}", first_response);

    // Check if first response needs further handling
    if first_response.contains("SIP/2.0 200") {
        // Call answered immediately
        println!("[SIP] 200 OK - call answered!");
        
        let to_tag = extract_to_tag(&first_response);
        println!("[SIP] To tag: {:?}", to_tag);
        
        let mut engine = SIP_ENGINE.lock().await;
        if let Some(ref mut dialog) = engine.active_dialog {
            dialog.to_tag = to_tag.clone();
            dialog.state = CallState::Confirmed;
            dialog.cseq = 2; // Auth used CSeq 2
        }
        drop(engine);
        
        send_ack(&socket, &dest_uri, &call_id, &from_tag, to_tag.as_deref(), &from_uri, &local_addr, server_addr).await?;
        
        println!("[SIP] ✓✓✓ Call established! ✓✓✓");
        
        // Start RTP media session
        match start_rtp_media(&first_response, rtp_port).await {
            Ok((rtp_session, tx_task, rx_task)) => {
                // Store RTP components in dialog
                let mut engine = SIP_ENGINE.lock().await;
                if let Some(ref mut dialog) = engine.active_dialog {
                    dialog.rtp_session = Some(rtp_session);
                    dialog.audio_tx_task = Some(Arc::new(tx_task));
                    dialog.audio_rx_task = Some(Arc::new(rx_task));
                }
                println!("[SIP] ✓ RTP media active - call has audio!");
            }
            Err(e) => {
                tracing::error!("[RTP] Failed to start media: {}", e);
                eprintln!("[RTP] Failed to start media: {}", e);
                println!("[SIP] Call established but no audio (RTP failed)");
            }
        }
        
        return Ok(());
    } else if first_response.contains("SIP/2.0 180") || first_response.contains("SIP/2.0 183") {
        println!("[SIP] 180/183 Ringing - waiting for answer...");
        let mut engine = SIP_ENGINE.lock().await;
        if let Some(ref mut dialog) = engine.active_dialog {
            dialog.state = CallState::Ringing;
            dialog.cseq = 2; // Auth used CSeq 2
        }
        drop(engine);
    }

    // Continue listening for more responses
    let mut buf = vec![0u8; 4096];
    loop {
        let response_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            socket.recv_from(&mut buf)
        ).await;

        match response_result {
            Ok(Ok((size, from_addr))) => {
                buf.truncate(size);
                let response_str = String::from_utf8_lossy(&buf);
                println!("[SIP] Received response from {} ({} bytes):", from_addr, size);
                println!("{}", response_str);

                if response_str.contains("SIP/2.0 100") {
                    println!("[SIP] 100 Trying - call is being processed");
                    buf = vec![0u8; 4096]; // Reset buffer
                    continue;
                } else if response_str.contains("SIP/2.0 180") || response_str.contains("SIP/2.0 183") {
                    println!("[SIP] 180/183 Ringing - remote party is being alerted");
                    let mut engine = SIP_ENGINE.lock().await;
                    if let Some(ref mut dialog) = engine.active_dialog {
                        dialog.state = CallState::Ringing;
                    }
                    drop(engine);
                    buf = vec![0u8; 4096]; // Reset buffer
                    continue;
                } else if response_str.contains("SIP/2.0 200") {
                    println!("[SIP] 200 OK - call answered!");
                    
                    // Extract To tag from response
                    let to_tag = extract_to_tag(&response_str);
                    println!("[SIP] To tag: {:?}", to_tag);
                    
                    // Update dialog
                    let mut engine = SIP_ENGINE.lock().await;
                    if let Some(ref mut dialog) = engine.active_dialog {
                        dialog.to_tag = to_tag.clone();
                        dialog.state = CallState::Confirmed;
                    }
                    drop(engine);
                    
                    // Send ACK
                    send_ack(&socket, &dest_uri, &call_id, &from_tag, to_tag.as_deref(), &from_uri, &local_addr, server_addr).await?;
                    
                    println!("[SIP] ✓✓��� Call established! ✓✓✓");
                    // Start RTP media session
                    match start_rtp_media(&response_str, rtp_port).await {
                        Ok((rtp_session, tx_task, rx_task)) => {
                            // Store RTP components in dialog
                            let mut engine = SIP_ENGINE.lock().await;
                            if let Some(ref mut dialog) = engine.active_dialog {
                                dialog.rtp_session = Some(rtp_session);
                                dialog.audio_tx_task = Some(Arc::new(tx_task));
                                dialog.audio_rx_task = Some(Arc::new(rx_task));
                            }
                            println!("[SIP] ✓ RTP media active - call has audio!");
                        }
                        Err(e) => {
                            tracing::error!("[RTP] Failed to start media: {}", e);
                            eprintln!("[RTP] Failed to start media: {}", e);
                            println!("[SIP] Call established but no audio (RTP failed)");
                        }
                    }
                    
                    return Ok(());
                } else if response_str.contains("SIP/2.0 4") || response_str.contains("SIP/2.0 5") || response_str.contains("SIP/2.0 6") {
                    let status_line = response_str.lines().next().unwrap_or("Unknown error");
                    println!("[SIP] Call failed: {}", status_line);
                    
                    // Clean up dialog
                    let mut engine = SIP_ENGINE.lock().await;
                    engine.active_dialog = None;
                    
                    return Err(format!("Call failed: {}", status_line));
                }
            }
            Ok(Err(e)) => {
                println!("[SIP] Socket error: {}", e);
                return Err(format!("Socket error: {}", e));
            }
            Err(_) => {
                println!("[SIP] Timeout waiting for response");
                return Err("Timeout waiting for call response".to_string());
            }
        }
    }
}

// Send ACK to confirm call establishment
async fn send_ack(
    socket: &UdpSocket,
    dest_uri: &str,
    call_id: &str,
    from_tag: &str,
    to_tag: Option<&str>,
    from_uri: &str,
    local_addr: &str,
    server_addr: std::net::SocketAddr,
) -> Result<(), String> {
    let branch = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
    
    let to_header = if let Some(tag) = to_tag {
        format!("<{}>;tag={}", dest_uri, tag)
    } else {
        format!("<{}>", dest_uri)
    };
    
    // ACK CSeq must match the INVITE CSeq (which is 2 after auth retry)
    let ack_msg = format!(
        "ACK {} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {};branch={}\r\n\
         From: <{}>;tag={}\r\n\
         To: {}\r\n\
         Call-ID: {}\r\n\
         CSeq: 2 ACK\r\n\
         Max-Forwards: 70\r\n\
         User-Agent: Platypus-Phone/0.1.0\r\n\
         Content-Length: 0\r\n\
         \r\n",
        dest_uri,
        local_addr,
        branch,
        from_uri,
        from_tag,
        to_header,
        call_id
    );

    println!("[SIP] Sending ACK...");
    println!("[SIP] ACK message:\n{}", ack_msg);
    
    socket.send_to(ack_msg.as_bytes(), server_addr).await
        .map_err(|e| format!("Failed to send ACK: {}", e))?;

    println!("[SIP] ✓ ACK sent");
    Ok(())
}

// Extract To tag from SIP response
fn extract_to_tag(response: &str) -> Option<String> {
    for line in response.lines() {
        if line.starts_with("To:") || line.starts_with("t:") {
            if let Some(tag_part) = line.split("tag=").nth(1) {
                let tag = tag_part.split(';').next()
                    .unwrap_or(tag_part)
                    .trim()
                    .to_string();
                return Some(tag);
            }
        }
    }
    None
}

pub async fn answer_call() -> Result<(), String> {
    let engine = SIP_ENGINE.lock().await;

    if !engine.registered {
        return Err("Not registered".to_string());
    }

    println!("[SIP] Answering incoming call");
    println!("[SIP] Answer functionality not yet implemented");
    println!("[SIP] In production, this would:");
    println!("  - Send 200 OK response to INVITE");
    println!("  - Include SDP in response");
    println!("  - Establish RTP media stream");

    Ok(())
}

pub async fn hangup_call() -> Result<(), String> {
    let engine = SIP_ENGINE.lock().await;

    if !engine.registered {
        return Err("Not registered".to_string());
    }

    let socket = engine.socket.as_ref().ok_or("SIP not initialized")?.clone();
    let server = engine.server.clone();
    
    let dialog = engine.active_dialog.as_ref()
        .ok_or("No active call")?
        .clone();
    
    if dialog.state == CallState::Terminated {
        return Err("Call already terminated".to_string());
    }
    
    drop(engine);

    println!("[SIP] Hanging up call");
    println!("[SIP] Call-ID: {}", dialog.call_id);

    // Abort audio tasks if they exist
    if let Some(tx_task) = dialog.audio_tx_task {
        tx_task.abort();
        println!("[Audio] TX task aborted");
    }
    if let Some(rx_task) = dialog.audio_rx_task {
        rx_task.abort();
        println!("[Audio] RX task aborted");
    }
    // Streams will be dropped automatically when dialog is cleared

    // Build BYE request
    let branch = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
    let local_addr = {
        let engine = SIP_ENGINE.lock().await;
        engine.local_addr.clone()
    };
    
    let to_header = if let Some(ref tag) = dialog.to_tag {
        format!("<{}>;tag={}", dialog.remote_uri, tag)
    } else {
        format!("<{}>", dialog.remote_uri)
    };
    
    let bye_msg = format!(
        "BYE {} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {};branch={}\r\n\
         From: <{}>;tag={}\r\n\
         To: {}\r\n\
         Call-ID: {}\r\n\
         CSeq: {} BYE\r\n\
         Max-Forwards: 70\r\n\
         User-Agent: Platypus-Phone/0.1.0\r\n\
         Content-Length: 0\r\n\
         \r\n",
        dialog.remote_uri,
        local_addr,
        branch,
        dialog.local_uri,
        dialog.from_tag,
        to_header,
        dialog.call_id,
        dialog.cseq + 1
    );

    println!("[SIP] Sending BYE...");
    println!("[SIP] Message:\n{}", bye_msg);

    // Resolve server address
    let server_addr: std::net::SocketAddr = if server.contains(':') {
        match server.parse() {
            Ok(addr) => addr,
            Err(_) => {
                let parts: Vec<&str> = server.split(':').collect();
                let host = parts[0];
                let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(5060);
                let addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await
                    .map_err(|e| format!("DNS lookup failed: {}", e))?;
                addrs.into_iter().next()
                    .ok_or_else(|| format!("No addresses found for {}", host))?
            }
        }
    } else {
        let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host(format!("{}:5060", server)).await
            .map_err(|e| format!("DNS lookup failed: {}", e))?
            .collect();
        *addrs.first()
            .ok_or_else(|| format!("No addresses found for {}", server))?
    };

    // Send BYE
    socket.send_to(bye_msg.as_bytes(), server_addr).await
        .map_err(|e| format!("Failed to send BYE: {}", e))?;

    println!("[SIP] ✓ BYE sent ({} bytes to {})", bye_msg.len(), server_addr);
    println!("[SIP] Waiting for 200 OK...");

    // Wait for 200 OK response
    let mut buf = vec![0u8; 4096];
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        socket.recv_from(&mut buf)
    ).await {
        Ok(Ok((size, _))) => {
            buf.truncate(size);
            let response_str = String::from_utf8_lossy(&buf);
            println!("[SIP] Response: {}", response_str.lines().next().unwrap_or("Unknown"));
            
            if response_str.contains("SIP/2.0 200") {
                println!("[SIP] ✓ Call terminated successfully");
            }
        }
        _ => {
            println!("[SIP] No response to BYE (call terminated anyway)");
        }
    }

    // Clean up dialog
    let mut engine = SIP_ENGINE.lock().await;
    engine.active_dialog = None;

    println!("[SIP] ✓ Call ended");
    Ok(())
}

// Unregister from SIP server (send REGISTER with Expires: 0)
pub async fn unregister() -> Result<(), String> {
    let engine = SIP_ENGINE.lock().await;

    let socket = match engine.socket.as_ref() {
        Some(s) => s.clone(),
        None => return Ok(()), // Not initialized, nothing to do
    };

    if !engine.registered {
        return Ok(()); // Not registered, nothing to do
    }

    let server = engine.server.clone();
    let user = engine.user.clone();
    let password = engine.password.clone();
    let local_addr = engine.local_addr.clone();
    
    drop(engine); // Release lock

    println!("[SIP] Unregistering from {}", server);

    // Build REGISTER with Expires: 0 to unregister
    let from_uri = format!("sip:{}@{}", user, server);
    let to_uri = from_uri.clone();
    let contact_uri = format!("sip:{}@{}", user, local_addr);
    let call_id = uuid::Uuid::new_v4().to_string();
    let branch = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
    let tag = uuid::Uuid::new_v4().simple().to_string();

    let unregister_msg = format!(
        "REGISTER sip:{} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {};branch={}\r\n\
         From: <{}>;tag={}\r\n\
         To: <{}>\r\n\
         Call-ID: {}\r\n\
         CSeq: 1 REGISTER\r\n\
         Contact: <{}>\r\n\
         Max-Forwards: 70\r\n\
         Expires: 0\r\n\
         User-Agent: Platypus-Phone/0.1.0\r\n\
         Content-Length: 0\r\n\
         \r\n",
        server,
        local_addr,
        branch,
        from_uri,
        tag,
        to_uri,
        call_id,
        contact_uri
    );

    // Resolve server address
    let server_addr: std::net::SocketAddr = if server.contains(':') {
        match server.parse() {
            Ok(addr) => addr,
            Err(_) => {
                let parts: Vec<&str> = server.split(':').collect();
                let host = parts[0];
                let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(5060);
                
                let addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await
                    .map_err(|e| format!("DNS lookup failed: {}", e))?;
                
                addrs.into_iter().next()
                    .ok_or_else(|| format!("No addresses found for {}", host))?
            }
        }
    } else {
        let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host(format!("{}:5060", server)).await
            .map_err(|e| format!("DNS lookup failed: {}", e))?
            .collect();
        
        *addrs.first()
            .ok_or_else(|| format!("No addresses found for {}", server))?
    };

    // Send initial unregister request
    socket.send_to(unregister_msg.as_bytes(), server_addr).await
        .map_err(|e| format!("Failed to send unregister: {}", e))?;

    println!("[SIP] ✓ Unregister sent (Expires: 0)");

    // Wait for response
    let mut buf = vec![0u8; 4096];
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        socket.recv_from(&mut buf)
    ).await {
        Ok(Ok((size, _))) => {
            buf.truncate(size);
            let response_str = String::from_utf8_lossy(&buf);
            
            if response_str.contains("SIP/2.0 200") {
                println!("[SIP] ✓ Unregistered successfully");
            } else if response_str.contains("SIP/2.0 401") || response_str.contains("SIP/2.0 407") {
                println!("[SIP] Authentication required for unregister, sending with auth...");
                
                // Parse authentication parameters
                let auth_params = parse_auth_header(&response_str)?;
                
                // Calculate digest response
                let auth_header = calculate_digest_response(
                    &user,
                    &password,
                    "REGISTER",
                    &format!("sip:{}", server),
                    &auth_params,
                )?;
                
                // Build authenticated unregister with same Call-ID and tag
                let branch2 = format!("z9hG4bK{}", uuid::Uuid::new_v4().simple());
                let auth_unregister_msg = format!(
                    "REGISTER sip:{} SIP/2.0\r\n\
                     Via: SIP/2.0/UDP {};branch={}\r\n\
                     From: <{}>;tag={}\r\n\
                     To: <{}>\r\n\
                     Call-ID: {}\r\n\
                     CSeq: 2 REGISTER\r\n\
                     Contact: <{}>\r\n\
                     Max-Forwards: 70\r\n\
                     Expires: 0\r\n\
                     Authorization: {}\r\n\
                     User-Agent: Platypus-Phone/0.1.0\r\n\
                     Content-Length: 0\r\n\
                     \r\n",
                    server,
                    local_addr,
                    branch2,
                    from_uri,
                    tag,
                    to_uri,
                    call_id,
                    contact_uri,
                    auth_header
                );
                
                // Send authenticated unregister
                socket.send_to(auth_unregister_msg.as_bytes(), server_addr).await
                    .map_err(|e| format!("Failed to send authenticated unregister: {}", e))?;
                
                println!("[SIP] ✓ Authenticated unregister sent");
                
                // Wait for final response
                let mut final_buf = vec![0u8; 4096];
                match tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    socket.recv_from(&mut final_buf)
                ).await {
                    Ok(Ok((final_size, _))) => {
                        final_buf.truncate(final_size);
                        let final_str = String::from_utf8_lossy(&final_buf);
                        if final_str.contains("SIP/2.0 200") {
                            println!("[SIP] ✓ Unregistered successfully");
                        } else {
                            println!("[SIP] Unregister response: {}", final_str.lines().next().unwrap_or("Unknown"));
                        }
                    }
                    _ => {
                        println!("[SIP] No response to authenticated unregister (continuing anyway)");
                    }
                }
            } else {
                println!("[SIP] Unregister response: {}", response_str.lines().next().unwrap_or("Unknown"));
            }
        }
        _ => {
            println!("[SIP] No response to unregister (continuing anyway)");
        }
    }

    // Update state
    let mut engine = SIP_ENGINE.lock().await;
    engine.registered = false;

    Ok(())
}

pub async fn shutdown() {
    let mut engine = SIP_ENGINE.lock().await;

    if engine.socket.is_some() {
        println!("[SIP] Shutting down SIP stack");
        engine.socket = None;
        engine.registered = false;
    }
}
