use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pitch_detection::detector::PitchDetector;
use pitch_detection::detector::mcleod::McLeodDetector;

const PITCH_WINDOW: usize = 2048;
const MIN_PITCH_HZ: f32 = 80.0;
const MAX_PITCH_HZ: f32 = 1000.0;
const PITCH_POWER_THRESHOLD: f32 = 0.2;
const PITCH_CLARITY_THRESHOLD: f32 = 0.4;
const MIC_RMS_GATE: f32 = 0.012;
const REF_RMS_GATE: f32 = 0.005;

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

pub struct MicPitchData {
    pub latest_pitch: Option<f32>,
    samples: VecDeque<f32>,
    sample_rate: u32,
}

impl MicPitchData {
    fn new(sample_rate: u32) -> Self {
        Self {
            latest_pitch: None,
            samples: VecDeque::with_capacity(PITCH_WINDOW * 2),
            sample_rate,
        }
    }
}

#[derive(Resource)]
pub struct MicrophoneCapture {
    pub active: bool,
    pub device_name: String,
    shared: Arc<Mutex<MicPitchData>>,
    _stream: Option<cpal::Stream>,
    stream_error: Arc<AtomicBool>,
    sample_counter: Arc<AtomicU64>,
    last_checked_count: u64,
    stale_ticks: u32,
    shutdown: Arc<AtomicBool>,
    pitch_thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MicrophoneCapture {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.pitch_thread.take() {
            let _ = handle.join();
        }
    }
}

const STALE_THRESHOLD_TICKS: u32 = 60;

impl MicrophoneCapture {
    pub fn latest_pitch(&self) -> Option<f32> {
        if !self.active {
            return None;
        }
        self.shared.lock().ok()?.latest_pitch
    }

    pub fn has_stream(&self) -> bool {
        self._stream.is_some()
    }

    pub fn check_health(&mut self) -> bool {
        if !self.active || !self.has_stream() {
            return true;
        }
        if self.stream_error.load(Ordering::Relaxed) {
            return false;
        }
        let count = self.sample_counter.load(Ordering::Relaxed);
        if count == self.last_checked_count {
            self.stale_ticks += 1;
        } else {
            self.stale_ticks = 0;
            self.last_checked_count = count;
        }
        self.stale_ticks < STALE_THRESHOLD_TICKS
    }
}

fn device_display_name(device: &cpal::Device) -> String {
    let Ok(desc) = device.description() else {
        return "(unknown)".into();
    };
    if let Some(friendly) = desc.extended().first() {
        return friendly.clone();
    }
    desc.to_string()
}

