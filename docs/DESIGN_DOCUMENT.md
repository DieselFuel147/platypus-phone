# Platypus – Native SIP Desktop Softphone

Architecture Design Document

---

## 1. Project Overview

Platypus is a lightweight cross-platform desktop softphone built using:

* Tauri (Rust backend)
* React (UI)
* Native SIP stack (PJSIP)

It connects directly to a PBX (e.g., MaxoTel) using:

* SIP over UDP
* SIP over TCP
* SIP over TLS
* RTP/SRTP for media

No WebRTC.
No WebSocket.
No SIP proxy server.

---

## 2. Goals

* Direct SIP registration to PBX
* No middleware relay server
* Low resource usage
* Reliable queue answering
* Native audio handling
* Secure credential storage
* Windows + Linux support

---

## 3. Non-Goals

* Browser-based SIP
* WebRTC media
* Mobile support (initially)

---

## 4. Technology Stack

### Frontend

* React
* TypeScript
* Vite
* Zustand (state)

### Desktop Framework

* Tauri

### Backend (Rust)

* PJSIP (native SIP stack)
* OS Keychain integration
* Native notification APIs

---

## 5. Core Capabilities

### SIP Registration

* UDP/TCP/TLS transport
* Automatic re-registration
* Keepalive handling
* NAT traversal

### Media

* RTP audio
* Optional SRTP
* Codec negotiation (G.711, Opus if supported)
* Echo cancellation

### Call Handling

* Inbound
* Outbound
* Hold
* Resume
* Transfer (Phase 2)
* Multiple calls (Phase 2)

---

## 6. System Architecture

```
React UI
   ↓
Tauri IPC
   ↓
Rust Core
   ↓
PJSIP
   ↓
SIP (UDP/TCP/TLS)
   ↓
RTP Media
   ↓
MaxoTel PBX
```

---

## 7. Rust Backend Responsibilities

* Initialise PJSIP
* Manage SIP accounts
* Handle call state
* Emit events to frontend
* Secure credential storage
* Handle reconnect logic
* Handle audio device enumeration

---

## 8. Frontend Responsibilities

* Dialpad
* Call UI
* Account settings
* Device selection UI
* Display call states
* Notification handling

All SIP logic remains backend-side.

---

## 9. Call State Machine

```
UNINITIALIZED
  ↓
INITIALIZED
  ↓
REGISTERING
  ↓
REGISTERED
  ↓
INCOMING / OUTGOING
  ↓
ACTIVE
  ↓
HELD
  ↓
TERMINATED
  ↓
REGISTERED
```

---

## 10. Security Model

* Credentials stored in OS keychain
* TLS supported
* SRTP supported
* No credentials stored in frontend
* No SIP password exposed to JS runtime

---

## 11. Development Phases

### Phase 1 – Core SIP Prototype

* Integrate PJSIP
* Register to MaxoTel
* Make outbound call
* Receive inbound call

### Phase 2 – UI Integration

* Connect Rust events to React
* Display call state
* Add controls

### Phase 3 – Production Hardening

* Transfer
* Multi-call
* Auto reconnect
* Sleep/wake handling

---

## 12. Advantages of This Approach

* Direct PBX integration
* No relay server required
* More reliable than browser SIP
* Full feature parity with Zoiper-style clients
* Better NAT handling
* True desktop behavior
