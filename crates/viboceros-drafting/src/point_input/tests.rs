use super::*;
use viboceros_geometry::{Tolerance, Vector3};

fn plane() -> Frame3 {
    Frame3::try_from_directions(
        Point3::try_new(10.0, 20.0, 30.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn resolve(input: &str, previous: Option<Point3>) -> Result<Point3, PointInputError> {
    PointInput::parse(input)
        .unwrap()?
        .resolve(plane(), previous)
}

fn resolve_in_units(
    input: &str,
    units: &LengthUnitSystem,
    previous: Option<Point3>,
) -> Result<Point3, PointInputError> {
    PointInput::parse_with_units(input, units)
        .unwrap()?
        .resolve(plane(), previous)
}

#[test]
fn cartesian_prefixes_respect_world_plane_and_previous_point() {
    let previous = Some(Point3::try_new(5.0, 6.0, 7.0).unwrap());
    for (input, expected) in [
        ("0", [10.0, 20.0, 30.0]),
        ("w0", [0.0; 3]),
        ("1,2", [10.0, 21.0, 32.0]),
        ("1,2,3", [13.0, 21.0, 32.0]),
        ("w1,2,3", [1.0, 2.0, 3.0]),
        ("r1,2,3", [8.0, 7.0, 9.0]),
        ("@1,2", [5.0, 7.0, 9.0]),
        ("wr1,2,3", [6.0, 8.0, 10.0]),
        ("RW1,2,3", [6.0, 8.0, 10.0]),
        ("@w1,2", [6.0, 8.0, 7.0]),
    ] {
        assert_eq!(
            resolve(input, previous).unwrap().to_array(),
            expected,
            "{input}"
        );
    }
    assert_eq!(
        resolve("r1,2", None),
        Err(PointInputError::MissingPreviousPoint)
    );
}

#[test]
fn polar_and_spherical_coordinates_have_exact_quadrants() {
    for (input, expected) in [
        ("w5<0", [5.0, 0.0, 0.0]),
        ("w5<90,8", [0.0, 5.0, 8.0]),
        ("w5<180", [-5.0, 0.0, 0.0]),
        ("w5<-90", [0.0, -5.0, 0.0]),
        ("w5<30<90", [0.0, 0.0, 5.0]),
        ("w5<180<-90", [0.0, 0.0, -5.0]),
        ("w5<810<450", [0.0, 0.0, 5.0]),
        ("w5<30<270", [0.0, 0.0, -5.0]),
        ("w1e300<90", [0.0, 1e300, 0.0]),
    ] {
        assert_eq!(
            resolve(input, None).unwrap().to_array(),
            expected,
            "{input}"
        );
    }
    let p = resolve("w2<45<30", None).unwrap();
    assert!((p.x() - 1.5_f64.sqrt()).abs() < 1e-14);
    assert!((p.y() - 1.5_f64.sqrt()).abs() < 1e-14);
    assert!((p.z() - 1.0).abs() < 1e-14);
    assert!(resolve("w1<1e308", None).is_ok());
    let near_axis = resolve("w1e16<-1e-14", None).unwrap();
    assert_eq!(near_axis.x(), 1e16);
    assert!((near_axis.y() + std::f64::consts::PI / 1.8).abs() < 1e-14);
    let negative = resolve("w-4<30<45", None).unwrap();
    assert!((negative.x() + 6.0_f64.sqrt()).abs() < 1e-14);
    assert!((negative.y() + 2.0_f64.sqrt()).abs() < 1e-14);
    assert!((negative.z() - 8.0_f64.sqrt()).abs() < 1e-14);
    let below = resolve("w-5<30<-30", None).unwrap();
    assert!((below.x() + 3.75).abs() < 1e-14);
    assert!((below.y() + 1.25 * 3.0_f64.sqrt()).abs() < 1e-14);
    assert!((below.z() + 2.5).abs() < 1e-14);
    for input in [
        "w5<30<120",
        "w-5<30<120",
        "w-5<30<-120",
        "w5<0<180",
        "w5<0<480",
    ] {
        assert_eq!(
            resolve(input, None),
            Err(PointInputError::ElevationRange),
            "{input}"
        );
    }
}

#[test]
fn cartesian_horizontal_components_accept_spherical_elevation() {
    let previous = Some(Point3::try_new(5.0, 6.0, 7.0).unwrap());
    for (input, expected) in [
        ("w3,4<30", [3.0, 4.0, 5.0 / 3.0_f64.sqrt()]),
        ("wr-3,4<45", [2.0, 10.0, 12.0]),
        ("3,4<30", [10.0 + 5.0 / 3.0_f64.sqrt(), 23.0, 34.0]),
        ("r-3,4<-45", [0.0, 3.0, 11.0]),
        ("w3,4<0", [3.0, 4.0, 0.0]),
        ("w0,0<90", [0.0, 0.0, 0.0]),
    ] {
        let actual = resolve(input, previous).unwrap().to_array();
        for (component, target) in actual.into_iter().zip(expected) {
            assert!((component - target).abs() <= 2e-14, "{input}: {actual:?}");
        }
    }
    assert_eq!(
        resolve("w1,0<90", None),
        Err(PointInputError::InvalidNumber)
    );
    assert_eq!(
        resolve("w1,0<120", None),
        Err(PointInputError::ElevationRange)
    );
    assert_eq!(resolve("w1,2,3<30", None), Err(PointInputError::Syntax));
    assert_eq!(resolve("w1,,2<30", None), Err(PointInputError::Syntax));
    assert_eq!(
        resolve("w1e308,1e308<0", None).unwrap().to_array(),
        [1e308, 1e308, 0.0]
    );
    let large = resolve("w1e308,1e308<30", None).unwrap();
    assert_eq!(large.x(), 1e308);
    assert_eq!(large.y(), 1e308);
    assert!((large.z() / 1e308 - (2.0_f64 / 3.0).sqrt()).abs() < 1e-15);
    assert_eq!(
        resolve("w1e308,1e308<89", None),
        Err(PointInputError::InvalidNumber)
    );
}

#[test]
fn arithmetic_and_mixed_fractions_resolve_in_cartesian_and_polar_inputs() {
    let previous = Some(Point3::try_new(5.0, 6.0, 7.0).unwrap());
    for (input, expected) in [
        ("w5/16,1-3/4", [0.3125, 1.75, 0.0]),
        ("wr(10-3)/7,1+1/2", [6.0, 7.5, 7.0]),
        ("2*(1+1)<90", [10.0, 20.0, 34.0]),
        ("r1-1/2<180<30", [5.75, 4.700961894323342, 7.0]),
    ] {
        let actual = resolve(input, previous).unwrap().to_array();
        for (component, target) in actual.into_iter().zip(expected) {
            assert!((component - target).abs() < 2e-14, "{input}: {actual:?}");
        }
    }
    for input in ["1/0,2", "2*(3+4,1", "1-2/0,3", "w1+,2"] {
        assert!(resolve(input, previous).is_err(), "{input}");
    }
}

#[test]
fn functions_constants_and_units_resolve_in_coordinate_components() {
    let previous = Some(Point3::try_new(5.0, 6.0, 7.0).unwrap());
    let world = resolve("w10*sin(30degrees),10*cos(30degrees)", None).unwrap();
    assert!((world.x() - 5.0).abs() < 2e-14);
    assert!((world.y() - 5.0 * 3.0_f64.sqrt()).abs() < 2e-14);
    let local = resolve("atan2(1,1),pow(2,3)", None).unwrap();
    assert_eq!(local.x(), 10.0);
    assert!((local.y() - (20.0 + std::f64::consts::FRAC_PI_4)).abs() < 2e-14);
    assert_eq!(local.z(), 38.0);
    for input in ["5<30degrees", "5<pi/6radians", "5<100/3gradians"] {
        let point = resolve(input, None).unwrap();
        assert_eq!(point.x(), 10.0);
        assert!(
            (point.y() - (20.0 + 2.5 * 3.0_f64.sqrt())).abs() < 1e-13,
            "{input}"
        );
        assert!((point.z() - 32.5).abs() < 1e-13, "{input}");
    }
    let relative = resolve("r2<pi/2radians,1", previous).unwrap();
    assert_eq!(relative.to_array(), [6.0, 6.0, 9.0]);
    for input in ["sqrt(-1),0", "unknown(1),0", "pow(2),0", "1,atan2(1,)"] {
        assert!(resolve(input, None).is_err(), "{input}");
    }
    assert_eq!(
        resolve("w5<sin(30degrees)", None),
        Err(PointInputError::Syntax)
    );
}

#[test]
fn explicit_length_units_follow_the_active_model_units() {
    for (units, expected) in [
        (LengthUnitSystem::Millimeters, [270.0, 1000.0, 0.0]),
        (LengthUnitSystem::Meters, [0.27, 1.0, 0.0]),
        (LengthUnitSystem::Inches, [270.0 / 25.4, 1000.0 / 25.4, 0.0]),
    ] {
        let actual = resolve_in_units("w27cm,1m", &units, None)
            .unwrap()
            .to_array();
        for (coordinate, target) in actual.into_iter().zip(expected) {
            assert!((coordinate - target).abs() < 1e-12, "{units:?}: {actual:?}");
        }
    }
    let feet = resolve_in_units("w1'2-3/4\",1/2in", &LengthUnitSystem::Millimeters, None).unwrap();
    assert!((feet.x() - 374.65).abs() < 1e-12);
    assert!((feet.y() - 12.7).abs() < 1e-12);
    assert_eq!(
        resolve_in_units("w1m+20,0", &LengthUnitSystem::Millimeters, None),
        Err(PointInputError::InvalidNumber)
    );
}

#[test]
fn invalid_point_input_is_not_confused_with_a_command() {
    for input in [
        "Line 0,0 1,2",
        "Rotate",
        "Rebuild Degree=3",
        "Weld",
        "Undo",
        "",
        "_Line",
    ] {
        assert_eq!(PointInput::parse(input), None, "{input}");
    }
    for input in [
        "1,,2", "1,2,3,4", "1, 2", "rr1,2", "ww1,2", "1<2<3<4", "1<NaN", "1e309,0", "NaN", "wInf",
        "NaN,0", "wInf,0", "w 1,2", "rw 1,2", "bad,1", "@", "rw",
    ] {
        assert!(PointInput::parse(input).unwrap().is_err(), "{input}");
    }
    assert_eq!(
        PointInput::parse("5"),
        Some(Err(PointInputError::DistanceConstraint))
    );
    assert_eq!(
        PointInput::parse("pi"),
        Some(Err(PointInputError::DistanceConstraint))
    );
    assert_eq!(
        PointInput::parse("sin(30degrees)"),
        Some(Err(PointInputError::DistanceConstraint))
    );
    assert!(
        resolve(
            "rw1e308,0,0",
            Some(Point3::try_new(1e308, 0.0, 0.0).unwrap())
        )
        .is_err()
    );
}