pub fn available_devices() -> Vec<String> {
    let host = cpal::default_host();
    let Some(devices) = host.input_devices().ok() else {
        return vec![];
    };
    let mut seen = HashSet::new();
    devices
        .filter(|d| d.default_input_config().is_ok())
        .map(|d| device_display_name(&d))
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

const VIRTUAL_DEVICE_PATTERNS: &[&str] = &[
    "JACK",
    "PulseAudio",
    "PipeWire",
    "Default ALSA",
    "Discard all samples",
    "Open Sound System",
];

fn is_virtual_device(name: &str) -> bool {
    VIRTUAL_DEVICE_PATTERNS.iter().any(|pat| name.contains(pat))
}

fn rank_devices(host: &cpal::Host, preferred: Option<&str>, fallback: bool) -> Vec<cpal::Device> {
    let Some(all_devices) = host.input_devices().ok() else {
        return vec![];
    };

    let mut seen = HashSet::new();
    let mut preferred_dev = None;
    let mut hardware: Vec<(cpal::Device, u32)> = Vec::new();
    let mut virtual_devs: Vec<(cpal::Device, u32)> = Vec::new();

    for dev in all_devices {
        let name = device_display_name(&dev);
        let Ok(cfg) = dev.default_input_config() else {
            continue;
        };
        if !seen.insert(name.clone()) {
            continue;
        }
        let sr = cfg.sample_rate();
        info!("  Available mic: '{name}' ({sr}Hz)");

        if preferred.is_some_and(|p| p == name) && preferred_dev.is_none() {
            preferred_dev = Some(dev);
        } else if is_virtual_device(&name) {
            virtual_devs.push((dev, sr));
        } else {
            hardware.push((dev, sr));
        }
    }

    let mut result = Vec::new();
    if let Some(pref) = preferred_dev {
        result.push(pref);
        if !fallback {
            return result;
        }
    } else if preferred.is_some() {
        if !fallback {
            return result;
        }
        warn!(
            "Preferred mic '{}' not found, auto-selecting",
            preferred.unwrap()
        );
    }

    hardware.sort_by(|a, b| b.1.cmp(&a.1));
    virtual_devs.sort_by(|a, b| b.1.cmp(&a.1));
    result.extend(hardware.into_iter().map(|(d, _)| d));
    result.extend(virtual_devs.into_iter().map(|(d, _)| d));

    result
}

pub fn start_microphone(preferred: Option<&str>, fallback: bool) -> MicrophoneCapture {
    let shutdown = Arc::new(AtomicBool::new(false));
    let (shared, stream, name, error_flag, counter, pitch_thread) =
        try_build_stream(preferred, fallback, Arc::clone(&shutdown));

    let shared = shared.unwrap_or_else(|| {
        warn!("No microphone found or permission denied; scoring disabled");
        Arc::new(Mutex::new(MicPitchData::new(44100)))
    });

    MicrophoneCapture {
        active: stream.is_some(),
        device_name: name,
        shared,
        _stream: stream,
        stream_error: error_flag,
        sample_counter: counter,
        last_checked_count: 0,
        stale_ticks: 0,
        shutdown,
        pitch_thread,
    }
}

fn try_build_stream(
    preferred: Option<&str>,
    fallback: bool,
    shutdown: Arc<AtomicBool>,
) -> (
    Option<Arc<Mutex<MicPitchData>>>,
    Option<cpal::Stream>,
    String,
    Arc<AtomicBool>,
    Arc<AtomicU64>,
    Option<std::thread::JoinHandle<()>>,
) {
    let error_flag = Arc::new(AtomicBool::new(false));
    let sample_counter = Arc::new(AtomicU64::new(0));

    let host = cpal::default_host();
    let candidates = rank_devices(&host, preferred, fallback);

    if candidates.is_empty() {
        return (
            None,
            None,
            "(no mic)".into(),
            Arc::clone(&error_flag),
            Arc::clone(&sample_counter),
            None,
        );
    }

    use cpal::SampleFormat;

    for device in candidates {
        let name = device_display_name(&device);

        let default_cfg = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                warn!("Skipping mic '{name}': {e}");
                continue;
            }
        };

        let sample_rate: u32 = default_cfg.sample_rate();
        let channels: u16 = default_cfg.channels();
        let sample_format = default_cfg.sample_format();
        info!("Trying mic '{name}': {sample_rate}Hz, {channels}ch, format={sample_format:?}");

        let config = cpal::StreamConfig {
            channels,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let shared = Arc::new(Mutex::new(MicPitchData::new(sample_rate)));
        let shared_cb = Arc::clone(&shared);
        let shared_detect = Arc::clone(&shared);

        let counter_cb = Arc::clone(&sample_counter);

        let push_samples: Arc<dyn Fn(&[f32]) + Send + Sync> = Arc::new(move |data: &[f32]| {
            if let Ok(mut lock) = shared_cb.try_lock() {
                let ch = channels as usize;
                for chunk in data.chunks(ch) {
                    let mono = chunk.iter().sum::<f32>() / ch as f32;
                    lock.samples.push_back(mono);
                }
                while lock.samples.len() > PITCH_WINDOW * 2 {
                    lock.samples.pop_front();
                }
            }
            counter_cb.fetch_add(data.len() as u64, Ordering::Relaxed);
        });

        let err_flag_clone = Arc::clone(&error_flag);
        let make_err_cb = move || {
            let flag = Arc::clone(&err_flag_clone);
            move |err: cpal::StreamError| {
                if !flag.swap(true, Ordering::Relaxed) {
                    error!("Microphone stream error: {err}");
                }
            }
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let push = push_samples.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| push(data),
                    make_err_cb(),
                    None,
                )
            }
            SampleFormat::I16 => {
                let push = push_samples.clone();
                let mut float_buf: Vec<f32> = Vec::new();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        float_buf.clear();
                        float_buf.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                        push(&float_buf);
                    },
                    make_err_cb(),
                    None,
                )
            }
            SampleFormat::I32 => {
                let push = push_samples.clone();
                let mut float_buf: Vec<f32> = Vec::new();
                device.build_input_stream(
                    &config,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        float_buf.clear();
                        float_buf.extend(data.iter().map(|&s| s as f32 / i32::MAX as f32));
                        push(&float_buf);
                    },
                    make_err_cb(),
                    None,
                )
            }
            other => {
                warn!("Skipping mic '{name}': unsupported format {other:?}");
                continue;
            }
        };

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                warn!("Skipping mic '{name}': failed to build stream: {e}");
                continue;
            }
        };

        if let Err(e) = stream.play() {
            warn!("Skipping mic '{name}': failed to play: {e}");
            continue;
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
        if error_flag.load(Ordering::Relaxed) {
            warn!("Skipping mic '{name}': stream errored immediately after start");
            error_flag.store(false, Ordering::Relaxed);
            drop(stream);
            continue;
        }

        info!("Microphone: {name}");
        let detect_shutdown = Arc::clone(&shutdown);
        let pitch_thread = std::thread::spawn(move || {
            pitch_detection_loop(shared_detect, detect_shutdown);
        });

        return (
            Some(shared),
            Some(stream),
            name,
            error_flag,
            sample_counter,
            Some(pitch_thread),
        );
    }

    warn!("No working microphone found");
    (
        None,
        None,
        "(no mic)".into(),
        error_flag,
        sample_counter,
        None,
    )
}

