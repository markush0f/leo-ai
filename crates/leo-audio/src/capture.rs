use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use crate::error::AudioError;
use crate::pcm::{resample_mono, samples_per_frame, DEVICE_RATE, ML_RATE};
use crate::pulse::PulseStream;

const APP: &str = "leo-ai";

#[derive(Clone, Debug)]
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub struct Capture {
    rx: Receiver<AudioFrame>,
    stop: Arc<AtomicBool>,
}

impl Capture {
    pub fn start(device: &str) -> Result<Self, AudioError> {
        let (tx, rx) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_t = Arc::clone(&stop);
        let device = device.to_string();

        thread::Builder::new()
            .name("leo-capture".into())
            .spawn(move || {
                if let Err(err) = capture_loop(&device, tx, &stop_t) {
                    tracing::error!(%err, "captura terminó");
                }
            })
            .map_err(|e| AudioError::Thread(e.to_string()))?;

        Ok(Self { rx, stop })
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<AudioFrame, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn capture_loop(
    device: &str,
    tx: SyncSender<AudioFrame>,
    stop: &AtomicBool,
) -> Result<(), AudioError> {
    let in_len = samples_per_frame(DEVICE_RATE);
    let latency_bytes = (in_len * std::mem::size_of::<f32>()) as u32;
    let stream = PulseStream::record(APP, device, DEVICE_RATE, latency_bytes)?;
    let mut bytes = vec![0u8; latency_bytes as usize];

    while !stop.load(Ordering::Relaxed) {
        stream.read(&mut bytes)?;
        let samples: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        let ml = resample_mono(&samples, DEVICE_RATE, ML_RATE);
        match tx.try_send(AudioFrame {
            samples: ml,
            sample_rate: ML_RATE,
        }) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                tracing::debug!("captura: el motor va atrasado, descarto frame");
            }
        }
    }
    Ok(())
}
