# Platypus Phone

Lightweight WebRTC-based desktop softphone built with React, SIP.js, and Tauri.

Platypus is a cross-platform SIP client designed to register directly to a PBX using standard SIP credentials. It uses WebRTC for secure media handling and is intended to provide a reliable, minimal-footprint alternative to traditional desktop softphone applications.

---

## Overview

Platypus is being built as:

* A browser-based WebRTC SIP client (initial prototype)
* Wrapped with Tauri for native desktop support (Windows & Linux)
* PBX-agnostic and standards-compliant
* Focused on reliability for queue-based call environments

The PBX backend handles:

* Call routing
* SIP trunks
* Extensions
* Queues
* IVRs
* Voicemail

Platypus functions purely as a SIP endpoint.

---

## Core Technology

Frontend:

* React
* TypeScript
* Vite

SIP & Media:

* SIP.js
* WebRTC (SRTP / DTLS / ICE)

Desktop Wrapper:

* Tauri (Rust backend)

---

## Current Status

Early development.

Initial focus:

* SIP registration
* Inbound and outbound calls
* Basic call controls
* WebRTC media validation

Tauri integration and production hardening will follow once the browser prototype is validated.

---

## Goals

* Lightweight desktop softphone
* Secure by default (WSS + SRTP)
* Low resource usage
* Cross-platform (Windows + Linux)
* Designed for future expansion

---

## Future Direction

* Native system tray integration
* Secure credential storage
* Call transfer & multi-call support
* Mobile client (future phase)

---

More detailed technical documentation and build instructions will be added as implementation progresses.
