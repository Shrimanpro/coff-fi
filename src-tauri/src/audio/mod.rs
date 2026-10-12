pub(crate) mod decoder;
pub mod output;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use parking_lot::{Mutex, RwLock};
use rubato::{Resampler as _, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use serde::Serialize;

use crate::library::db;
use crate::queue::Queue;
use crate::settings;
use decoder::Decoder;
use output::Output;

pub enum Cmd {
    Load { id: i64, start: f64, play: bool },
    SetPlaying(bool),
    Toggle,
    Seek(f64),
    Next,
    Prev,
    Stop,
    Volume(f32),
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Status {
    pub track_id: Option<i64>,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
}

pub enum Event {
    State(Status),
    Track(Option<i64>),
    Error(String),
}

#[derive(Clone)]
pub struct Player {
    tx: Sender<Cmd>,
    pub queue: Arc<Mutex<Queue>>,
    pub status: Arc<RwLock<Status>>,
}

impl Player {
    pub fn spawn(db_path: PathBuf, s: &settings::Settings, queue: Queue, on_event: impl Fn(Event) + Send + 'static) -> Self {
        let (tx, rx) = unbounded();
        let queue = Arc::new(Mutex::new(queue));
        let status = Arc::new(RwLock::new(Status::default()));
        let init = Init { volume: s.volume };
        let (q, st) = (queue.clone(), status.clone());
        std::thread::Builder::new()
            .name("player".into())
            .spawn(move || {
                let conn = db::open(&db_path).expect("open database");
                Engine::new(conn, rx, q, st, init, Box::new(on_event)).run();
            })
            .expect("spawn player thread");
        Self { tx, queue, status }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}

struct Init {
    volume: f32,
}

/// Sinc resampler fed with interleaved stereo.
struct Rs {
    r: SincFixedIn<f32>,
    inp: [Vec<f32>; 2],
}

impl Rs {
    fn new(ratio: f64) -> Result<Self> {
        let p = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        Ok(Self { r: SincFixedIn::new(ratio, 1.0, p, 1024, 2)?, inp: [Vec::new(), Vec::new()] })
    }

    fn push(&mut self, data: &[f32], out: &mut Vec<f32>, last: bool) -> Result<()> {
        for f in data.chunks_exact(2) {
            self.inp[0].push(f[0]);
            self.inp[1].push(f[1]);
        }
        loop {
            let need = self.r.input_frames_next();
            if self.inp[0].len() < need {
                break;
            }
            let res = self.r.process(&[&self.inp[0][..need], &self.inp[1][..need]], None)?;
            interleave(&res, out);
            self.inp.iter_mut().for_each(|c| {
                c.drain(..need);
            });
        }
        if last && !self.inp[0].is_empty() {
            let res = self.r.process_partial(Some(&[&self.inp[0][..], &self.inp[1][..]]), None)?;
            interleave(&res, out);
            self.inp.iter_mut().for_each(Vec::clear);
        }
        Ok(())
    }
}

fn interleave(ch: &[Vec<f32>], out: &mut Vec<f32>) {
    out.reserve(ch[0].len() * 2);
    for (l, r) in ch[0].iter().zip(&ch[1]) {
        out.extend_from_slice(&[*l, *r]);
    }
}

/// One track being decoded into the output's sample rate.
struct Src {
    id: i64,
    dec: Decoder,
    rs: Option<Rs>,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    eof: bool,
    out_frames: u64,
    total_frames: u64,
}

impl Src {
    fn open(path: &Path, id: i64, start: f64, duration: f64, rate: u32) -> Result<(Self, f64)> {
        let mut dec = Decoder::open(path)?;
        let actual = if start > 0.05 { dec.seek(start).unwrap_or(0.0) } else { 0.0 };
        let ratio = rate as f64 / dec.sample_rate as f64;
        let rs = if (ratio - 1.0).abs() < 1e-9 { None } else { Some(Rs::new(ratio)?) };
        let total_frames = ((duration - actual).max(0.0) * rate as f64) as u64;
        Ok((Self { id, dec, rs, pending: Vec::new(), scratch: Vec::new(), eof: false, out_frames: 0, total_frames }, actual))
    }

    fn remaining(&self) -> u64 {
        self.total_frames.saturating_sub(self.out_frames)
    }

    fn exhausted(&self) -> bool {
        self.eof && self.pending.is_empty()
    }

    /// Appends up to `n` frames to `out`; fewer only at end of stream.
    fn take(&mut self, n: usize, out: &mut Vec<f32>) -> usize {
        while self.pending.len() < n * 2 && !self.eof {
            self.scratch.clear();
            let more = self.dec.next_chunk(&mut self.scratch).unwrap_or(false);
            match &mut self.rs {
                Some(rs) => {
                    if rs.push(&self.scratch, &mut self.pending, !more).is_err() {
                        self.eof = true;
                    }
                }
                None => self.pending.extend_from_slice(&self.scratch),
            }
            self.eof |= !more;
        }
        let k = (n * 2).min(self.pending.len());
        out.extend(self.pending.drain(..k));
        self.out_frames += (k / 2) as u64;
        k / 2
    }
}

struct Boundary {
    at: u64,
    id: i64,
    start: f64,
    duration: f64,
}

struct Engine {
    conn: rusqlite::Connection,
    rx: Receiver<Cmd>,
    queue: Arc<Mutex<Queue>>,
    status: Arc<RwLock<Status>>,
    emit: Box<dyn Fn(Event) + Send>,
    out: Option<Output>,
    volume: f32,
    cur: Option<Src>,
    written: u64,
    bounds: VecDeque<Boundary>,
    ended: bool,
    playing: bool,
    last_emit: Instant,
    last_status: Status,
    mix: Vec<f32>,
}

/// How far ahead of the device the engine decodes, in seconds.
const LEAD_FOREGROUND: f64 = 0.3;
/// While playing, a state event is sent at least this often (the UI interpolates in between).
const HEARTBEAT: Duration = Duration::from_secs(5);

impl Engine {
    fn new(conn: rusqlite::Connection, rx: Receiver<Cmd>, queue: Arc<Mutex<Queue>>, status: Arc<RwLock<Status>>, i: Init, emit: Box<dyn Fn(Event) + Send>) -> Self {
        Self {
            conn,
            rx,
            queue,
            status,
            emit,
            out: None,
            volume: i.volume,
            cur: None,
            written: 0,
            bounds: VecDeque::new(),
            ended: false,
            playing: false,
            last_emit: Instant::now(),
            last_status: Status::default(),
            mix: Vec::new(),
        }
    }

    fn run(mut self) {
        loop {
            match self.rx.recv_timeout(self.wait()) {
                Ok(c) => self.handle(c),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while let Ok(c) = self.rx.try_recv() {
                self.handle(c);
            }
            if self.playing {
                self.fill();
                self.track_position();
            }
            if let Some(o) = &mut self.out {
                o.idle();
            }
            self.publish(false);
        }
    }

    /// Frames to keep decoded ahead of the device.
    fn target(&self) -> u64 {
        let lead = LEAD_FOREGROUND;
        (self.rate() as f64 * lead) as u64
    }

    /// How long the player thread can sleep before it has something to do: refill the buffer
    /// once it is half drained, notice a track boundary, or count a play. Commands still wake
    /// it immediately through the channel.
    fn wait(&self) -> Duration {
        let (true, Some(o)) = (self.playing, &self.out) else { return Duration::from_millis(250) };
        if !o.flushed() {
            return Duration::from_millis(5);
        }
        let rate = o.rate as f64;
        let played = o.played();
        let buffered = self.written.saturating_sub(played);
        let mut secs = buffered.saturating_sub(self.target() / 2) as f64 / rate;
        if let Some(b) = self.bounds.get(1) {
            secs = secs.min(b.at.saturating_sub(played) as f64 / rate);
        }
        wait_clamp(secs)
    }

    fn rate(&self) -> u32 {
        self.out.as_ref().map_or(48000, |o| o.rate)
    }

    fn ensure_output(&mut self) -> Result<()> {
        if self.out.is_none() {
            let o = Output::open(self.volume)?;
            self.written = o.played();
            self.out = Some(o);
        }
        Ok(())
    }

    fn handle(&mut self, c: Cmd) {
        match c {
            Cmd::Load { id, start, play } => {
                if let Err(e) = self.load(id, start, play) {
                    (self.emit)(Event::Error(format!("{e:#}")));
                    // Without an output device every track would fail; only skip genuinely broken files.
                    if self.out.is_some() {
                        self.skip_broken(id);
                    }
                }
            }
            Cmd::SetPlaying(on) => self.set_playing(on),
            Cmd::Toggle => self.set_playing(!self.playing),
            Cmd::Seek(s) => self.seek(s),
            Cmd::Next => {
                let next = self.queue.lock().skip_next();
                self.load_or_stop(next);
            }
            Cmd::Prev => {
                if self.position() > 3.0 {
                    self.seek(0.0);
                } else {
                    let prev = self.queue.lock().skip_prev();
                    self.load_or_stop(prev);
                }
            }
            Cmd::Stop => self.stop(),
            Cmd::Volume(v) => {
                self.volume = v;
                if let Some(o) = &self.out {
                    o.set_volume(v);
                }
            }
        }
        self.publish(true);
    }

    fn load_or_stop(&mut self, id: Option<i64>) {
        match id {
            Some(id) => self.handle(Cmd::Load { id, start: 0.0, play: true }),
            None => self.stop(),
        }
    }

    /// A file failed to open: move past it rather than stalling.
    fn skip_broken(&mut self, id: i64) {
        let next = {
            let mut q = self.queue.lock();
            if q.current() == Some(id) && q.pos + 1 < q.items.len() { q.skip_next() } else { None }
        };
        match next {
            Some(n) if n != id => self.handle(Cmd::Load { id: n, start: 0.0, play: true }),
            _ => self.stop(),
        }
    }

    fn open_src(&self, id: i64, start: f64) -> Result<(Src, f64, f64)> {
        let t = db::track(&self.conn, id)?.ok_or_else(|| anyhow!("track {id} is no longer in the library"))?;
        let (src, actual) = Src::open(Path::new(&t.path), id, start, t.duration, self.rate())
            .map_err(|e| anyhow!("can't play \"{}\": {e}", t.title))?;
        Ok((src, actual, t.duration))
    }

    fn load(&mut self, id: i64, start: f64, play: bool) -> Result<()> {
        self.ensure_output()?;
        let (src, actual, duration) = self.open_src(id, start)?;
        if let Some(o) = &mut self.out {
            o.flush();
        }
        let changed = self.audible() != Some(id);
        self.cur = Some(src);
        self.ended = false;
        self.bounds.clear();
        self.bounds.push_back(Boundary { at: self.written, id, start: actual, duration });
        if changed {
            (self.emit)(Event::Track(Some(id)));
        }
        self.set_playing(play);
        Ok(())
    }

    fn seek(&mut self, s: f64) {
        let Some(id) = self.audible() else { return };
        let dur = self.bounds.front().map_or(0.0, |b| b.duration);
        let s = s.clamp(0.0, (dur - 0.25).max(0.0));
        if let Err(e) = self.load(id, s, self.playing) {
            (self.emit)(Event::Error(format!("{e:#}")));
        }
    }

    fn set_playing(&mut self, on: bool) {
        if on && self.bounds.is_empty() {
            let id = self.queue.lock().current();
            if let Some(id) = id {
                self.handle(Cmd::Load { id, start: 0.0, play: true });
            }
            return;
        }
        self.playing = on && !self.bounds.is_empty();
        if let Some(o) = &mut self.out {
            o.set_playing(self.playing);
        }
    }

    fn stop(&mut self) {
        if let Some(o) = &mut self.out {
            o.set_playing(false);
            o.flush();
        }
        self.playing = false;
        self.cur = None;
        self.bounds.clear();
        self.ended = false;
        (self.emit)(Event::Track(None));
    }

    fn audible(&self) -> Option<i64> {
        self.bounds.front().map(|b| b.id)
    }

    fn position(&self) -> f64 {
        let (Some(b), Some(o)) = (self.bounds.front(), &self.out) else { return 0.0 };
        let played = o.played().saturating_sub(b.at);
        (b.start + played as f64 / o.rate as f64).min(b.duration)
    }

    fn fill(&mut self) {
        let Some(out) = &self.out else { return };
        if !out.flushed() {
            return;
        }
        let rate = out.rate;
        let target = self.target();
        loop {
            let space = self.out.as_ref().map_or(0, |o| {
                let buffered = self.written.saturating_sub(o.played());
                (o.prod.slots() / 2).min(target.saturating_sub(buffered) as usize)
            });
            if space < 256 || self.cur.is_none() {
                break;
            }
            let n = space.min(4096);
            self.mix.clear();

            if let Some(cur) = &mut self.cur {
                let got = cur.take(n, &mut self.mix);
                if got < n && cur.exhausted() {
                    let at = self.written + got as u64;
                            self.cur = None;
                            self.ended = true;
                            self.bounds.push_back(Boundary { at, id: -1, start: 0.0, duration: 0.0 });
                }
            }

            if self.mix.is_empty() {
                break;
            }
            let o = self.out.as_mut().expect("output exists");
            if let Ok(chunk) = o.prod.write_chunk_uninit(self.mix.len()) {
                chunk.fill_from_iter(self.mix.iter().copied());
                self.written += (self.mix.len() / 2) as u64;
            }
        }
    }

    /// Advances the audible track as the device plays past buffered boundaries.
    fn track_position(&mut self) {
        let Some(o) = &self.out else { return };
        let played = o.played();
        while self.bounds.len() > 1 && played >= self.bounds[1].at {
            self.bounds.pop_front();
            let id = self.bounds[0].id;
            if id < 0 {
                let next = self.queue.lock().advance();
                self.load_or_stop(next);
                return;
            }
            self.queue.lock().advance();
            (self.emit)(Event::Track(Some(id)));
        }
    }

    fn publish(&mut self, force: bool) {
        let st = Status {
            track_id: self.audible().filter(|&i| i >= 0),
            playing: self.playing,
            position: self.position(),
            duration: self.bounds.front().map_or(0.0, |b| b.duration),
        };
        *self.status.write() = st.clone();
        let due = self.playing && self.last_emit.elapsed() >= HEARTBEAT;
        let changed = st.playing != self.last_status.playing || st.track_id != self.last_status.track_id;
        if force || due || changed {
            self.last_emit = Instant::now();
            self.last_status = st.clone();
            (self.emit)(Event::State(st));
        }
    }
}

fn wait_clamp(secs: f64) -> Duration {
    Duration::from_secs_f64(secs.clamp(0.005, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_is_clamped() {
        assert_eq!(wait_clamp(0.0), Duration::from_millis(5));
        assert_eq!(wait_clamp(-3.0), Duration::from_millis(5));
        assert_eq!(wait_clamp(0.15), Duration::from_secs_f64(0.15));
        assert_eq!(wait_clamp(9.0), Duration::from_secs(1));
    }
}
