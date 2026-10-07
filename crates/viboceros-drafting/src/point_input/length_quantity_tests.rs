use super::*;

#[test]
fn scalar_lengths_keep_zero_calculator_expressions_and_explicit_units() {
    for (text, expected) in [
        ("0", 0.),
        ("1-1", 0.),
        ("2", 2.),
        ("-2", -2.),
        ("1/2", 0.5),
        ("sqrt(4)", 2.),
        ("2mm", 2.),
    ] {
        assert_eq!(
            PointInput::parse_length_with_units(text, &LengthUnitSystem::Millimeters)
                .unwrap()
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        PointInput::parse_length_with_units("2mm", &LengthUnitSystem::Meters)
            .unwrap()
            .unwrap(),
        0.002
    );
    for text in ["NaN", "inf", "1/0", "2 + 3"] {
        assert!(
            PointInput::parse_length_with_units(text, &LengthUnitSystem::Millimeters)
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn length_parser_preserves_point_angle_and_command_routing() {
    for text in [
        "w0",
        "r0",
        "@1,2",
        "1,2,3",
        "<30",
        "2<45",
        "Line",
        "Line 1,2 3,4",
    ] {
        assert!(
            PointInput::parse_length_with_units(text, &LengthUnitSystem::Millimeters).is_none(),
            "{text}"
        );
    }
}
