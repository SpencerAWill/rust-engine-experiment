//! `rare_components`: a component only about 1% of entities have; each step updates the common
//! component of exactly those entities from the rare one (M3.25 spec §4.2).
//!
//! The rules here are the M3.25 spec §4.2; the common rules of M3.2 §4.4 apply too.

use engine::prelude::Prng;

use crate::{Fnv1a, Spec};

/// The `rare_components` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct RareComponents;

impl Spec for RareComponents {
    const NAME: &'static str = "rare_components";
    const SIZES: &'static [usize] = &[10_000, 100_000, 1_000_000];
}

/// An entity is rare when its rarity draw is below this.
const RARE_BELOW: f32 = 0.01;

/// The component every entity has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Common(pub f32);

/// The component about 1% of entities have. Never added or removed after `setup`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rare(pub f32);

/// Entity `i`'s components, three draws from `prng.stream(i)` in `0.0_f32..1.0`: `Common`'s
/// value; the rarity draw `d`; then, only if `d < 0.01`, `Rare`'s value. Without `Rare` there is
/// no third draw.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> (Common, Option<Rare>) {
    let mut r = prng.stream(i);
    let common = Common(r.range(0.0_f32..1.0));
    let d = r.range(0.0_f32..1.0);
    let rare = (d < RARE_BELOW).then(|| Rare(r.range(0.0_f32..1.0)));
    (common, rare)
}

/// The update rule: `x = common + rare`, then `x - 1.0` if `x >= 1.0`, else `x`. Values stay in
/// `0.0..1.0`.
#[inline]
pub fn update(common: &mut Common, rare: &Rare) {
    let x = common.0 + rare.0;
    common.0 = if x >= 1.0 { x - 1.0 } else { x };
}

/// One entity's term of the checksum: `f32(common)`, then `u8(0)` without `Rare`, or `u8(1)` and
/// `f32(rare)` with it.
#[must_use]
pub fn hash(common: &Common, rare: Option<&Rare>) -> u64 {
    let h = Fnv1a::new().f32(common.0);
    match rare {
        None => h.u8(0),
        Some(rare) => h.u8(1).f32(rare.0),
    }
    .finish()
}

#[cfg(test)]
mod tests {
    use engine::prelude::Prng;

    use super::{Common, Rare, hash, spawn, update};

    /// Replays entity `i`'s draws by hand: `(common, d, rare if d < 0.01)`.
    fn replay(prng: &Prng, i: u64) -> (f32, f32, Option<f32>) {
        let mut r = prng.stream(i);
        let common = r.range(0.0_f32..1.0);
        let d = r.range(0.0_f32..1.0);
        let rare = (d < 0.01).then(|| r.range(0.0_f32..1.0));
        (common, d, rare)
    }

    #[test]
    fn spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let first_rare = (0..10_000_u64)
            .find(|&i| replay(&prng, i).1 < 0.01)
            .expect("a rare entity exists");
        let first_common = (0..10_000_u64)
            .find(|&i| replay(&prng, i).1 >= 0.01)
            .expect("a non-rare entity exists");

        for i in [first_rare, first_common] {
            let (common, d, rare) = replay(&prng, i);
            let (got_common, got_rare) = spawn(&prng, i);
            assert_eq!(got_common, Common(common), "Common of entity {i}");
            assert_eq!(got_rare, rare.map(Rare), "Rare of entity {i}");
            assert_eq!(got_rare.is_some(), d < 0.01, "rarity of entity {i}");
        }
        assert!(spawn(&prng, first_rare).1.is_some());
        assert!(spawn(&prng, first_common).1.is_none());
    }

    #[test]
    fn rare_count_at_seed_42() {
        let prng = Prng::from_seed(42);
        let rare = (0..10_000_u64)
            .filter(|&i| {
                let mut r = prng.stream(i);
                let _ = r.range(0.0_f32..1.0);
                r.range(0.0_f32..1.0) < 0.01
            })
            .count();
        assert_eq!(rare, 78);
        let spawned = (0..10_000_u64)
            .filter(|&i| spawn(&prng, i).1.is_some())
            .count();
        assert_eq!(spawned, 78);
    }

    #[test]
    fn update_wraps() {
        let mut c = Common(0.25);
        update(&mut c, &Rare(0.5));
        assert_eq!(c, Common(0.75));

        let mut c = Common(0.75);
        update(&mut c, &Rare(0.5));
        assert_eq!(c, Common(0.75_f32 + 0.5_f32 - 1.0_f32));
        assert!(c.0 < 0.75);

        let mut c = Common(0.5);
        update(&mut c, &Rare(0.5));
        assert_eq!(c, Common(0.0));
    }

    #[test]
    fn hash_sees_common_and_rare() {
        let base = hash(&Common(0.25), Some(&Rare(0.5)));
        assert_ne!(base, hash(&Common(0.5), Some(&Rare(0.5))));
        assert_ne!(base, hash(&Common(0.25), Some(&Rare(0.75))));
        assert_ne!(base, hash(&Common(0.25), None));
        assert_ne!(hash(&Common(0.25), None), hash(&Common(0.5), None));
    }
}
