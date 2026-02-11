#[cfg(feature = "audio")]
use anyhow::{Context, Result};
#[cfg(feature = "audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
#[cfg(feature = "audio")]
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AudioData {
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
}

#[cfg(feature = "audio")]
pub struct AudioCapture {
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    fft_size: usize,
}

#[cfg(feature = "audio")]
fn get_default_monitor_source() -> Option<String> {
    // Try to get the default sink's monitor source using pactl
    let output = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()?;

    if output.status.success() {
        let sink_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !sink_name.is_empty() {
            return Some(format!("{}.monitor", sink_name));
        }
    }
    None
}

#[cfg(feature = "audio")]
impl AudioCapture {
    pub fn new(device_name: &str, fft_size: usize) -> Result<Self> {
        let host = cpal::default_host();

        let device = if !device_name.is_empty() {
            // User specified a device
            host.input_devices()?
                .find(|d| d.name().map(|n| n.contains(device_name)).unwrap_or(false))
                .context(format!("Device '{}' not found", device_name))?
        } else {
            // Auto-detect: try monitor source first, then any monitor, then default
            let monitor_name = get_default_monitor_source();

            let device = if let Some(ref monitor) = monitor_name {
                host.input_devices()?
                    .find(|d| d.name().map(|n| n.contains(monitor)).unwrap_or(false))
            } else {
                None
            };

            // If no default monitor found, try any device with "monitor" in the name
            let device = device.or_else(|| {
                host.input_devices().ok()?.find(|d| {
                    d.name()
                        .map(|n| n.to_lowercase().contains("monitor"))
                        .unwrap_or(false)
                })
            });

            // Fall back to default input device
            device
                .or_else(|| host.default_input_device())
                .context("No audio input device available")?
        };

        let config = device.default_input_config()?;
        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();

        let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(vec![0.0; fft_size]));
        let samples_clone = samples.clone();

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut buffer = samples_clone.lock().unwrap();
                    for &sample in data {
                        buffer.push(sample);
                        if buffer.len() > fft_size {
                            buffer.remove(0);
                        }
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut buffer = samples_clone.lock().unwrap();
                    for &sample in data {
                        let f = sample as f32 / i16::MAX as f32;
                        buffer.push(f);
                        if buffer.len() > fft_size {
                            buffer.remove(0);
                        }
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let mut buffer = samples_clone.lock().unwrap();
                    for &sample in data {
                        let f = (sample as f32 / u16::MAX as f32) * 2.0 - 1.0;
                        buffer.push(f);
                        if buffer.len() > fft_size {
                            buffer.remove(0);
                        }
                    }
                },
                err_fn,
                None,
            )?,
            _ => anyhow::bail!("Unsupported sample format"),
        };

        stream.play()?;

        Ok(Self {
            _stream: stream,
            samples,
            fft_size,
        })
    }

    pub fn get_data(&self) -> AudioData {
        let samples = self.samples.lock().unwrap();
        let waveform = samples.clone();

        // Compute FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.fft_size);

        let mut buffer: Vec<Complex<f32>> = samples
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        // Apply Hann window
        for (i, sample) in buffer.iter_mut().enumerate() {
            let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / self.fft_size as f32).cos());
            sample.re *= window;
        }

        fft.process(&mut buffer);

        // Convert to magnitude (only positive frequencies)
        let spectrum: Vec<f32> = buffer[..self.fft_size / 2]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt() / self.fft_size as f32)
            .collect();

        AudioData { spectrum, waveform }
    }
}

// Mock audio for when cpal is not available or no device found
pub struct MockAudioCapture {
    phase: f32,
    fft_size: usize,
}

impl MockAudioCapture {
    pub fn new(fft_size: usize) -> Self {
        Self { phase: 0.0, fft_size }
    }

    pub fn get_data(&mut self) -> AudioData {
        self.phase += 0.1;

        // Generate mock waveform (sine wave with harmonics)
        let waveform: Vec<f32> = (0..self.fft_size)
            .map(|i| {
                let t = i as f32 / self.fft_size as f32;
                (self.phase + t * 10.0).sin() * 0.5
                    + (self.phase * 2.3 + t * 25.0).sin() * 0.25
                    + (self.phase * 0.7 + t * 5.0).sin() * 0.15
            })
            .collect();

        // Compute real FFT on the mock waveform
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.fft_size);

        let mut buffer: Vec<Complex<f32>> = waveform
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        // Apply Hann window
        for (i, sample) in buffer.iter_mut().enumerate() {
            let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / self.fft_size as f32).cos());
            sample.re *= window;
        }

        fft.process(&mut buffer);

        let spectrum: Vec<f32> = buffer[..self.fft_size / 2]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt() / self.fft_size as f32)
            .collect();

        AudioData { spectrum, waveform }
    }
}

pub enum AudioSource {
    #[cfg(feature = "audio")]
    Real(AudioCapture),
    Mock(MockAudioCapture),
}

impl AudioSource {
    #[cfg(feature = "audio")]
    pub fn new(device_name: &str, fft_size: usize) -> Self {
        match AudioCapture::new(device_name, fft_size) {
            Ok(capture) => AudioSource::Real(capture),
            Err(_) => AudioSource::Mock(MockAudioCapture::new(fft_size)),
        }
    }

    #[cfg(not(feature = "audio"))]
    pub fn new(_device_name: &str, fft_size: usize) -> Self {
        AudioSource::Mock(MockAudioCapture::new(fft_size))
    }

    pub fn get_data(&mut self) -> AudioData {
        match self {
            #[cfg(feature = "audio")]
            AudioSource::Real(capture) => capture.get_data(),
            AudioSource::Mock(mock) => mock.get_data(),
        }
    }
}
