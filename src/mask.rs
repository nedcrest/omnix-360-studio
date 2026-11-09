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

fn transpose<T: Copy + Default>(m: &[T], w: usize, h: usize) -> Vec<T> {
    let mut out = vec![T::default(); w * h];
    for y in 0..h {
        for x in 0..w {
            out[x * h + y] = m[y * w + x];
        }
    }
    out
}

/// How far each pixel of a `w`×`h` picture is from the nearest one
/// `inside`, in pixels (exact, in two passes: Felzenszwalb and
/// Huttenlocher's distance transform).
pub fn distance(inside: &[bool], (w, h): (usize, usize)) -> Vec<f32> {
    let far = ((w * w + h * h) as f64) * 4.0;
    let mut f: Vec<f64> = inside.iter().map(|&i| if i { 0.0 } else { far }).collect();
    f.par_chunks_mut(w).for_each(squared_1d);
    let mut f = transpose(&f, w, h);
    f.par_chunks_mut(h).for_each(squared_1d);
    transpose(&f, h, w).into_iter().map(|d| d.sqrt() as f32).collect()
}

/// A row's squared distances, given each pixel's own (0 inside, far
/// outside): the lower envelope of parabolas.
fn squared_1d(f: &mut [f64]) {
    let n = f.len();
    let (mut v, mut z) = (vec![0usize; n], vec![0f64; n + 1]);
    let (mut k, at) = (0usize, |q: usize, f: &[f64]| f[q] + (q * q) as f64);
    (z[0], z[1]) = (f64::NEG_INFINITY, f64::INFINITY);
    for q in 1..n {
        let mut s = (at(q, f) - at(v[k], f)) / (2 * q - 2 * v[k]) as f64;
        while s <= z[k] {
            k -= 1;
            s = (at(q, f) - at(v[k], f)) / (2 * q - 2 * v[k]) as f64;
        }
        k += 1;
        (v[k], z[k], z[k + 1]) = (q, s, f64::INFINITY);
    }
    let old = f.to_vec();
    k = 0;
    for (q, d) in f.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        *d = (q as f64 - v[k] as f64).powi(2) + old[v[k]];
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn distance_is_euclidean() {
        let (w, h) = (9, 7);
        let mut inside = vec![false; w * h];
        inside[3 * w + 4] = true;
        let d = super::distance(&inside, (w, h));
        assert_eq!((d[3 * w + 4], d[3 * w + 7], d[6 * w + 8]), (0.0, 3.0, 5.0));
    }
}
