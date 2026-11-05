//! Loudness as the platforms measure it (ITU-R BS.1770, ffmpeg's `ebur128`).
//! The meter measures the mix as it plays: short-term (the last 3 s) and
//! integrated since play started, beside the peak meter, so «too quiet»
//! shows in the units the targets are given in (YouTube −14 LUFS; gap 46b).
//! The mix check measures each clip with it too.
//!
//! Each channel is K-weighted (a high shelf for the head's effect, a high
//! pass), squared and summed per 100 ms; a 400 ms block every 100 ms is
//! what the integrated value gates: blocks under −70 LUFS, then those
//! 10 LU under the mean of the rest, left out.

use std::collections::VecDeque;
use std::f64::consts::PI;

/// One biquad stage, direct form I, per channel.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn run(&mut self, x0: f64) -> f64 {
        let y0 = self.b[0] * x0 + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x0, self.x[0]];
        self.y = [y0, self.y[0]];
        y0
    }
}

/// BS.1770's two K-weighting stages at `rate` (libebur128's formulas, so
/// any device rate, not only 48 kHz).
fn k_weighting(rate: f64) -> [Biquad; 2] {
    let (f0, gain, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (PI * f0 / rate).tan();
    let vh = 10f64.powf(gain / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad { b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0], ..Default::default() };
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let high = Biquad { b: [1.0, -2.0, 1.0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0], ..Default::default() };
    [shelf, high]
}

/// Mean squares to LUFS.
fn lufs(mean_square: f64) -> f64 {
    -0.691 + 10.0 * mean_square.max(1e-20).log10()
}

#[derive(Default)]
pub struct Loudness {
    rate: f64,
    filters: [[Biquad; 2]; 2],
    /// The 100 ms block being filled: its sum of squares and frames.
    sum: f64,
    n: usize,
    /// The last 3 s of 100 ms blocks' mean squares, newest last.
    recent: VecDeque<f64>,
    /// Every 400 ms block's mean square since play started.
    blocks: Vec<f64>,
}

impl Loudness {
    /// Takes in frames the mix played at `rate` (left, right).
    pub fn feed(&mut self, rate: f64, frames: &[[f32; 2]]) {
        if rate <= 0.0 {
            return;
        }
        if rate != self.rate {
            *self = Loudness { rate, filters: [k_weighting(rate); 2], ..Default::default() };
        }
        let per_block = (rate / 10.0).round() as usize;
        for f in frames {
            for c in 0..2 {
                let [shelf, high] = &mut self.filters[c];
                let y = high.run(shelf.run(f[c] as f64));
                self.sum += y * y;
            }
            self.n += 1;
            if self.n == per_block {
                self.recent.push_back(self.sum / per_block as f64);
                (self.sum, self.n) = (0.0, 0);
                if self.recent.len() > 30 {
                    self.recent.pop_front();
                }
                if self.recent.len() >= 4 {
                    let last4 = self.recent.iter().rev().take(4).sum::<f64>() / 4.0;
                    self.blocks.push(last4);
                }
            }
        }
    }

    /// The last 3 s (fewer at the start), LUFS; `None` before 400 ms.
    pub fn short_term(&self) -> Option<f64> {
        (self.recent.len() >= 4).then(|| lufs(self.recent.iter().sum::<f64>() / self.recent.len() as f64))
    }

    /// The loudest 400 ms so far, LUFS; `None` before 400 ms.
    pub fn loudest(&self) -> Option<f64> {
        self.blocks.iter().copied().reduce(f64::max).map(lufs)
    }

    /// Since play started, gated as BS.1770 says; `None` while silent.
    pub fn integrated(&self) -> Option<f64> {
        let loud: Vec<f64> = self.blocks.iter().copied().filter(|&b| lufs(b) > -70.0).collect();
        if loud.is_empty() {
            return None;
        }
        let relative = lufs(loud.iter().sum::<f64>() / loud.len() as f64) - 10.0;
        let kept: Vec<f64> = loud.into_iter().filter(|&b| lufs(b) > relative).collect();
        (!kept.is_empty()).then(|| lufs(kept.iter().sum::<f64>() / kept.len() as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: f64, secs: f64, amplitude: f32) -> Vec<[f32; 2]> {
        (0..(rate * secs) as usize).map(|i| [(amplitude as f64 * (2.0 * PI * 997.0 * i as f64 / rate).sin()) as f32; 2]).collect()
    }

    #[test]
    fn a_tone_measures_as_the_standard_says() {
        // BS.1770: a 997 Hz sine at 0 dBFS in one channel reads −3.01 LUFS,
        // so in both at amplitude 0.1 (−20 dBFS) about −20.
        for rate in [44100.0, 48000.0] {
            let mut l = Loudness::default();
            l.feed(rate, &tone(rate, 4.0, 0.1));
            let (s, i) = (l.short_term().unwrap(), l.integrated().unwrap());
            assert!((s + 20.0).abs() < 0.2 && (i + 20.0).abs() < 0.2, "{rate}: short {s:.2}, integrated {i:.2}");
        }
    }

    #[test]
    fn pink_noise_measures_as_ffmpeg_does() {
        use crate::ffmpeg::testmedia;
        let dir = testmedia::dir("lufs-pink");
        let wav = dir.join("pink.wav");
        testmedia::ffmpeg(&["-f", "lavfi", "-i", "anoisesrc=color=pink:d=6:a=0.2:r=48000", "-ac", "2", wav.to_str().unwrap()]);
        let theirs = crate::ffmpeg::tool("ffmpeg").args(["-hide_banner", "-nostdin", "-i"]).arg(&wav).args(["-af", "ebur128=framelog=quiet", "-f", "null", "-"]).output().unwrap();
        let text = String::from_utf8_lossy(&theirs.stderr);
        let theirs: f64 = text.lines().filter_map(|l| l.trim().strip_prefix("I:")?.trim().strip_suffix("LUFS")?.trim().parse().ok()).next_back().unwrap();
        let raw = crate::ffmpeg::tool("ffmpeg").args(["-v", "error", "-nostdin", "-i"]).arg(&wav).args(["-f", "f32le", "-"]).output().unwrap().stdout;
        std::fs::remove_dir_all(&dir).unwrap();
        let samples: Vec<f32> = raw.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let frames: Vec<[f32; 2]> = samples.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        let mut l = Loudness::default();
        l.feed(48000.0, &frames);
        let ours = l.integrated().unwrap();
        assert!((ours - theirs).abs() < 0.3, "ours {ours:.2}, ffmpeg {theirs:.2}");
    }

    #[test]
    fn quiet_stretches_are_gated_out_of_the_integrated() {
        let mut l = Loudness::default();
        // 3 s at −20, then 3 s at −50: the quiet part is more than 10 LU
        // under, so the integrated stays at −20; the short-term falls.
        l.feed(48000.0, &tone(48000.0, 3.0, 0.1));
        l.feed(48000.0, &tone(48000.0, 3.0, 0.00316));
        assert!((l.integrated().unwrap() + 20.0).abs() < 0.3, "{:?}", l.integrated());
        assert!(l.short_term().unwrap() < -45.0);
        assert_eq!(Loudness::default().integrated(), None);
    }
}
