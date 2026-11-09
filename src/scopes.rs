//! Scopes (gap 69): a histogram and a luma waveform of the picture shown,
//! to see whether it's clipped and whether its black is black (a film's
//! darkest was 14 of 255, gap 14). Measured from the cut's finished
//! picture (graded, its LUT and style on; overlays apart), read back from
//! the player's GPU a few times a second: plain counting here.

/// Levels a channel has (8-bit display values).
pub const LEVELS: usize = 256;
/// The waveform's columns, across the picture.
pub const COLUMNS: usize = 256;
/// The share of pixels at either end that doesn't count as the picture's
/// darkest or brightest (a dead pixel, a speck).
const SPECK: f64 = 0.001;
/// At or under this the picture is crushed, at or over this blown.
pub const CRUSHED: u8 = 2;
pub const BLOWN: u8 = 253;

/// What a picture holds, by level.
#[derive(Clone, Debug, PartialEq)]
pub struct Scopes {
    /// Pixels at each level: red, green, blue, luma (Rec. 709 weights on
    /// the display values).
    pub histogram: [[u32; LEVELS]; 4],
    /// Pixels at each luma level in each column, `COLUMNS` × `LEVELS`,
    /// column by column.
    pub waveform: Vec<u32>,
    pub pixels: u32,
    /// Its darkest and brightest luma, specks aside.
    pub black: u8,
    pub white: u8,
    /// The shares of pixels crushed and blown.
    pub crushed: f64,
    pub blown: f64,
}

/// Rec. 709's luma of display values.
fn luma(r: u8, g: u8, b: u8) -> u8 {
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32).round().min(255.0) as u8
}

/// Measures a `w` × `h` picture of 4 bytes a pixel, rows `row` bytes
/// apart, its bytes blue first if `bgra`; every `step`th pixel each way.
pub fn measure(data: &[u8], (w, h): (usize, usize), row: usize, bgra: bool, step: usize) -> Scopes {
    let step = step.max(1);
    let mut histogram = [[0u32; LEVELS]; 4];
    let mut waveform = vec![0u32; COLUMNS * LEVELS];
    let mut pixels = 0u32;
    for y in (0..h).step_by(step) {
        let line = &data[y * row..y * row + w * 4];
        for x in (0..w).step_by(step) {
            let p = &line[x * 4..x * 4 + 4];
            let (r, g, b) = if bgra { (p[2], p[1], p[0]) } else { (p[0], p[1], p[2]) };
            let l = luma(r, g, b);
            for (k, v) in [r, g, b, l].into_iter().enumerate() {
                histogram[k][v as usize] += 1;
            }
            waveform[(x * COLUMNS / w) * LEVELS + l as usize] += 1;
            pixels += 1;
        }
    }
    let at = |share: f64| {
        let want = (pixels as f64 * share).ceil() as u32;
        let mut seen = 0;
        histogram[3].iter().position(|&n| {
            seen += n;
            seen >= want.max(1)
        })
    };
    let share = |n: u32| if pixels == 0 { 0.0 } else { n as f64 / pixels as f64 };
    Scopes {
        black: at(SPECK).unwrap_or(0) as u8,
        white: at(1.0 - SPECK).unwrap_or(255) as u8,
        crushed: share(histogram[3][..=CRUSHED as usize].iter().sum()),
        blown: share(histogram[3][BLOWN as usize..].iter().sum()),
        histogram,
        waveform,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `w` × 2 picture: a dark row over a light one, the light one
    /// brightening left to right.
    fn picture(w: usize) -> Vec<u8> {
        let mut data = Vec::new();
        for _ in 0..w {
            data.extend([14, 14, 14, 255]);
        }
        for x in 0..w {
            let v = (128 + x * 127 / (w - 1)) as u8;
            data.extend([v, v, v, 255]);
        }
        data
    }

    #[test]
    fn a_pictures_levels_are_counted_and_its_ends_found() {
        let s = measure(&picture(256), (256, 2), 256 * 4, false, 1);
        assert_eq!((s.pixels, s.histogram[3][14], s.black, s.white), (512, 256, 14, 255));
        assert_eq!((s.crushed, s.blown), (0.0, 5.0 / 512.0), "the last five columns at 253 and over");
        // The waveform: the light row climbs across the columns.
        assert_eq!((s.waveform[14], s.waveform[128], s.waveform[255 * LEVELS + 255]), (1, 1, 1));
        // Blue first reads the same of grey, and a step skips pixels.
        assert_eq!(measure(&picture(256), (256, 2), 1024, true, 2).pixels, 128);
    }

    #[test]
    fn colour_goes_to_its_channel_and_luma_weighs_green() {
        let red = [255u8, 0, 0, 255].repeat(4);
        let s = measure(&red, (2, 2), 8, false, 1);
        assert_eq!((s.histogram[0][255], s.histogram[1][0], s.histogram[3][54]), (4, 4, 4));
        let s = measure(&red, (2, 2), 8, true, 1);
        assert_eq!(s.histogram[2][255], 4, "the first byte is blue");
    }
}
