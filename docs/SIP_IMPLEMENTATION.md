# SIP Implementation Technical Reference

## Overview

Platypus Phone implements a native SIP client using direct UDP socket communication with full RFC 3261 compliance for basic call functionality.

---

## Architecture

### Stack Components

```
React UI (TypeScript)
        ↓
   Tauri IPC
        ↓
  Rust Backend
        ↓
  Tokio UDP Socket
        ↓
   Raw SIP/UDP
        ↓
  SIP Server (PBX)
```

### Dependencies

- **tokio**: Async runtime and UDP socket
- **uuid**: Generating unique Call-IDs, branches, tags
- **md5**: Digest authentication calculations
- **once_cell**: Thread-safe global state
- **serde/serde_json**: Serialization for Tauri IPC

---

## SIP Registration Flow

### 1. Initial REGISTER (No Auth)

**Request:**
```
REGISTER sip:server.com SIP/2.0
Via: SIP/2.0/UDP local_ip:port;branch=z9hG4bK{uuid}
From: <sip:user@server.com>;tag={uuid}
To: <sip:user@server.com>
Call-ID: {uuid}
CSeq: 1 REGISTER
Contact: <sip:user@local_ip:port>
Max-Forwards: 70
Expires: 3600
User-Agent: Platypus-Phone/0.1.0
Content-Length: 0
```

**Response:**
```
SIP/2.0 401 Unauthorized
WWW-Authenticate: Digest realm="...", nonce="..."
```

### 2. Authenticated REGISTER

**Digest Calculation (RFC 2617):**

```
HA1 = MD5(username:realm:password)
HA2 = MD5(REGISTER:sip:server.com)
response = MD5(HA1:nonce:HA2)
```

**With qop (quality of protection):**
```
response = MD5(HA1:nonce:nc:cnonce:qop:HA2)
```

**Request:**
```
REGISTER sip:server.com SIP/2.0
Via: SIP/2.0/UDP local_ip:port;branch=z9hG4bK{new_uuid}
From: <sip:user@server.com>;tag={same_tag}
To: <sip:user@server.com>
Call-ID: {same_call_id}
CSeq: 2 REGISTER
Contact: <sip:user@local_ip:port>
Max-Forwards: 70
Expires: 3600
Authorization: Digest username="...", realm="...", nonce="...", uri="...", response="..."
User-Agent: Platypus-Phone/0.1.0
Content-Length: 0
```

**Response:**
```
SIP/2.0 200 OK
Contact: <sip:user@local_ip:port>;expires=120
```

### Key Points

- **Call-ID and From tag** must remain the same across both REGISTER requests
- **Branch** must be unique for each request (RFC 3261 requirement)
- **CSeq** increments for each request in the same dialog
- **Via received/rport** parameters added by server show NAT traversal info

---

## Current Implementation Status

### ✅ Completed

1. **UDP Transport**
   - Tokio async UDP socket
   - Ephemeral port binding (0.0.0.0:0)
   - Local IP detection via test connection to 8.8.8.8
   - Bidirectional communication (send/receive)

2. **SIP REGISTER (Registration)**
   - Initial unauthenticated REGISTER
   - Response listening with 10s timeout
   - 401/407 challenge detection
   - WWW-Authenticate header parsing
   - MD5 digest calculation (with/without qop)
   - Authenticated REGISTER with Authorization header
   - 200 OK success detection
   - State management (registered flag)
   - Proper Call-ID and tag management

3. **SIP UNREGISTER (De-registration)**
   - REGISTER with Expires: 0
   - Authentication challenge handling (401/407)
   - Digest authentication for unregister
   - Automatic cleanup on app close
   - Graceful shutdown with proper de-registration

4. **DNS Resolution**
   - Async DNS lookup via tokio::net::lookup_host
   - Fallback to direct IP parsing
   - Default port 5060 if not specified

5. **Application Lifecycle**
   - Window close event interception
   - Async cleanup before exit
   - Proper resource cleanup
   - Zero compiler warnings

### 🚧 In Progress

4. **INVITE (Outbound Calls)**
   - Build INVITE request with SDP
   - Handle provisional responses (100 Trying, 180 Ringing, 183 Progress)
   - Handle 200 OK and send ACK
   - Dialog state management

5. **BYE (Call Termination)**
   - Send BYE request
   - Handle 200 OK response
   - Clean up dialog state

6. **RTP Media**
   - SDP generation and parsing
   - RTP socket creation
   - Audio codec negotiation (G.711, Opus)
   - Audio device enumeration
   - Audio capture and playback

### 📋 Planned

7. **Incoming Calls**
   - Listen for INVITE requests
   - Send 180 Ringing
   - Send 200 OK with SDP
   - Handle ACK

8. **Call Hold/Resume**
   - Re-INVITE with modified SDP
   - Media stream pause/resume

9. **Keep-Alive**
   - Periodic re-REGISTER (before expiry)
   - OPTIONS ping for NAT keep-alive

10. **Error Handling**
    - Network disconnection recovery
    - Registration refresh on failure
    - Call failure handling

---

## SIP Message Format Reference

