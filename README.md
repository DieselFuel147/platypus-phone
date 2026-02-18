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

## Status

Early development.

Initial focus:

* SIP registration
* Inbound and outbound calls
* Native audio validation

More detailed build and architecture documentation will be added as implementation progresses.