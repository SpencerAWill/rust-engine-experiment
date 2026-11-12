//! Edge cases of the sampling API: full-width ranges, invalid ranges, degenerate inputs.

use engine_random::Prng;

#[test]
fn full_range_signed_and_unsigned() {
    let mut p = Prng::from_seed(9);
    let v: Vec<i64> = (0..64).map(|_| p.range(i64::MIN..=i64::MAX)).collect();
    assert!(v.iter().any(|x| *x < 0) && v.iter().any(|x| *x > 0));
    let _ = p.range(0..=u64::MAX);
    let _ = p.range(i8::MIN..=i8::MAX);
    let all: Vec<i8> = (0..2000).map(|_| p.range(i8::MIN..=i8::MAX)).collect();
    assert!(
        all.contains(&i8::MIN) && all.contains(&i8::MAX),
        "extremes are reachable"
    );
}

#[test]
#[allow(
    clippy::reversed_empty_ranges,
    reason = "invalid ranges are the point of this test"
)]
fn invalid_ranges_return_none_without_drawing() {
    let mut p = Prng::from_seed(1);
    let before = p.clone();
    assert_eq!(p.try_range(5..5), None);
    assert_eq!(p.try_range(6..=5), None);
    assert_eq!(p.try_range(1.0_f32..1.0), None);
    assert_eq!(p.try_range(-f32::MAX..f32::MAX), None); // hi - lo overflows
    assert_eq!(p.try_range(f64::NAN..1.0), None);
    assert_eq!(p.try_range(0.0_f64..f64::INFINITY), None);
    assert_eq!(p.try_range(1.0_f64..=0.0), None);
    assert_eq!(p, before, "an invalid range must not advance the generator");
}

#[test]
fn inclusive_degenerate_and_bounds() {
    let mut p = Prng::from_seed(2);
    assert_eq!(p.range(3.5_f32..=3.5).to_bits(), 3.5_f32.to_bits());
    assert_eq!(p.range(7..=7), 7);
    assert!((0..10_000).all(|_| (0.0..1.0).contains(&p.range(0.0_f32..1.0))));
    assert!((0..10_000).all(|_| (-1.0..=1.0).contains(&p.range(-1.0_f64..=1.0))));
    assert!((0..10_000).all(|_| (-5..5).contains(&p.range(-5_i32..5))));
}

#[test]
fn chance_edges() {
    let mut p = Prng::from_seed(3);
    assert!((0..100).all(|_| !p.chance(0.0)));
    assert!((0..100).all(|_| p.chance(1.0)));
    assert!((0..100).all(|_| !p.chance(f64::NAN)));
    assert!((0..100).all(|_| !p.chance(-1.0)));
    assert!((0..100).all(|_| p.chance(2.0)));
}

#[test]
fn choose_and_shuffle_edges() {
    let mut p = Prng::from_seed(4);
    let before = p.clone();
    assert_eq!(p.choose::<u8>(&[]), None);
    let mut empty: [u8; 0] = [];
    p.shuffle(&mut empty);
    let mut one = [1];
    p.shuffle(&mut one);
    assert_eq!(one, [1]);
    assert_eq!(p, before, "these cases draw nothing");
    assert_eq!(p.choose(&[42]), Some(&42));
    assert_ne!(p, before, "choosing from one element still draws once");
}

#[test]
fn fill_bytes_is_little_endian_prefix() {
    for len in 0..=17 {
        let mut a = Prng::from_seed(5);
        let mut b = a.clone();
        let mut buf = vec![0xAA_u8; len];
        a.fill_bytes(&mut buf);

        let words = len.div_ceil(8);
        let expected: Vec<u8> = (0..words)
            .flat_map(|_| b.next_u64().to_le_bytes())
            .collect();
        assert_eq!(buf, expected[..len], "len {len}");
        assert_eq!(a, b, "len {len}: both consumed the same number of words");
    }
}

#[test]
fn shuffle_is_a_permutation() {
    let mut p = Prng::from_seed(6);
    let mut v: Vec<u32> = (0..100).collect();
    p.shuffle(&mut v);
    let mut sorted = v.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..100).collect::<Vec<u32>>());
    assert_ne!(
        v, sorted,
        "100 elements are overwhelmingly unlikely to stay sorted"
    );
}
