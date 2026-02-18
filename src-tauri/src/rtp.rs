use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

/// RTP packet structure (RFC 3550)
#[derive(Debug, Clone)]
pub struct RtpPacket {
    pub version: u8,           // 2 bits (always 2)
    pub padding: bool,         // 1 bit
    pub extension: bool,       // 1 bit
    pub csrc_count: u8,        // 4 bits
    pub marker: bool,          // 1 bit
    pub payload_type: u8,      // 7 bits
    pub sequence_number: u16,  // 16 bits
    pub timestamp: u32,        // 32 bits
    pub ssrc: u32,             // 32 bits (synchronization source)
    pub payload: Vec<u8>,      // Variable length
}

impl RtpPacket {
    /// Create a new RTP packet
    pub fn new(payload_type: u8, sequence_number: u16, timestamp: u32, ssrc: u32, payload: Vec<u8>) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            payload,
        }
    }

    /// Serialize RTP packet to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(12 + self.payload.len());

        // Byte 0: V(2), P(1), X(1), CC(4)
        let byte0 = (self.version << 6) | 
                    ((self.padding as u8) << 5) | 
                    ((self.extension as u8) << 4) | 
                    self.csrc_count;
        bytes.push(byte0);

        // Byte 1: M(1), PT(7)
        let byte1 = ((self.marker as u8) << 7) | self.payload_type;
        bytes.push(byte1);

        // Bytes 2-3: Sequence number
        bytes.extend_from_slice(&self.sequence_number.to_be_bytes());

        // Bytes 4-7: Timestamp
        bytes.extend_from_slice(&self.timestamp.to_be_bytes());

        // Bytes 8-11: SSRC
        bytes.extend_from_slice(&self.ssrc.to_be_bytes());

        // Payload
        bytes.extend_from_slice(&self.payload);

        bytes
    }

    /// Parse RTP packet from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 12 {
            return Err("RTP packet too short".to_string());
        }

        let version = (bytes[0] >> 6) & 0x03;
        let padding = (bytes[0] & 0x20) != 0;
        let extension = (bytes[0] & 0x10) != 0;
        let csrc_count = bytes[0] & 0x0F;

        let marker = (bytes[1] & 0x80) != 0;
        let payload_type = bytes[1] & 0x7F;

        let sequence_number = u16::from_be_bytes([bytes[2], bytes[3]]);
        let timestamp = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let ssrc = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        let header_len = 12 + (csrc_count as usize * 4);
        if bytes.len() < header_len {
            return Err("RTP packet header incomplete".to_string());
        }

        let payload = bytes[header_len..].to_vec();

        Ok(Self {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            payload,
        })
    }
}

/// G.711 μ-law (PCMU) codec
pub mod g711 {
    const BIAS: i16 = 0x84;
    const CLIP: i16 = 32635;

    /// Encode 16-bit linear PCM to μ-law
    pub fn encode_ulaw(sample: i16) -> u8 {
        let mut sample = sample;
        
        // Get the sign bit
        let sign = if sample < 0 {
            sample = -sample;
            0x80
        } else {
            0x00
        };

        // Clip the magnitude
        if sample > CLIP {
            sample = CLIP;
        }

        // Add bias
        sample = sample + BIAS;

        // Find the exponent (position of highest set bit)
        let exponent = (7 - sample.leading_zeros().saturating_sub(9)) as u8;

        // Get the mantissa (4 bits after the exponent bit)
        let mantissa = ((sample >> (exponent + 3)) & 0x0F) as u8;

        // Combine sign, exponent, and mantissa
        let ulaw = sign | (exponent << 4) | mantissa;

        // Invert all bits (μ-law convention)
        !ulaw
    }

    /// Decode μ-law to 16-bit linear PCM
    pub fn decode_ulaw(ulaw: u8) -> i16 {
        // Invert all bits
        let ulaw = !ulaw;

        // Extract components
        let sign = (ulaw & 0x80) != 0;
        let exponent = ((ulaw >> 4) & 0x07) as u32;
        let mantissa = (ulaw & 0x0F) as i16;

        // Reconstruct the sample
        let mut sample = ((mantissa << 3) + BIAS) << exponent;
        sample = sample - BIAS;

        if sign {
            -sample
        } else {
            sample
        }
    }

    /// Encode 16-bit linear PCM to A-law
    pub fn encode_alaw(sample: i16) -> u8 {
        let mut sample = sample;
        
        // Get the sign bit
        let sign = if sample < 0 {
            sample = -sample;
            0x00
        } else {
            0x80
        };

        // Clip the magnitude
        if sample > CLIP {
            sample = CLIP;
        }

        let mut alaw: u8;

        if sample < 256 {
            alaw = (sample >> 4) as u8;
        } else {
            // Find the exponent
            let exponent = (7 - sample.leading_zeros().saturating_sub(9)) as u8;
            let mantissa = ((sample >> (exponent + 3)) & 0x0F) as u8;
            alaw = (exponent << 4) | mantissa;
        }

        sign | alaw ^ 0x55
    }

    /// Decode A-law to 16-bit linear PCM
    pub fn decode_alaw(alaw: u8) -> i16 {
        let alaw = alaw ^ 0x55;
        
        let sign = (alaw & 0x80) != 0;
        let exponent = ((alaw >> 4) & 0x07) as u32;
        let mantissa = (alaw & 0x0F) as i16;

        let mut sample = if exponent == 0 {
            (mantissa << 4) + 8
        } else {
            ((mantissa << 4) + 0x108) << (exponent - 1)
        };

        if sign {
            sample
        } else {
            -sample
        }
    }
}

