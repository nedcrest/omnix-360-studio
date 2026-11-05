//! Crossfades between cuts in an export: frames written one cut after
//! another, the last frames of a cut held back and blended into the first
//! of the next.
//!
//! Each cut is rendered on its own (its own decoder and framing), so the two
//! cuts of a crossfade are never rendered side by side. The outgoing cut's
//! tail goes to a file instead, and is read back frame by frame as the
//! incoming cut's head arrives: at 2160p a 2 s crossfade would hold 1.5 GB
//! in memory.

use crate::ffmpeg::FrameSink;
use crate::stitch::Frame;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

pub struct Dissolve<'a> {
    out: &'a mut dyn FrameSink,
    /// The cut being written: frames so far, how many it has, how many of
    /// its first blend with the last cut's held tail, and how many of its
    /// last are held for the next.
    n: usize,
    frames: usize,
    head: usize,
    tail: usize,
    held: Option<BufReader<File>>,
    holding: Option<BufWriter<File>>,
    /// The two files, swapped at each cut: one read, one written.
    files: [PathBuf; 2],
}

impl<'a> Dissolve<'a> {
    /// Its files are named unguessably in the temp folder (by the pid
    /// alone, another user could plant a link there for it to write through).
    pub fn new(out: &'a mut dyn FrameSink) -> Self {
        use std::hash::{BuildHasher, Hasher};
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        let key = hasher.finish();
        let file = |k| std::env::temp_dir().join(format!("omnix-dissolve-{}-{key:016x}-{k}.rgb", std::process::id()));
        Dissolve { out, n: 0, frames: 0, head: 0, tail: 0, held: None, holding: None, files: [file(0), file(1)] }
    }

    /// Starts the next cut of `frames` frames, whose first `head` cross the
    /// last cut's held tail and whose last `tail` cross the next cut.
    pub fn cut(&mut self, frames: usize, head: usize, tail: usize) -> Result<()> {
        self.held = None;
        if let Some(mut w) = self.holding.take() {
            w.flush()?;
            drop(w);
            self.files.swap(0, 1);
            self.held = Some(BufReader::new(File::open(&self.files[0])?));
        }
        if tail > 0 {
            // Made new, never opened through whatever is at the name: the
            // last cut's file is removed first.
            let _ = std::fs::remove_file(&self.files[1]);
            let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&self.files[1]);
            self.holding = Some(BufWriter::new(file.with_context(|| format!("creating {}", self.files[1].display()))?));
        }
        (self.n, self.frames, self.head, self.tail) = (0, frames, head.min(frames), tail.min(frames));
        Ok(())
    }
}

impl FrameSink for Dissolve<'_> {
    fn write(&mut self, frame: &Frame) -> Result<()> {
        let n = self.n;
        self.n += 1;
        if n + self.tail >= self.frames
            && let Some(w) = &mut self.holding
        {
            return Ok(w.write_all(&frame.rgb)?);
        }
        if n < self.head
            && let Some(r) = &mut self.held
        {
            let mut before = vec![0; frame.rgb.len()];
            r.read_exact(&mut before).context("reading the crossfade's held frames")?;
            // Linear, by frame centres: the first frame already a little in.
            let a = (n as f32 + 0.5) / self.head as f32;
            let mix = |b: f32, c: f32| b + (c - b) * a;
            let rgb = if frame.deep {
                let sample = |s: &[u8]| u16::from_le_bytes([s[0], s[1]]) as f32;
                before.chunks_exact(2).zip(frame.rgb.chunks_exact(2)).flat_map(|(b, c)| (mix(sample(b), sample(c)).round() as u16).to_le_bytes()).collect()
            } else {
                before.iter().zip(&frame.rgb).map(|(&b, &c)| mix(b as f32, c as f32).round() as u8).collect()
            };
            return self.out.write(&Frame { width: frame.width, height: frame.height, rgb, deep: frame.deep });
        }
        self.out.write(frame)
    }
}

impl Drop for Dissolve<'_> {
    fn drop(&mut self) {
        (self.held, self.holding) = (None, None);
        for f in &self.files {
            let _ = std::fs::remove_file(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collects what comes out.
    struct Collect(Vec<Frame>);

    impl FrameSink for Collect {
        fn write(&mut self, frame: &Frame) -> Result<()> {
            self.0.push(Frame { width: frame.width, height: frame.height, rgb: frame.rgb.clone(), deep: frame.deep });
            Ok(())
        }
    }

    fn grey(v: u8) -> Frame {
        Frame { width: 2, height: 1, rgb: vec![v; 6], deep: false }
    }

    fn deep(v: u16) -> Frame {
        Frame { width: 2, height: 1, rgb: v.to_le_bytes().repeat(6), deep: true }
    }

    /// Two 10-frame cuts crossfading over 4: what comes out, and whether
    /// the held frames' files are gone once it's done.
    fn run(a: Frame, b: Frame) -> (Vec<Frame>, bool) {
        let mut out = Collect(Vec::new());
        let pid = format!("omnix-dissolve-{}-", std::process::id());
        let files = {
            let mut d = Dissolve::new(&mut out);
            let files = d.files.clone();
            d.cut(10, 0, 4).unwrap();
            for _ in 0..10 {
                d.write(&a).unwrap();
            }
            d.cut(10, 4, 0).unwrap();
            for _ in 0..10 {
                d.write(&b).unwrap();
            }
            files
        };
        assert!(files.iter().all(|f| f.to_string_lossy().contains(&pid)));
        (out.0, files.iter().all(|f| !f.exists()))
    }

    #[test]
    fn the_outgoing_tail_blends_into_the_incoming_head() {
        let (frames, cleaned) = run(grey(0), grey(200));
        // 10 + 10 frames, 4 shared: 16, the shared ones by frame centres.
        assert_eq!(frames.len(), 16);
        let values: Vec<u8> = frames.iter().map(|f| f.rgb[0]).collect();
        assert_eq!(values, [0, 0, 0, 0, 0, 0, 25, 75, 125, 175, 200, 200, 200, 200, 200, 200]);
        assert!(cleaned, "no held frames left in the temp folder");
    }

    #[test]
    fn sixteen_bit_frames_blend_at_their_own_depth() {
        let (frames, _) = run(deep(0), deep(40000));
        let first = |f: &Frame| u16::from_le_bytes([f.rgb[0], f.rgb[1]]);
        assert_eq!(frames.len(), 16);
        assert_eq!((first(&frames[6]), first(&frames[9]), first(&frames[15])), (5000, 35000, 40000));
        assert!(frames.iter().all(|f| f.deep && f.rgb.len() == 12));
    }
}
