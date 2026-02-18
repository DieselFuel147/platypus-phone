# Platypus

Lightweight native desktop SIP softphone built with Tauri, Rust, and React.

Platypus connects directly to a PBX using standard SIP over UDP, TCP, or TLS. It is designed as a minimal, secure, and reliable alternative to traditional desktop softphones.

---

## Overview

Platypus is a cross-platform desktop application that:

* Registers directly to a SIP PBX
* Uses native RTP audio (no WebRTC)
* Runs on Windows and Linux
* Stores credentials securely using OS keychain
* Requires no relay or proxy server

It functions purely as a SIP endpoint. All call routing and telephony logic is handled by the PBX.

---

## Technology Stack

Frontend:

* React
* TypeScript
* Vite

Desktop Framework:

* Tauri

Backend:

* Rust
* PJSIP (native SIP stack)

---

## Quick Start

### Prerequisites

- Node.js (v18+)
- Rust (latest stable)
- System dependencies for Tauri:
  - **Linux**: `sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Microsoft Visual Studio C++ Build Tools

### Development

1. Install dependencies:
```bash
npm install
```

2. Run in development mode:
```bash
npm run tauri dev
```

## Status

**Minimal prototype** - verifying basic stack integration.

**What works:**
- ✅ Tauri app launches
- ✅ React UI with dialpad
- ✅ State management with Zustand
- ✅ Rust backend with Tauri commands
- ✅ Event communication between frontend and backend
- ✅ Basic call state machine UI

**What's implemented:**
- ✅ Direct UDP socket communication (tokio)
- ✅ UDP transport layer with DNS resolution
- ✅ SIP REGISTER with full digest authentication (MD5)
- ✅ Response listening and parsing
- ✅ 401/407 authentication challenge handling
- ✅ Automatic de-registration on app close
- ✅ Clean shutdown handling

**What's in progress:**
- ⚠️ INVITE request (outbound calls)
- ⚠️ ACK response
- ⚠️ BYE request (call termination)
- ⚠️ RTP/SRTP media streams
- ⚠️ Audio device management
- ⚠️ Incoming call handling

### Next Steps

1. Implement SIP response listener and parser
2. Add digest authentication for REGISTER
3. Implement INVITE request with SDP
4. Add RTP media handling
5. Implement audio device enumeration and selection
6. Add secure credential storage (OS keychain)

See `docs/DESIGN_DOCUMENT.md` for full architecture details.