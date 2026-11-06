use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// Decodes a file to interleaved stereo f32 at its native sample rate.
pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    pub sample_rate: u32,
    buf: Option<SampleBuffer<f32>>,
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Self> {
        let p = path.to_string_lossy();
        let cached = crate::navidrome::song_id(&p).and_then(crate::cache::cached);
        let src: Box<dyn MediaSource> = if let Some(file) = cached {
            Box::new(File::open(file)?)
        } else if p.starts_with(crate::navidrome::PREFIX) {
            Box::new(crate::navidrome::Stream::open(&p)?)
        } else {
            Box::new(File::open(path)?)
        };
        let mss = MediaSourceStream::new(src, Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let fmt_opts = FormatOptions { enable_gapless: true, ..Default::default() };
        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &MetadataOptions::default())?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow!("no audio track"))?;
        let decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
        Ok(Self {
            track_id: track.id,
            sample_rate: track.codec_params.sample_rate.unwrap_or(44100),
            format,
            decoder,
            buf: None,
        })
    }

    /// Appends the next decoded packet (as stereo) to `out`. Returns false at end of stream.
    pub fn next_chunk(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
                Err(Error::ResetRequired) => return Ok(false),
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // Corrupt frames are skipped rather than ending playback.
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            };
            let spec = *decoded.spec();
            let frames = decoded.capacity() as u64;
            if self.buf.as_ref().is_none_or(|b| b.capacity() < decoded.capacity() * spec.channels.count()) {
                self.buf = Some(SampleBuffer::new(frames, spec));
            }
            let buf = self.buf.as_mut().expect("buffer allocated above");
            buf.copy_interleaved_ref(decoded);
            let ch = spec.channels.count().max(1);
            let s = buf.samples();
            out.reserve(s.len() / ch * 2);
            if ch == 1 {
                for &x in s {
                    out.extend_from_slice(&[x, x]);
                }
            } else {
                for f in s.chunks_exact(ch) {
                    out.extend_from_slice(&[f[0], f[1]]);
                }
            }
            return Ok(true);
        }
    }

    /// Seeks to `secs`, returning the actual position reached.
    pub fn seek(&mut self, secs: f64) -> Result<f64> {
        let to = self.format.seek(SeekMode::Accurate, SeekTo::Time { time: Time::from(secs.max(0.0)), track_id: Some(self.track_id) })?;
        self.decoder.reset();
        let tb = self.format.tracks().iter().find(|t| t.id == self.track_id).and_then(|t| t.codec_params.time_base);
        Ok(tb.map(|tb| {
            let t = tb.calc_time(to.actual_ts);
            t.seconds as f64 + t.frac
        })
        .unwrap_or(secs))
    }
}