/// RTP session for a call
#[derive(Debug)]
pub struct RtpSession {
    socket: Arc<UdpSocket>,
    remote_addr: std::net::SocketAddr,
    local_port: u16,
    ssrc: u32,
    sequence_number: Arc<Mutex<u16>>,
    timestamp: Arc<Mutex<u32>>,
    payload_type: u8, // 0 = PCMU, 8 = PCMA
}

impl RtpSession {
    /// Create a new RTP session
    pub async fn new(
        local_port: u16,
        remote_addr: std::net::SocketAddr,
        payload_type: u8,
    ) -> Result<Self, String> {
        // Bind UDP socket for RTP
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", local_port))
            .await
            .map_err(|e| format!("Failed to bind RTP socket: {}", e))?;

        println!("[RTP] Socket bound to 0.0.0.0:{}", local_port);
        println!("[RTP] Remote address: {}", remote_addr);

        // Generate random SSRC
        let ssrc = rand::random::<u32>();

        Ok(Self {
            socket: Arc::new(socket),
            remote_addr,
            local_port,
            ssrc,
            sequence_number: Arc::new(Mutex::new(rand::random_u16())),
            timestamp: Arc::new(Mutex::new(0)),
            payload_type,
        })
    }

    /// Send RTP packet with audio payload
    pub async fn send_audio(&self, audio_data: &[u8]) -> Result<(), String> {
        let mut seq = self.sequence_number.lock().await;
        let mut ts = self.timestamp.lock().await;

        let packet = RtpPacket::new(
            self.payload_type,
            *seq,
            *ts,
            self.ssrc,
            audio_data.to_vec(),
        );

        let bytes = packet.to_bytes();
        
        self.socket
            .send_to(&bytes, self.remote_addr)
            .await
            .map_err(|e| format!("Failed to send RTP packet: {}", e))?;

        // Increment sequence number
        *seq = seq.wrapping_add(1);
        
        // Increment timestamp (160 samples for 20ms at 8kHz)
        *ts = ts.wrapping_add(160);

        Ok(())
    }

    /// Receive RTP packet
    pub async fn receive_audio(&self) -> Result<Vec<u8>, String> {
        let mut buf = vec![0u8; 2048];
        
        let (size, _) = self.socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| format!("Failed to receive RTP packet: {}", e))?;

        buf.truncate(size);

        let packet = RtpPacket::from_bytes(&buf)?;
        
        Ok(packet.payload)
    }

    /// Get local port
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// Get socket for async operations
    pub fn socket(&self) -> Arc<UdpSocket> {
        self.socket.clone()
    }
}

/// Parse SDP to extract remote RTP address and port
pub fn parse_sdp(sdp: &str) -> Result<(String, u16, u8), String> {
    let mut remote_ip: Option<String> = None;
    let mut remote_port: Option<u16> = None;
    let mut payload_type: u8 = 0; // Default to PCMU

    for line in sdp.lines() {
        let line = line.trim();
        
        // Connection line: c=IN IP4 <address>
        if line.starts_with("c=") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                remote_ip = Some(parts[2].to_string());
            }
        }
        
        // Media line: m=audio <port> RTP/AVP <payload_types>
        if line.starts_with("m=audio") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                remote_port = parts[1].parse().ok();
                // Get first payload type
                if let Some(pt) = parts.get(3) {
                    payload_type = pt.parse().unwrap_or(0);
                }
            }
        }
    }

    let ip = remote_ip.ok_or("No connection address in SDP")?;
    let port = remote_port.ok_or("No media port in SDP")?;

    println!("[RTP] Parsed SDP: {}:{}, payload type: {}", ip, port, payload_type);

    Ok((ip, port, payload_type))
}

// Helper function to generate random numbers (simple implementation)
mod rand {
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn random<T>() -> T 
    where
        T: From<u32>
    {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        T::from(nanos)
    }

    // Specialized version for u16
    pub fn random_u16() -> u16 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        (nanos & 0xFFFF) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtp_packet_serialization() {
        let packet = RtpPacket::new(0, 1234, 5678, 9012, vec![1, 2, 3, 4]);
        let bytes = packet.to_bytes();
        let parsed = RtpPacket::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.payload_type, 0);
        assert_eq!(parsed.sequence_number, 1234);
        assert_eq!(parsed.timestamp, 5678);
        assert_eq!(parsed.ssrc, 9012);
        assert_eq!(parsed.payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_g711_ulaw_codec() {
        let samples = vec![0i16, 100, -100, 1000, -1000, 10000, -10000];
        
        for sample in samples {
            let encoded = g711::encode_ulaw(sample);
            let decoded = g711::decode_ulaw(encoded);
            
            // G.711 is lossy, so we check if it's close enough
            let diff = (sample - decoded).abs();
            assert!(diff < 100, "Sample {} decoded to {} (diff: {})", sample, decoded, diff);
        }
    }

    #[test]
    fn test_sdp_parsing() {
        let sdp = "v=0\r\n\
                   o=root 123 456 IN IP4 192.168.1.1\r\n\
                   s=Test\r\n\
                   c=IN IP4 192.168.1.100\r\n\
                   t=0 0\r\n\
                   m=audio 12345 RTP/AVP 0 8 101\r\n";

        let (ip, port, pt) = parse_sdp(sdp).unwrap();
        assert_eq!(ip, "192.168.1.100");
        assert_eq!(port, 12345);
        assert_eq!(pt, 0);
    }
}
