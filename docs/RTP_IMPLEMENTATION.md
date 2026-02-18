# RTP Media Implementation - Phase 1

## Summary

Implemented the foundational components for RTP media streaming to enable audio in SIP calls.

## What Was Implemented

### 1. RTP Module (`src-tauri/src/rtp.rs`)

**RTP Packet Handling**:
- ✅ `RtpPacket` struct with full RFC 3550 compliance
- ✅ Packet serialization (`to_bytes()`)
- ✅ Packet deserialization (`from_bytes()`)
- ✅ Support for all RTP header fields (version, padding, extension, CSRC, marker, PT, seq, timestamp, SSRC)

**G.711 Codec Implementation**:
- ✅ PCMU (μ-law) encoder and decoder
- ✅ PCMA (A-law) encoder and decoder
- ✅ 16-bit linear PCM ↔ 8-bit compressed
- ✅ Proper sign, exponent, and mantissa handling
- ✅ Tested with various sample values

**RTP Session Management**:
- ✅ `RtpSession` struct for managing RTP streams
- ✅ UDP socket creation and binding
- ✅ Random SSRC generation
- ✅ Sequence number tracking with wrapping
- ✅ Timestamp management (160 samples per packet for 20ms at 8kHz)
- ✅ `send_audio()` - Send RTP packets with encoded audio
- ✅ `receive_audio()` - Receive and parse RTP packets

**SDP Parsing**:
- ✅ `parse_sdp()` function to extract:
  - Remote IP address from `c=` line
  - Remote port from `m=audio` line
  - Payload type (codec) from media line
- ✅ Handles standard SDP format from SIP 200 OK responses

### 2. Audio Module (`src-tauri/src/audio.rs`)

**Audio Device Management**:
- ✅ `AudioManager` struct using cpal library
- ✅ Cross-platform audio support (Linux/Windows/macOS)
- ✅ `list_input_devices()` - Enumerate microphones
- ✅ `list_output_devices()` - Enumerate speakers
- ✅ `init_input()` - Initialize default microphone
- ✅ `init_output()` - Initialize default speaker

**Audio Capture (Microphone)**:
- ✅ `start_capture()` - Start capturing audio
- ✅ Returns async channel receiver for audio samples
- ✅ Configured for 8kHz mono (G.711 requirements)
- ✅ 160 samples per buffer (20ms frames)
- ✅ 16-bit PCM samples

**Audio Playback (Speaker)**:
- ✅ `start_playback()` - Start playing audio
- ✅ Returns async channel sender for audio samples
- ✅ Configured for 8kHz mono
- ✅ Internal buffering for smooth playback
- ✅ Silence filling when no data available

### 3. Dependencies Added

```toml
cpal = "0.15"  # Cross-platform audio I/O
```

System dependencies installed:
- `libasound2-dev` - ALSA development libraries (Linux)
- `pkg-config` - For finding system libraries

## Architecture

```
Microphone → cpal capture → i16 samples → G.711 encode → RTP packet → UDP → Network
                                                                                ↓
Network → UDP → RTP packet → G.711 decode → i16 samples → cpal playback → Speaker
```

## Technical Details

### RTP Packet Format (RFC 3550)
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|X|  CC   |M|     PT      |       sequence number         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           timestamp                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           synchronization source (SSRC) identifier            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                            payload                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### G.711 Codec
- **PCMU (μ-law)**: Payload type 0, used in North America/Japan
- **PCMA (A-law)**: Payload type 8, used in Europe/rest of world
- **Sample rate**: 8000 Hz
- **Bit depth**: 8 bits (compressed from 16-bit linear PCM)
- **Packet size**: 160 bytes (20ms of audio)
- **Bandwidth**: 64 kbps

### Audio Configuration
- **Sample rate**: 8000 Hz (required for G.711)
- **Channels**: 1 (mono)
- **Buffer size**: 160 samples (20ms frames)
- **Sample format**: i16 (16-bit signed integer)

## What's Next (Phase 2)

### Integration with SIP Module

Need to integrate RTP with the existing SIP call flow:

1. **In `make_call()` after receiving 200 OK**:
   - Parse remote SDP from 200 OK response
   - Create RTP session with remote address/port
   - Start audio capture and playback
   - Spawn tasks for:
     - Capturing audio → Encoding → Sending RTP
     - Receiving RTP → Decoding → Playing audio

2. **Audio Pipeline Tasks**:
   ```rust
   // TX task: Microphone → RTP
   tokio::spawn(async move {
       while let Some(samples) = audio_rx.recv().await {
           let encoded = samples.iter()
               .map(|&s| g711::encode_ulaw(s))
               .collect();
           rtp_session.send_audio(&encoded).await;
       }
   });
   
   // RX task: RTP → Speaker
   tokio::spawn(async move {
       loop {
           let encoded = rtp_session.receive_audio().await;
           let decoded: Vec<i16> = encoded.iter()
               .map(|&b| g711::decode_ulaw(b))
               .collect();
           speaker_tx.send(decoded).await;
       }
   });
   ```

3. **Cleanup on Hangup**:
   - Stop audio streams
   - Close RTP socket
   - Clean up tasks

### Testing Plan

1. **Unit tests** (already included):
   - RTP packet serialization/deserialization ✅
   - G.711 codec encode/decode ✅
   - SDP parsing ✅

2. **Integration tests** (next):
   - Full audio pipeline
   - RTP send/receive
   - Audio device initialization

3. **Real-world test**:
   - Make actual SIP call
   - Verify two-way audio
   - Check audio quality
   - Test with different codecs (PCMU/PCMA)

## Known Limitations

1. **No jitter buffer**: May experience choppy audio on poor networks
2. **No packet loss concealment**: Missing packets = silence
3. **No echo cancellation**: May hear echo
4. **No automatic gain control**: Volume not adjusted
5. **No noise suppression**: Background noise not filtered
6. **Fixed codec**: Only G.711 (PCMU/PCMA)
7. **No DTMF**: RFC 2833 telephone events not implemented

## Future Enhancements

- Jitter buffer for smooth playback
- Packet loss concealment (PLC)
- Comfort noise generation (CNG)
- Echo cancellation (AEC)
- Automatic gain control (AGC)
- Noise suppression
- Additional codecs (Opus, G.722)
- DTMF support (RFC 2833)
- RTCP for quality monitoring

## References

- RFC 3550: RTP - Real-time Transport Protocol
- RFC 3551: RTP Profile for Audio and Video Conferences
- ITU-T G.711: Pulse Code Modulation (PCM) of Voice Frequencies
- cpal documentation: https://docs.rs/cpal/
