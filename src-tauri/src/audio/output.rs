use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::*};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use rtrb::{Consumer, Producer, RingBuffer};

/// State shared between the player thread and the realtime audio callback.
pub struct Shared {
    volume: AtomicU32,
    paused: AtomicBool,
    /// Output frames consumed (played or flushed) since the stream was created.
    pub played: AtomicU64,
    flush_req: AtomicU64,
    flush_ack: AtomicU64,
}

pub struct Output {
    stream: cpal::Stream,
    pub prod: Producer<f32>,
    pub shared: Arc<Shared>,
    pub rate: u32,
}

/// Perceptual volume curve.
fn gain(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    v * v
}

impl Output {
    pub fn open(volume: f32) -> Result<Self> {
        let host = cpal::default_host();
        let dev = host.default_output_device().ok_or_else(|| anyhow!("no audio output device"))?;
        let cfg = dev.default_output_config()?;
        let rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        // Room for 2 s of stereo audio. The engine decides how much of it to fill (a short lead in
        // the foreground so EQ changes feel live, a long one in the background to wake up rarely).
        let (prod, cons) = RingBuffer::<f32>::new(rate as usize * 2 * 2);
        let shared = Arc::new(Shared {
            volume: AtomicU32::new(gain(volume).to_bits()),
            paused: AtomicBool::new(true),
            played: AtomicU64::new(0),
            flush_req: AtomicU64::new(0),
            flush_ack: AtomicU64::new(0),
        });
        let sc: cpal::StreamConfig = cfg.clone().into();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&dev, &sc, channels, cons, shared.clone()),
            cpal::SampleFormat::I16 => build::<i16>(&dev, &sc, channels, cons, shared.clone()),
            cpal::SampleFormat::U16 => build::<u16>(&dev, &sc, channels, cons, shared.clone()),
            cpal::SampleFormat::I32 => build::<i32>(&dev, &sc, channels, cons, shared.clone()),
            f => Err(anyhow!("unsupported sample format {f:?}")),
        }?;
        stream.play()?;
        Ok(Self { stream, prod, shared, rate })
    }

    pub fn set_volume(&self, v: f32) {
        self.shared.volume.store(gain(v).to_bits(), Relaxed);
    }

    pub fn set_playing(&mut self, on: bool) {
        self.shared.paused.store(!on, Relaxed);
    }

    /// Discards everything queued. Writing must wait until `flushed()`.
    pub fn flush(&mut self) {
        self.shared.flush_req.fetch_add(1, AcqRel);
    }

    pub fn flushed(&self) -> bool {
        self.shared.flush_ack.load(Acquire) == self.shared.flush_req.load(Acquire)
    }

    pub fn played(&self) -> u64 {
        self.shared.played.load(Acquire)
    }
}

fn build<T: SizedSample + FromSample<f32>>(
    dev: &cpal::Device,
    cfg: &cpal::StreamConfig,
    channels: usize,
    mut cons: Consumer<f32>,
    sh: Arc<Shared>,
) -> Result<cpal::Stream> {
    let mut seen_flush = 0u64;
    let mut fade = 0f32;
    let mut vol = f32::from_bits(sh.volume.load(Relaxed));
    let step = 1.0 / (cfg.sample_rate.0 as f32 * 0.015); // 15 ms fades on pause/resume
    let stream = dev.build_output_stream(
        cfg,
        move |data: &mut [T], _| {
            let req = sh.flush_req.load(Acquire);
            if req != seen_flush {
                let n = cons.slots();
                if let Ok(c) = cons.read_chunk(n) {
                    c.commit_all();
                }
                sh.played.fetch_add((n / 2) as u64, AcqRel);
                seen_flush = req;
                sh.flush_ack.store(req, Release);
            }
            let paused = sh.paused.load(Relaxed);
            if paused && fade <= 0.0 {
                data.iter_mut().for_each(|s| *s = T::EQUILIBRIUM);
                return;
            }
            let target_vol = f32::from_bits(sh.volume.load(Relaxed));
            let frames = data.len() / channels;
            let avail = cons.slots() / 2;
            let n = frames.min(avail);
            let mut played = 0u64;
            if let Ok(chunk) = cons.read_chunk(n * 2) {
                let (a, b) = chunk.as_slices();
                let mut it = a.iter().chain(b.iter());
                for f in data.chunks_exact_mut(channels).take(n) {
                    let l = *it.next().unwrap_or(&0.0);
                    let r = *it.next().unwrap_or(&0.0);
                    fade = if paused { (fade - step).max(0.0) } else { (fade + step).min(1.0) };
                    vol += (target_vol - vol) * 0.002;
                    let g = vol * fade * fade;
                    let (l, r) = ((l * g).clamp(-1.0, 1.0), (r * g).clamp(-1.0, 1.0));
                    if channels == 1 {
                        f[0] = T::from_sample((l + r) * 0.5);
                    } else {
                        f[0] = T::from_sample(l);
                        f[1] = T::from_sample(r);
                        f[2..].iter_mut().for_each(|s| *s = T::EQUILIBRIUM);
                    }
                }
                chunk.commit_all();
                played = n as u64;
            }
            // Underrun (or end of queue): pad with silence.
            data[n * channels..].iter_mut().for_each(|s| *s = T::EQUILIBRIUM);
            if paused && n < frames {
                fade = 0.0;
            }
            sh.played.fetch_add(played, AcqRel);
        },
        |e| eprintln!("audio stream error: {e}"),
        None,
    )?;
    Ok(stream)
}
