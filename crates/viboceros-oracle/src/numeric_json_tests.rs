//! Run with `cargo test -p viboceros-oracle`, independently of app/dev features.

#[test]
fn protocol_numbers_preserve_binary64_bits_in_standalone_builds() {
    for text in ["1.8952380418777466", "0.10520002990961075"] {
        let expected = text.parse::<f64>().unwrap();
        assert_eq!(
            serde_json::from_str::<f64>(text).unwrap().to_bits(),
            expected.to_bits()
        );
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(value.as_f64().unwrap().to_bits(), expected.to_bits());
    }
    let mut state = 719_u64;
    for bits in [
        0,
        1,
        1_u64 << 63,
        f64::MAX.to_bits(),
        f64::MIN_POSITIVE.to_bits(),
    ]
    .into_iter()
    .chain((0..10_000).map(|_| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state
    })) {
        let value = f64::from_bits(bits);
        if !value.is_finite() {
            continue;
        }
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.as_f64().unwrap().to_bits(), bits, "{encoded}");
    }
}
