#[cfg(feature = "audio")]
use anyhow::{Context, Result};
#[cfg(feature = "audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{num_complex::Complex, FftPlanner};
#[cfg(feature = "audio")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "audio")]
use std::io::Read;
#[cfg(feature = "audio")]
use std::process::{Command, Stdio};

#[derive(Clone)]
pub struct AudioData {
    pub spectrum: Vec<f32>,
    pub waveform: Vec<f32>,
}

/// Smoothed audio data with exponential decay for fluid animations
pub struct SmoothedAudio {
    spectrum: Vec<f32>,
    waveform: Vec<f32>,
    attack: f32,  // How fast values rise (0-1, higher = faster)
    decay: f32,   // How fast values fall (0-1, higher = faster)
}

impl SmoothedAudio {
    pub fn new(fft_size: usize, attack: f32, decay: f32) -> Self {
        Self {
            spectrum: vec![0.0; fft_size / 2],
            waveform: vec![0.0; fft_size],
            attack,
            decay,
        }
    }

    pub fn update(&mut self, data: &AudioData) -> AudioData {
        // Smooth spectrum with asymmetric attack/decay
        for (i, &target) in data.spectrum.iter().enumerate() {
            if i < self.spectrum.len() {
                let current = self.spectrum[i];
                if target > current {
                    self.spectrum[i] = current + (target - current) * self.attack;
                } else {
                    self.spectrum[i] = current + (target - current) * self.decay;
                }
            }
        }

        // Waveform uses faster response (it needs to track audio closely)
        for (i, &target) in data.waveform.iter().enumerate() {
            if i < self.waveform.len() {
                self.waveform[i] = self.waveform[i] * 0.3 + target * 0.7;
            }
        }

        AudioData {
            spectrum: self.spectrum.clone(),
            waveform: self.waveform.clone(),
        }
    }
}

#[cfg(feature = "audio")]
pub struct AudioCapture {
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    fft_size: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    // Pre-allocated buffers
    waveform_buf: Vec<f32>,
    fft_buffer: Vec<Complex<f32>>,
    spectrum_buf: Vec<f32>,
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

        // Pre-compute FFT and window
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos()))
            .collect();

        // Pre-allocate buffers
        let waveform_buf = vec![0.0f32; fft_size];
        let fft_buffer = vec![Complex::new(0.0f32, 0.0f32); fft_size];
        let spectrum_buf = vec![0.0f32; fft_size / 2];

        Ok(Self {
            _stream: stream,
            samples,
            fft_size,
            fft,
            window,
            waveform_buf,
            fft_buffer,
            spectrum_buf,
        })
    }

    pub fn get_data(&mut self) -> AudioData {
        // Copy samples with minimal lock time
        {
            let samples = self.samples.lock().unwrap();
            self.waveform_buf.copy_from_slice(&samples);
        }

        // Apply window and prepare FFT input (no allocation)
        for i in 0..self.fft_size {
            self.fft_buffer[i] = Complex::new(self.waveform_buf[i] * self.window[i], 0.0);
        }

        self.fft.process(&mut self.fft_buffer);

        // Compute spectrum magnitudes (no allocation)
        let scale = 1.0 / self.fft_size as f32;
        for i in 0..self.fft_size / 2 {
            let c = &self.fft_buffer[i];
            self.spectrum_buf[i] = (c.re * c.re + c.im * c.im).sqrt() * scale;
        }

        AudioData {
            spectrum: self.spectrum_buf.clone(),
            waveform: self.waveform_buf.clone(),
        }
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

// PulseAudio capture using parec - works with monitor sources
#[cfg(feature = "audio")]
pub struct PulseCapture {
    buffer: Arc<Mutex<RingBuffer>>,
    fft_size: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    // Pre-allocated buffers to avoid per-frame allocations
    waveform_buf: Vec<f32>,
    fft_buffer: Vec<Complex<f32>>,
    spectrum_buf: Vec<f32>,
    _handle: std::thread::JoinHandle<()>,
}

// Lock-free-ish ring buffer for audio samples
#[cfg(feature = "audio")]
struct RingBuffer {
    data: Vec<f32>,
    write_pos: usize,
}

#[cfg(feature = "audio")]
impl RingBuffer {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0.0; size],
            write_pos: 0,
        }
    }

    fn push(&mut self, sample: f32) {
        self.data[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.data.len();
    }

    fn copy_ordered_into(&self, dest: &mut [f32]) {
        let first_part = &self.data[self.write_pos..];
        let second_part = &self.data[..self.write_pos];
        dest[..first_part.len()].copy_from_slice(first_part);
        dest[first_part.len()..].copy_from_slice(second_part);
    }
}

