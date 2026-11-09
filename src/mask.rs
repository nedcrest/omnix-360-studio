//! Coverage masks, one value a pixel in rows: moved, blurred, measured.
//! Text's outline and shadow and the overlay kinds' (`dress`) draw with
//! them.

use rayon::prelude::*;

/// `mask` moved `by` pixels right and down.
pub fn shift<T: Copy + Default>(mask: &[T], (w, h): (usize, usize), by: f64) -> Vec<T> {
    let d = by.round() as usize;
    let mut out = vec![T::default(); w * h];
    for y in d..h {
        out[y * w + d..(y + 1) * w].copy_from_slice(&mask[(y - d) * w..(y - d + 1) * w - d]);
    }
    out
}

/// A box blur of radius `r` across and then down, twice: near enough a
/// Gaussian for a shadow.
pub fn blur(mask: &[f32], (w, h): (usize, usize), r: f64) -> Vec<f32> {
    let r = r.round() as usize;
    if r == 0 {
        return mask.to_vec();
    }
    let mut a = mask.to_vec();
    for _ in 0..2 {
        a = box_rows(&a, w, h, r);
        a = transpose(&box_rows(&transpose(&a, w, h), h, w, r), h, w);
    }
    a
}

fn box_rows(m: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut out = vec![0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        // Running sums: each pixel's window in two lookups, however wide.
        let mut sums = vec![0f64; w + 1];
        for (x, v) in m[y * w..(y + 1) * w].iter().enumerate() {
            sums[x + 1] = sums[x] + *v as f64;
        }
        for (x, o) in row.iter_mut().enumerate() {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            *o = ((sums[b + 1] - sums[a]) / (2 * r + 1) as f64) as f32;
        }
    });
    out
}

fn transpose(m: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            out[x * h + y] = m[y * w + x];
        }
    }
    out
}
