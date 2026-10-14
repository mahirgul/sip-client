//! Pure Rust G.711 Audio Codec (PCMU / PCMA) and WAV file loader.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use anyhow::{Context, Result};
use hound::WavReader;

/// Converts linear 16-bit PCM sample to 8-bit G.711 u-law (PCMU)
pub fn linear_to_ulaw(sample: i16) -> u8 {
    const BIAS: i16 = 0x84;
    const CLIP: i16 = 32635;

    let mut sign = 0;
    let mut sample = sample;

    if sample < 0 {
        sample = -sample;
        sign = 0x80;
    }

    if sample > CLIP {
        sample = CLIP;
    }

    sample += BIAS;

    let mut exponent = 7;
    let mut mask = 0x4000;
    while (sample & mask) == 0 && exponent > 0 {
        exponent -= 1;
        mask >>= 1;
    }

    let mantissa = (sample >> (exponent + 3)) & 0x0F;
    let ulaw_byte = !(sign | (exponent << 4) | mantissa as u8);
    ulaw_byte
}

/// Converts linear 16-bit PCM sample to 8-bit G.711 A-law (PCMA)
pub fn linear_to_alaw(sample: i16) -> u8 {
    let mut sample = sample;
    let sign = if sample < 0 {
        sample = -sample;
        0
    } else {
        0x80
    };

    let exponent: u8;
    let mantissa: u8;

    if sample >= 4096 {
        exponent = 7;
        mantissa = ((sample >> 7) & 0x0F) as u8;
    } else if sample >= 2048 {
        exponent = 6;
        mantissa = ((sample >> 6) & 0x0F) as u8;
    } else if sample >= 1024 {
        exponent = 5;
        mantissa = ((sample >> 5) & 0x0F) as u8;
    } else if sample >= 512 {
        exponent = 4;
        mantissa = ((sample >> 4) & 0x0F) as u8;
    } else if sample >= 256 {
        exponent = 3;
        mantissa = ((sample >> 3) & 0x0F) as u8;
    } else if sample >= 128 {
        exponent = 2;
        mantissa = ((sample >> 2) & 0x0F) as u8;
    } else if sample >= 64 {
        exponent = 1;
        mantissa = ((sample >> 1) & 0x0F) as u8;
    } else {
        exponent = 0;
        mantissa = (sample & 0x0F) as u8;
    }

    let alaw_byte = (sign | (exponent << 4) | mantissa) ^ 0xD5;
    alaw_byte
}

/// Audio frames source for 20ms RTP packets (160 samples of 8kHz G.711 audio per packet)
pub struct AudioSource {
    pub frames: Vec<Vec<u8>>,
    pub is_loop: bool,
    current_index: usize,
}

impl AudioSource {
    /// Loads a standard .wav file and converts it into 20ms G.711u chunks (160 bytes each)
    pub fn from_wav_file<P: AsRef<Path>>(path: P, is_alaw: bool, is_loop: bool) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open WAV audio file: {:?}", path.as_ref()))?;
        let mut reader = WavReader::new(BufReader::new(file))
            .with_context(|| format!("Invalid WAV format in: {:?}", path.as_ref()))?;

        let spec = reader.spec();
        let sample_rate = spec.sample_rate;
        let channels = spec.channels as usize;

        let mut samples_8k = Vec::new();

        // Read all samples converting to 16-bit mono
        let all_samples: Vec<i16> = match spec.sample_format {
            hound::SampleFormat::Int => {
                match spec.bits_per_sample {
                    8 => reader.samples::<i8>().filter_map(|s| s.ok()).map(|s| (s as i16) << 8).collect(),
                    16 => reader.samples::<i16>().filter_map(|s| s.ok()).collect(),
                    24 => reader.samples::<i32>().filter_map(|s| s.ok()).map(|s| (s >> 8) as i16).collect(),
                    32 => reader.samples::<i32>().filter_map(|s| s.ok()).map(|s| (s >> 16) as i16).collect(),
                    bits => anyhow::bail!("Unsupported bit depth: {bits}"),
                }
            }
            hound::SampleFormat::Float => {
                reader.samples::<f32>()
                    .filter_map(|s| s.ok())
                    .map(|f| (f.clamp(-1.0, 1.0) * 32767.0) as i16)
                    .collect()
            }
        };

        // Downmix stereo/multichannel to mono
        let mono_samples: Vec<i16> = if channels > 1 {
            all_samples
                .chunks(channels)
                .map(|chunk| (chunk.iter().map(|&s| s as i32).sum::<i32>() / channels as i32) as i16)
                .collect()
        } else {
            all_samples
        };

        // Resample to 8000 Hz if necessary
        if sample_rate == 8000 {
            samples_8k = mono_samples;
        } else {
            let step = sample_rate as f64 / 8000.0;
            let mut pos = 0.0;
            while (pos as usize) < mono_samples.len() {
                samples_8k.push(mono_samples[pos as usize]);
                pos += step;
            }
        }

        // Convert 16-bit PCM samples to G.711 (PCMU or PCMA)
        let g711_bytes: Vec<u8> = samples_8k
            .into_iter()
            .map(|s| if is_alaw { linear_to_alaw(s) } else { linear_to_ulaw(s) })
            .collect();

        // Chunk into 160-byte packets (20ms per packet at 8000 Hz)
        let frames: Vec<Vec<u8>> = g711_bytes
            .chunks(160)
            .map(|chunk| {
                if chunk.len() == 160 {
                    chunk.to_vec()
                } else {
                    let mut padded = chunk.to_vec();
                    padded.resize(160, if is_alaw { 0xD5 } else { 0xFF });
                    padded
                }
            })
            .collect();

        log::info!(
            "Loaded audio file: {:?} ({} samples, {} RTP frames, {:.2}s duration)",
            path.as_ref(),
            frames.len() * 160,
            frames.len(),
            (frames.len() * 160) as f32 / 8000.0
        );

        Ok(Self {
            frames,
            is_loop,
            current_index: 0,
        })
    }

    /// Creates a fallback 440Hz test sine tone (3 seconds)
    pub fn test_tone(is_alaw: bool) -> Self {
        const FREQ: f32 = 440.0;
        const DURATION_SECS: f32 = 5.0;
        const TOTAL_SAMPLES: usize = (8000.0 * DURATION_SECS) as usize;

        let mut samples = Vec::with_capacity(TOTAL_SAMPLES);
        for i in 0..TOTAL_SAMPLES {
            let t = i as f32 / 8000.0;
            let val = (t * FREQ * 2.0 * std::f32::consts::PI).sin();
            let pcm = (val * 16384.0) as i16;
            samples.push(if is_alaw { linear_to_alaw(pcm) } else { linear_to_ulaw(pcm) });
        }

        let frames: Vec<Vec<u8>> = samples.chunks(160).map(|c| c.to_vec()).collect();

        Self {
            frames,
            is_loop: true,
            current_index: 0,
        }
    }

    /// Retrieve next 20ms audio frame
    pub fn next_frame(&mut self) -> Option<&[u8]> {
        if self.frames.is_empty() {
            return None;
        }

        if self.current_index >= self.frames.len() {
            if self.is_loop {
                self.current_index = 0;
            } else {
                return None;
            }
        }

        let frame = &self.frames[self.current_index];
        self.current_index += 1;
        Some(frame)
    }

    #[allow(dead_code)]
    pub fn is_finished(&self) -> bool {
        !self.is_loop && self.current_index >= self.frames.len()
    }
}