#[cfg(feature = "audio")]
impl PulseCapture {
    pub fn new(fft_size: usize) -> Result<Self> {
        // Get default monitor source
        let output = Command::new("pactl")
            .args(["get-default-sink"])
            .output()
            .context("pactl not found")?;

        if !output.status.success() {
            anyhow::bail!("Failed to get default sink");
        }

        let sink = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let monitor = format!("{}.monitor", sink);

        let buffer = Arc::new(Mutex::new(RingBuffer::new(fft_size)));
        let buffer_clone = buffer.clone();

        // Spawn parec in a thread
        let handle = std::thread::spawn(move || {
            let mut child = match Command::new("parec")
                .args([
                    "--device", &monitor,
                    "--format=float32le",
                    "--channels=1",
                    "--rate=48000",
                    "--latency-msec=10",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => return,
            };

            let mut stdout = match child.stdout.take() {
                Some(s) => s,
                None => return,
            };

            // Small buffer for low latency (64 samples = ~1.3ms at 48kHz)
            let mut buf = [0u8; 256];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // Use try_lock to avoid blocking if main thread is reading
                        if let Ok(mut ring) = buffer_clone.try_lock() {
                            for chunk in buf[..n].chunks_exact(4) {
                                let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                                ring.push(sample);
                            }
                        }
                        // If lock failed, just drop this batch - smoother than blocking
                    }
                    Err(_) => break,
                }
            }
        });

        // Pre-compute FFT and window function
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos()))
            .collect();

        // Pre-allocate buffers
        let waveform_buf = vec![0.0f32; fft_size];
        let fft_buffer = vec![Complex::new(0.0f32, 0.0f32); fft_size];
        let spectrum_buf = vec![0.0f32; fft_size / 2];

        Ok(Self {
            buffer,
            fft_size,
            fft,
            window,
            waveform_buf,
            fft_buffer,
            spectrum_buf,
            _handle: handle,
        })
    }

    pub fn get_data(&mut self) -> AudioData {
        // Try to copy from ring buffer - skip if locked (don't block render)
        if let Ok(ring) = self.buffer.try_lock() {
            ring.copy_ordered_into(&mut self.waveform_buf);
        }

        // Apply window and prepare FFT input (no allocation)
        for i in 0..self.fft_size {
            self.fft_buffer[i] = Complex::new(self.waveform_buf[i] * self.window[i], 0.0);
        }

        self.fft.process(&mut self.fft_buffer);

        // Compute spectrum magnitudes (no allocation)
        let scale = 1.0 / self.fft_size as f32;
        for i in 0..self.fft_size / 2 {
            let c = &self.fft_buffer[i];
            self.spectrum_buf[i] = (c.re * c.re + c.im * c.im).sqrt() * scale;
        }

        AudioData {
            spectrum: self.spectrum_buf.clone(),
            waveform: self.waveform_buf.clone(),
        }
    }
}

pub enum AudioSource {
    #[cfg(feature = "audio")]
    Pulse(PulseCapture),
    #[cfg(feature = "audio")]
    Cpal(AudioCapture),
    Mock(MockAudioCapture),
}

impl AudioSource {
    #[cfg(feature = "audio")]
    pub fn new(device_name: &str, fft_size: usize) -> Self {
        // Try PulseAudio first (works with monitor sources)
        if device_name.is_empty() {
            if let Ok(capture) = PulseCapture::new(fft_size) {
                return AudioSource::Pulse(capture);
            }
        }

        // Fall back to cpal for explicit device names
        match AudioCapture::new(device_name, fft_size) {
            Ok(capture) => AudioSource::Cpal(capture),
            Err(e) => {
                eprintln!("Audio capture failed: {}. Using mock audio.", e);
                AudioSource::Mock(MockAudioCapture::new(fft_size))
            }
        }
    }

    #[cfg(not(feature = "audio"))]
    pub fn new(_device_name: &str, fft_size: usize) -> Self {
        AudioSource::Mock(MockAudioCapture::new(fft_size))
    }

    pub fn get_data(&mut self) -> AudioData {
        match self {
            #[cfg(feature = "audio")]
            AudioSource::Pulse(capture) => capture.get_data(),
            #[cfg(feature = "audio")]
            AudioSource::Cpal(capture) => capture.get_data(),
            AudioSource::Mock(mock) => mock.get_data(),
        }
    }
}