fn pitch_detection_loop(shared: Arc<Mutex<MicPitchData>>, shutdown: Arc<AtomicBool>) {
    let sleep_dur = std::time::Duration::from_millis(25);
    let mut detector = McLeodDetector::new(PITCH_WINDOW, PITCH_WINDOW / 2);
    let mut detect_count: u64 = 0;
    let mut hit_count: u64 = 0;

    loop {
        std::thread::sleep(sleep_dur);

        if shutdown.load(Ordering::Relaxed) {
            info!("Mic pitch detection thread shutting down");
            return;
        }

        let (window, sr) = {
            let Ok(lock) = shared.lock() else { return };
            if lock.samples.len() < PITCH_WINDOW {
                continue;
            }
            let start = lock.samples.len() - PITCH_WINDOW;
            let w: Vec<f32> = lock.samples.range(start..).copied().collect();
            (w, lock.sample_rate)
        };

        let pitch = if rms(&window) < MIC_RMS_GATE {
            None
        } else {
            detector
                .get_pitch(
                    &window,
                    sr as usize,
                    PITCH_POWER_THRESHOLD,
                    PITCH_CLARITY_THRESHOLD,
                )
                .filter(|p| p.frequency >= MIN_PITCH_HZ && p.frequency <= MAX_PITCH_HZ)
                .map(|p| p.frequency)
        };

        if let Ok(mut lock) = shared.lock() {
            lock.latest_pitch = pitch;
        }

        detect_count += 1;
        if pitch.is_some() {
            hit_count += 1;
        }
        if detect_count % 200 == 0 {
            info!("Mic pitch: {hit_count}/{detect_count} detections, latest={pitch:?}");
        }
    }
}

pub fn detect_pitch_from_samples(samples: &[f32], sample_rate: u32) -> Option<f32> {
    if samples.len() < PITCH_WINDOW {
        return None;
    }
    let window = &samples[samples.len() - PITCH_WINDOW..];
    if rms(window) < REF_RMS_GATE {
        return None;
    }
    let mut detector = McLeodDetector::new(PITCH_WINDOW, PITCH_WINDOW / 2);
    detector
        .get_pitch(
            window,
            sample_rate as usize,
            PITCH_POWER_THRESHOLD,
            PITCH_CLARITY_THRESHOLD,
        )
        .filter(|p| p.frequency >= MIN_PITCH_HZ && p.frequency <= MAX_PITCH_HZ)
        .map(|p| p.frequency)
}

#[derive(Resource)]
pub struct MicLoadTask {
    rx: Mutex<mpsc::Receiver<MicrophoneCapture>>,
}

pub fn spawn_mic_load(commands: &mut Commands, preferred: Option<String>) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let capture = start_microphone(preferred.as_deref(), true);
        let _ = tx.send(capture);
    });
    commands.insert_resource(MicLoadTask { rx: Mutex::new(rx) });
}

pub fn poll_mic_load(
    mut commands: Commands,
    task: Option<Res<MicLoadTask>>,
    config: Res<crate::config::AppConfig>,
    mut mic_text_query: Query<&mut Text, With<super::scoring::MicStatusText>>,
) {
    let Some(task) = task else { return };
    let Ok(rx) = task.rx.lock() else { return };
    if let Ok(mut capture) = rx.try_recv() {
        drop(rx);
        let has_device = capture.active;
        if has_device {
            capture.active = config.mic_active.unwrap_or(true);
        }
        let mic_active = capture.active;
        let device_name = capture.device_name.clone();
        commands.insert_resource(capture);
        commands.remove_resource::<MicLoadTask>();

        if let Ok(mut text) = mic_text_query.single_mut() {
            **text = super::format_mic_text(mic_active, &device_name);
        }
    }
}
