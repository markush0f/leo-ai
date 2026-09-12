use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;

use crate::error::AudioError;
use crate::pcm::{DEVICE_RATE, ML_RATE, resample_mono, samples_per_frame};
use crate::pulse::PulseStream;

const APP: &str = "leo-ai";

enum Cmd {
    Play { pcm: Vec<f32>, sample_rate: u32 },
    Stop,
    Shutdown,
}

#[derive(Clone)]
pub struct Player {
    tx: Sender<Cmd>,
    stop: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
}

impl Player {
    pub fn start(device: &str) -> Result<Self, AudioError> {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let playing = Arc::new(AtomicBool::new(false));
        let stop_t = Arc::clone(&stop);
        let playing_t = Arc::clone(&playing);
        let device = device.to_string();

        thread::Builder::new()
            .name("leo-player".into())
            .spawn(move || {
                if let Err(err) = player_loop(&device, rx, &stop_t, &playing_t) {
                    tracing::error!(%err, "player terminó");
                }
            })
            .map_err(|e| AudioError::Thread(e.to_string()))?;

        Ok(Self { tx, stop, playing })
    }

    pub fn play(&self, pcm: Vec<f32>, sample_rate: u32) -> Result<(), AudioError> {
        self.stop.store(false, Ordering::SeqCst);
        self.tx
            .send(Cmd::Play { pcm, sample_rate })
            .map_err(|_| AudioError::Closed)
    }

    pub fn stop(&self) -> Result<(), AudioError> {
        self.stop.store(true, Ordering::SeqCst);
        self.tx.send(Cmd::Stop).map_err(|_| AudioError::Closed)
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Cmd::Shutdown);
    }
}

fn player_loop(
    device: &str,
    rx: mpsc::Receiver<Cmd>,
    stop: &AtomicBool,
    playing: &AtomicBool,
) -> Result<(), AudioError> {
    let chunk = samples_per_frame(DEVICE_RATE);
    let latency_bytes = (chunk * std::mem::size_of::<f32>()) as u32;
    let stream = PulseStream::playback(APP, device, DEVICE_RATE, latency_bytes)?;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Shutdown => break,
            Cmd::Stop => {
                playing.store(false, Ordering::SeqCst);
                let _ = stream.flush();
            }
            Cmd::Play { pcm, sample_rate } => {
                let device_pcm = if sample_rate == DEVICE_RATE {
                    pcm
                } else {
                    resample_mono(&pcm, sample_rate, DEVICE_RATE)
                };
                playing.store(true, Ordering::SeqCst);
                stop.store(false, Ordering::SeqCst);
                for frame in device_pcm.chunks(chunk) {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let mut bytes = Vec::with_capacity(frame.len() * 4);
                    for s in frame {
                        bytes.extend_from_slice(&s.to_le_bytes());
                    }
                    if stream.write(&bytes).is_err() {
                        break;
                    }
                }
                playing.store(false, Ordering::SeqCst);
                if stop.load(Ordering::Relaxed) {
                    let _ = stream.flush();
                }
            }
        }
    }
    Ok(())
}

pub fn play_beep(player: &Player, freq: f32, ms: u32) -> Result<(), AudioError> {
    let pcm = crate::pcm::sine_beep(freq, ms, ML_RATE, 0.18);
    player.play(pcm, ML_RATE)
}
