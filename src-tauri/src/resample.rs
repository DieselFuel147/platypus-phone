use std::sync::Mutex;

/// Audio resampler using linear interpolation
/// Handles conversion between 48kHz (typical audio device) and 8kHz (VoIP standard)
/// Simple but effective - works with any buffer size
pub struct AudioResampler {
    input_rate: u32,
    output_rate: u32,
    /// Position tracker for downsampling (to maintain phase across chunks)
    downsample_position: Mutex<f64>,
}

impl AudioResampler {
    /// Create a new audio resampler
    /// 
    /// # Arguments
    /// * `input_rate` - Input sample rate (typically 48000 Hz for audio devices)
    /// * `output_rate` - Output sample rate (typically 8000 Hz for VoIP)
    /// * `_chunk_size` - Unused, kept for API compatibility
    pub fn new(input_rate: u32, output_rate: u32, _chunk_size: usize) -> Result<Self, String> {
        tracing::info!(
            "[Resample] Created resampler: {}Hz ↔ {}Hz (linear interpolation)",
            input_rate,
            output_rate
        );

        Ok(Self {
            input_rate,
            output_rate,
            downsample_position: Mutex::new(0.0),
        })
    }

    /// Downsample audio from high sample rate to low sample rate (e.g., 48kHz → 8kHz)
    /// Used for TX path: Microphone → Network
    /// 
    /// # Arguments
    /// * `input` - Input samples at high sample rate (i16 format)
    /// 
    /// # Returns
    /// * Downsampled audio at low sample rate (i16 format)
    pub fn downsample(&self, input: &[i16]) -> Result<Vec<i16>, String> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let ratio = self.input_rate as f64 / self.output_rate as f64;
        let output_len = (input.len() as f64 / ratio).floor() as usize;
        let mut output = Vec::with_capacity(output_len);

        let mut position = self.downsample_position.lock()
            .map_err(|e| format!("Failed to lock position: {}", e))?;

        for _ in 0..output_len {
            let src_idx = (*position).floor() as usize;
            let frac = *position - (*position).floor();

            if src_idx + 1 < input.len() {
                // Linear interpolation
                let sample1 = input[src_idx] as f64;
                let sample2 = input[src_idx + 1] as f64;
                let interpolated = sample1 + (sample2 - sample1) * frac;
                output.push(interpolated.clamp(-32768.0, 32767.0) as i16);
            } else if src_idx < input.len() {
                output.push(input[src_idx]);
            }

            *position += ratio;
        }

        // Keep fractional part for next chunk
        *position -= input.len() as f64;
        if *position < 0.0 {
            *position = 0.0;
        }

        tracing::debug!(
            "[Resample] Downsampled {} → {} samples",
            input.len(),
            output.len()
        );

        Ok(output)
    }

    /// Upsample audio from low sample rate to high sample rate (e.g., 8kHz → 48kHz)
    /// Used for RX path: Network → Speaker
    /// 
    /// # Arguments
    /// * `input` - Input samples at low sample rate (i16 format)
    /// 
    /// # Returns
    /// * Upsampled audio at high sample rate (i16 format)
    pub fn upsample(&self, input: &[i16]) -> Result<Vec<i16>, String> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let ratio = self.output_rate as f64 / self.input_rate as f64;
        let output_len = (input.len() as f64 * ratio).floor() as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_pos = i as f64 / ratio;
            let src_idx = src_pos.floor() as usize;
            let frac = src_pos - src_pos.floor();

            if src_idx + 1 < input.len() {
                // Linear interpolation
                let sample1 = input[src_idx] as f64;
                let sample2 = input[src_idx + 1] as f64;
                let interpolated = sample1 + (sample2 - sample1) * frac;
                output.push(interpolated.clamp(-32768.0, 32767.0) as i16);
            } else if src_idx < input.len() {
                output.push(input[src_idx]);
            }
        }

        tracing::debug!(
            "[Resample] Upsampled {} → {} samples",
            input.len(),
            output.len()
        );

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_resampler_creation() {
        let resampler = AudioResampler::new(48000, 8000, 960);
        assert!(resampler.is_ok());
    }

    #[test]
    fn test_downsample() {
        let resampler = AudioResampler::new(48000, 8000, 960).unwrap();
        
        // Create 960 samples at 48kHz (20ms)
        let input: Vec<i16> = (0..960).map(|i| (i * 100) as i16).collect();
        
        let output = resampler.downsample(&input).unwrap();
        
        // Should produce ~160 samples at 8kHz (20ms)
        assert!(output.len() >= 150 && output.len() <= 170);
    }

    #[test]
    fn test_downsample_variable_sizes() {
        let resampler = AudioResampler::new(48000, 8000, 960).unwrap();
        
        // Test with 480 samples (10ms)
        let input1: Vec<i16> = (0..480).map(|i| (i * 100) as i16).collect();
        let output1 = resampler.downsample(&input1).unwrap();
        assert!(output1.len() >= 75 && output1.len() <= 85);
        
        // Test with 240 samples (5ms)
        let input2: Vec<i16> = (0..240).map(|i| (i * 100) as i16).collect();
        let output2 = resampler.downsample(&input2).unwrap();
        assert!(output2.len() >= 35 && output2.len() <= 45);
    }

    #[test]
    fn test_upsample() {
        let resampler = AudioResampler::new(48000, 8000, 960).unwrap();
        
        // Create 160 samples at 8kHz (20ms)
        let input: Vec<i16> = (0..160).map(|i| (i * 100) as i16).collect();
        
        let output = resampler.upsample(&input).unwrap();
        
        // Should produce ~960 samples at 48kHz (20ms)
        assert!(output.len() >= 900 && output.len() <= 1000);
    }

    #[test]
    fn test_empty_input() {
        let resampler = AudioResampler::new(48000, 8000, 960).unwrap();
        
        let output = resampler.downsample(&[]).unwrap();
        assert_eq!(output.len(), 0);
        
        let output = resampler.upsample(&[]).unwrap();
        assert_eq!(output.len(), 0);
    }
}
