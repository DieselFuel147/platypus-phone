use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Audio manager for handling microphone input and speaker output
pub struct AudioManager {
    host: Host,
    input_device: Option<Device>,
    output_device: Option<Device>,
}

impl AudioManager {
    /// Create a new audio manager
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        
        println!("[Audio] Available audio host: {}", host.id().name());

        Ok(Self {
            host,
            input_device: None,
            output_device: None,
        })
    }

    /// List available input devices
    pub fn list_input_devices(&self) -> Result<Vec<String>, String> {
        let devices = self.host
            .input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?;

        let mut device_names = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                device_names.push(name);
            }
        }

        Ok(device_names)
    }

    /// List available output devices
    pub fn list_output_devices(&self) -> Result<Vec<String>, String> {
        let devices = self.host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?;

        let mut device_names = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                device_names.push(name);
            }
        }

        Ok(device_names)
    }

    /// Initialize default input device
    pub fn init_input(&mut self) -> Result<(), String> {
        // Try to get default device
        let device = self.host
            .default_input_device()
            .ok_or_else(|| {
                // List available devices for debugging
                println!("[Audio] No default input device found. Available devices:");
                if let Ok(devices) = self.host.input_devices() {
                    for (i, dev) in devices.enumerate() {
                        if let Ok(name) = dev.name() {
                            println!("[Audio]   {}: {}", i, name);
                        }
                    }
                }
                "No default input device available. Please check your audio configuration.".to_string()
            })?;

        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        println!("[Audio] Using input device: {}", name);

        self.input_device = Some(device);
        Ok(())
    }

    /// Initialize default output device
    pub fn init_output(&mut self) -> Result<(), String> {
        // Try to get default device
        let device = self.host
            .default_output_device()
            .ok_or_else(|| {
                // List available devices for debugging
                println!("[Audio] No default output device found. Available devices:");
                if let Ok(devices) = self.host.output_devices() {
                    for (i, dev) in devices.enumerate() {
                        if let Ok(name) = dev.name() {
                            println!("[Audio]   {}: {}", i, name);
                        }
                    }
                }
                "No default output device available. Please check your audio configuration.".to_string()
            })?;

        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        println!("[Audio] Using output device: {}", name);

        self.output_device = Some(device);
        Ok(())
    }
    
    /// Initialize specific input device by name
    pub fn init_input_by_name(&mut self, device_name: &str) -> Result<(), String> {
        let devices = self.host
            .input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?;

        for device in devices {
            if let Ok(name) = device.name() {
                if name == device_name {
                    println!("[Audio] Using input device: {}", name);
                    self.input_device = Some(device);
                    return Ok(());
                }
            }
        }

        Err(format!("Input device '{}' not found", device_name))
    }

    /// Initialize specific output device by name
    pub fn init_output_by_name(&mut self, device_name: &str) -> Result<(), String> {
        let devices = self.host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?;

        for device in devices {
            if let Ok(name) = device.name() {
                if name == device_name {
                    println!("[Audio] Using output device: {}", name);
                    self.output_device = Some(device);
                    return Ok(());
                }
            }
        }

        Err(format!("Output device '{}' not found", device_name))
    }

    /// Start capturing audio from microphone
    /// Returns a channel receiver that will receive audio samples
    pub fn start_capture(&self) -> Result<(Stream, mpsc::Receiver<Vec<i16>>), String> {
        let device = self.input_device
            .as_ref()
            .ok_or("Input device not initialized")?;

        // Get supported config
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        println!("[Audio] Input config: {:?}", config);

        // We need 8kHz mono for G.711
        let desired_config = StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(8000),
            buffer_size: cpal::BufferSize::Fixed(160), // 20ms at 8kHz
        };

        let (tx, rx) = mpsc::channel(100);

        let err_fn = |err| eprintln!("[Audio] Input stream error: {}", err);

        // Build input stream
        let stream = device
            .build_input_stream(
                &desired_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    // Send audio data through channel
                    let samples = data.to_vec();
                    if let Err(e) = tx.blocking_send(samples) {
                        eprintln!("[Audio] Failed to send audio data: {}", e);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| format!("Failed to build input stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start input stream: {}", e))?;

        println!("[Audio] ✓ Microphone capture started");

        Ok((stream, rx))
    }

    /// Start playing audio to speaker
    /// Returns a channel sender to send audio samples for playback
    pub fn start_playback(&self) -> Result<(Stream, mpsc::Sender<Vec<i16>>), String> {
        let device = self.output_device
            .as_ref()
            .ok_or("Output device not initialized")?;

        // Get supported config
        let config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {}", e))?;

        println!("[Audio] Output config: {:?}", config);

        // We need 8kHz mono for G.711
        let desired_config = StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(8000),
            buffer_size: cpal::BufferSize::Fixed(160), // 20ms at 8kHz
        };

        let (tx, mut rx) = mpsc::channel::<Vec<i16>>(100);
        let buffer = Arc::new(std::sync::Mutex::new(Vec::<i16>::new()));
        let buffer_clone = buffer.clone();

        let err_fn = |err| eprintln!("[Audio] Output stream error: {}", err);

        // Build output stream
        let stream = device
            .build_output_stream(
                &desired_config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    // Try to receive new audio data
                    while let Ok(samples) = rx.try_recv() {
                        let mut buf = buffer_clone.lock().unwrap();
                        buf.extend_from_slice(&samples);
                    }

                    // Fill output buffer
                    let mut buf = buffer_clone.lock().unwrap();
                    let available = buf.len().min(data.len());
                    
                    if available > 0 {
                        data[..available].copy_from_slice(&buf[..available]);
                        buf.drain(..available);
                        
                        // Fill remaining with silence
                        if available < data.len() {
                            data[available..].fill(0);
                        }
                    } else {
                        // No data available, output silence
                        data.fill(0);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start output stream: {}", e))?;

        println!("[Audio] ✓ Speaker playback started");

        Ok((stream, tx))
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new().expect("Failed to create audio manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_manager_creation() {
        let manager = AudioManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_list_devices() {
        let manager = AudioManager::new().unwrap();
        
        let input_devices = manager.list_input_devices();
        println!("Input devices: {:?}", input_devices);
        
        let output_devices = manager.list_output_devices();
        println!("Output devices: {:?}", output_devices);
    }
}
