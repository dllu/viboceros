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
    assert!(
        resolve(
            "rw1e308,0,0",
            Some(Point3::try_new(1e308, 0.0, 0.0).unwrap())
        )
        .is_err()
    );
}