### INVITE Request

```
INVITE sip:number@server.com SIP/2.0
Via: SIP/2.0/UDP local_ip:port;branch=z9hG4bK{uuid}
From: <sip:user@server.com>;tag={uuid}
To: <sip:number@server.com>
Call-ID: {uuid}
CSeq: 1 INVITE
Contact: <sip:user@local_ip:port>
Max-Forwards: 70
Content-Type: application/sdp
Content-Length: {sdp_length}

v=0
o=- {session_id} {session_version} IN IP4 {local_ip}
s=Platypus Phone Call
c=IN IP4 {local_ip}
t=0 0
m=audio {rtp_port} RTP/AVP 0 8 101
a=rtpmap:0 PCMU/8000
a=rtpmap:8 PCMA/8000
a=rtpmap:101 telephone-event/8000
a=sendrecv
```

### BYE Request

```
BYE sip:number@server.com SIP/2.0
Via: SIP/2.0/UDP local_ip:port;branch=z9hG4bK{uuid}
From: <sip:user@server.com>;tag={from_tag}
To: <sip:number@server.com>;tag={to_tag}
Call-ID: {call_id}
CSeq: {next_seq} BYE
Max-Forwards: 70
User-Agent: Platypus-Phone/0.1.0
Content-Length: 0
```

### ACK Request

```
ACK sip:number@server.com SIP/2.0
Via: SIP/2.0/UDP local_ip:port;branch=z9hG4bK{uuid}
From: <sip:user@server.com>;tag={from_tag}
To: <sip:number@server.com>;tag={to_tag}
Call-ID: {call_id}
CSeq: {invite_seq} ACK
Max-Forwards: 70
Content-Length: 0
```

---

## Dialog State Management

### Dialog Parameters

Each call maintains:
- **Call-ID**: Unique identifier for the call
- **From tag**: Local tag (generated once)
- **To tag**: Remote tag (from 200 OK response)
- **CSeq**: Sequence number (increments per request)
- **Remote URI**: Target of the call
- **Route Set**: Record-Route headers from responses

### State Machine

```
IDLE
  ↓ (send INVITE)
CALLING
  ↓ (receive 180/183)
RINGING
  ↓ (receive 200 OK, send ACK)
CONFIRMED
  ↓ (send/receive BYE)
TERMINATED
```

---

## Network Details

### Local IP Detection

Uses test connection to 8.8.8.8:80 to determine which network interface would be used for internet connectivity. This gives us the correct local IP to advertise in SIP messages.

### NAT Traversal

The server adds `received` and `rport` parameters to Via header:
```
Via: SIP/2.0/UDP 10.100.1.10:43224;received=125.253.43.197;rport=43224
```

This shows:
- **Local IP**: 10.100.1.10 (private)
- **Public IP**: 125.253.43.197 (NAT address)
- **Port**: 43224 (same, no port translation)

---

## Security

### Digest Authentication

- **Algorithm**: MD5 (default)
- **Realm**: Server-provided authentication realm
- **Nonce**: Server-provided random value
- **Response**: Calculated digest proving password knowledge
- **Password**: Never sent in plaintext

### Future Enhancements

- TLS transport (SIP over TLS)
- SRTP (encrypted media)
- OS keychain integration for credential storage

---

## Testing

### Successful Registration Log

```
[SIP] ✓ REGISTER sent successfully
[SIP] Received response from X.X.X.X:5060
SIP/2.0 401 Unauthorized
WWW-Authenticate: Digest realm="...", nonce="..."
[SIP] Authentication required (401/407)
[SIP] Calculating digest...
[SIP] ✓ Authenticated REGISTER sent
[SIP] Final response:
SIP/2.0 200 OK
[SIP] ✓✓✓ Registration successful! ✓✓✓
```

### Test Server

- **Server**: softphone.propel.tech
- **Port**: 5060 (UDP)
- **Resolved IP**: 202.52.129.60
- **Auth**: Digest with MD5
- **Expires**: 120 seconds (server-controlled)

---

## Next Steps

1. Implement INVITE request with SDP
2. Add RTP socket creation
3. Implement ACK response
4. Add BYE request
5. Implement audio device handling
6. Add incoming call support (listen for INVITE)
7. Implement call hold/resume
8. Add periodic re-REGISTER

---

## Code Structure

### Files

- `src-tauri/src/main.rs` - Tauri commands and IPC
- `src-tauri/src/sip.rs` - SIP protocol implementation
- `src/App.tsx` - React UI
- `src/store.ts` - Zustand state management

### Key Functions

- `init_pjsip()` - Initialize UDP socket
- `register_account()` - Complete registration with auth
- `make_call()` - Initiate outbound call (TODO)
- `answer_call()` - Answer incoming call (TODO)
- `hangup_call()` - Terminate call (TODO)

---

## References

- RFC 3261: SIP - Session Initiation Protocol
- RFC 2617: HTTP Digest Authentication
- RFC 4566: SDP - Session Description Protocol
- RFC 3264: Offer/Answer Model with SDP
- RFC 3550: RTP - Real-time Transport Protocol
