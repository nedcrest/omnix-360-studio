//! A keyed value over time, whatever it holds (`CORE-API.md` step 8): the
//! effects' grade, an overlay's place, and later the automation lanes (gap
//! 39). «The key at t, or a new one there holding the value the line has
//! there» was written out for each kind; it's here once.

use crate::grade::{EffectKey, Grade, grade_at};
use crate::overlay::{MotionKey, Place, place_at};
use crate::view::{Ease, Timed};

/// A keyframe holding a value `V` that eases on to the next.
pub trait Key: Timed + Sized {
    type V: Copy;
    fn value_mut(&mut self) -> &mut Self::V;
    /// A smooth key at `t` holding `v`.
    fn make(t: f64, v: Self::V) -> Self;
    /// The value `keys` give at `t`.
    fn sample(keys: &[Self], t: f64) -> Self::V;
}

/// The key within `tolerance` of `t`.
pub fn find<K: Timed>(keys: &[K], t: f64, tolerance: f64) -> Option<usize> {
    keys.iter().position(|k| (k.time() - t).abs() <= tolerance)
}

/// The key within `tolerance` of `t`, or a new one at `t` holding what the
/// keys gave there, so nothing changes until it's set.
pub fn key_at<K: Key>(keys: &mut Vec<K>, t: f64, tolerance: f64) -> usize {
    find(keys, t, tolerance).unwrap_or_else(|| {
        let i = keys.partition_point(|k| k.time() <= t);
        let v = K::sample(keys, t);
        keys.insert(i, K::make(t, v));
        i
    })
}

impl Key for EffectKey {
    type V = Grade;
    fn value_mut(&mut self) -> &mut Grade {
        &mut self.grade
    }
    fn make(t: f64, grade: Grade) -> Self {
        EffectKey { t, grade, ease: Ease::Smooth }
    }
    fn sample(keys: &[Self], t: f64) -> Grade {
        grade_at(keys, t)
    }
}

impl Key for MotionKey {
    type V = Place;
    fn value_mut(&mut self) -> &mut Place {
        &mut self.place
    }
    fn make(t: f64, place: Place) -> Self {
        MotionKey { t, place, ease: Ease::Smooth }
    }
    fn sample(keys: &[Self], t: f64) -> Place {
        place_at(keys, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_key_holds_the_value_there_and_one_near_is_reused() {
        let mut keys = vec![EffectKey::make(0.0, Grade::default()), EffectKey::make(2.0, Grade { exposure: 1.0, ..Grade::default() })];
        keys[0].ease = Ease::Linear;
        let i = key_at(&mut keys, 1.0, 0.01);
        assert_eq!((i, keys.len()), (1, 3));
        assert!((keys[1].grade.exposure - 0.5).abs() < 1e-9, "halfway, linear: {}", keys[1].grade.exposure);
        assert_eq!(key_at(&mut keys, 1.005, 0.01), 1);
        assert_eq!(keys.len(), 3);
    }
}
