use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::Mutex;

/// High-quality audio resampler using the rubato crate
/// Handles conversion between 48kHz (typical audio device) and 8kHz (VoIP standard)
pub struct AudioResampler {
    /// Resampler for downsampling (48kHz → 8kHz) for TX
    downsampler: Mutex<SincFixedIn<f32>>,
    /// Resampler for upsampling (8kHz → 48kHz) for RX
    upsampler: Mutex<SincFixedIn<f32>>,
}

impl AudioResampler {
    /// Create a new audio resampler
    /// 
    /// # Arguments
    /// * `input_rate` - Input sample rate (typically 48000 Hz for audio devices)
    /// * `output_rate` - Output sample rate (typically 8000 Hz for VoIP)
    /// * `chunk_size` - Number of samples per chunk (e.g., 960 for 20ms at 48kHz)
    pub fn new(input_rate: u32, output_rate: u32, chunk_size: usize) -> Result<Self, String> {
        // Create high-quality sinc interpolation parameters for downsampler
        let downsample_params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        // Create downsampler (48kHz → 8kHz)
        let downsampler = SincFixedIn::<f32>::new(
            output_rate as f64 / input_rate as f64,
            2.0, // max_resample_ratio_relative
            downsample_params,
            chunk_size,
            1, // mono channel
        )
        .map_err(|e| format!("Failed to create downsampler: {:?}", e))?;

        // Create high-quality sinc interpolation parameters for upsampler
        let upsample_params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        // Create upsampler (8kHz → 48kHz)
        // For upsampling, chunk_size should be for the input (8kHz)
        let upsample_chunk_size = (chunk_size as f64 * output_rate as f64 / input_rate as f64) as usize;
        let upsampler = SincFixedIn::<f32>::new(
            input_rate as f64 / output_rate as f64,
            2.0, // max_resample_ratio_relative
            upsample_params,
            upsample_chunk_size,
            1, // mono channel
        )
        .map_err(|e| format!("Failed to create upsampler: {:?}", e))?;

        tracing::info!(
            "[Resample] Created resampler: {}Hz ↔ {}Hz, chunk_size={}",
            input_rate,
            output_rate,
            chunk_size
        );

        Ok(Self {
            downsampler: Mutex::new(downsampler),
            upsampler: Mutex::new(upsampler),
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

        // Convert i16 to f32 for processing
        let input_f32: Vec<f32> = input.iter().map(|&s| s as f32 / 32768.0).collect();

        // Prepare input as 2D vector (channels x samples)
        let input_frames = vec![input_f32];

        // Process through resampler
        let mut downsampler = self
            .downsampler
            .lock()
            .map_err(|e| format!("Failed to lock downsampler: {}", e))?;

        let output_frames = downsampler
            .process(&input_frames, None)
            .map_err(|e| format!("Downsampling failed: {:?}", e))?;

        // Convert back to i16
        let output: Vec<i16> = output_frames[0]
            .iter()
            .map(|&s| (s * 32768.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

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

        // Convert i16 to f32 for processing
        let input_f32: Vec<f32> = input.iter().map(|&s| s as f32 / 32768.0).collect();

        // Prepare input as 2D vector (channels x samples)
        let input_frames = vec![input_f32];

        // Process through resampler
        let mut upsampler = self
            .upsampler
            .lock()
            .map_err(|e| format!("Failed to lock upsampler: {}", e))?;

        let output_frames = upsampler
            .process(&input_frames, None)
            .map_err(|e| format!("Upsampling failed: {:?}", e))?;

        // Convert back to i16
        let output: Vec<i16> = output_frames[0]
            .iter()
            .map(|&s| (s * 32768.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        tracing::debug!(
            "[Resample] Upsampled {} → {} samples",
            input.len(),
            output.len()
        );

        Ok(output)
    }
}

/// Simple resampler that uses basic interpolation (fallback if rubato fails)
pub struct SimpleResampler {
    input_rate: u32,
    output_rate: u32,
}

impl SimpleResampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            input_rate,
            output_rate,
        }
    }

    /// Downsample using linear interpolation
    pub fn downsample(&self, input: &[i16]) -> Vec<i16> {
        if input.is_empty() {
            return Vec::new();
        }

        let ratio = self.input_rate as f64 / self.output_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_pos = i as f64 * ratio;
            let src_idx = src_pos as usize;
            let frac = src_pos - src_idx as f64;

            if src_idx + 1 < input.len() {
                // Linear interpolation
                let sample1 = input[src_idx] as f64;
                let sample2 = input[src_idx + 1] as f64;
                let interpolated = sample1 + (sample2 - sample1) * frac;
                output.push(interpolated as i16);
            } else if src_idx < input.len() {
                output.push(input[src_idx]);
            }
        }

        output
    }

    /// Upsample using linear interpolation
    pub fn upsample(&self, input: &[i16]) -> Vec<i16> {
        if input.is_empty() {
            return Vec::new();
        }

        let ratio = self.output_rate as f64 / self.input_rate as f64;
        let output_len = (input.len() as f64 * ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_pos = i as f64 / ratio;
            let src_idx = src_pos as usize;
            let frac = src_pos - src_idx as f64;

            if src_idx + 1 < input.len() {
                // Linear interpolation
                let sample1 = input[src_idx] as f64;
                let sample2 = input[src_idx + 1] as f64;
                let interpolated = sample1 + (sample2 - sample1) * frac;
                output.push(interpolated as i16);
            } else if src_idx < input.len() {
                output.push(input[src_idx]);
            }
        }

        output
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
    fn test_upsample() {
        let resampler = AudioResampler::new(48000, 8000, 960).unwrap();
        
        // Create 160 samples at 8kHz (20ms)
        let input: Vec<i16> = (0..160).map(|i| (i * 100) as i16).collect();
        
        let output = resampler.upsample(&input).unwrap();
        
        // Should produce ~960 samples at 48kHz (20ms)
        assert!(output.len() >= 900 && output.len() <= 1000);
    }

    #[test]
    fn test_simple_resampler() {
        let resampler = SimpleResampler::new(48000, 8000);
        
        // Create 960 samples at 48kHz
        let input: Vec<i16> = (0..960).map(|i| (i * 100) as i16).collect();
        
        let output = resampler.downsample(&input);
        
        // Should produce 160 samples at 8kHz
        assert_eq!(output.len(), 160);
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
