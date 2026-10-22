use super::*;
use crate::{Frame3, Vector3};

#[test]
fn legacy_polycurve_payload_remains_readable_and_arc_payload_is_strict() {
    let line = LineSegment::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let mut writer = Writer::default();
    writer.bytes.extend_from_slice(POLYCURVE_MAGIC);
    writer.u32(1);
    writer.len(1).unwrap();
    writer.f64(-3.0);
    writer.f64(9.0);
    writer.curve3(&line).unwrap();
    let decoded = decode_polycurve(&writer.bytes).unwrap();
    assert_eq!(decoded.parameters(), &[-3.0, 9.0]);
    assert_eq!(decoded.segments(), &[CurveSegment3::NurbsCurve(line)]);
    let circle = Circle3::try_new(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        2.0,
        Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let arc = CircularArc3::try_from_circle_sweep(circle, 1.0).unwrap();
    let mut legacy = Writer::default();
    legacy.u32(ARC_VERSION);
    legacy.u8(2);
    legacy.point3(arc.center());
    legacy.point3(Point3::try_from(arc.x_axis().as_vector().to_array()).unwrap());
    legacy.point3(Point3::try_from(arc.normal().unwrap().as_vector().to_array()).unwrap());
    legacy.f64(arc.radius());
    legacy.f64(arc.sweep_radians());
    legacy.f64(*arc.domain().start());
    legacy.f64(*arc.domain().end());
    assert_eq!(decode_arc(&legacy.bytes).unwrap(), arc);
    let bytes = encode_arc(arc).unwrap();
    assert_eq!(decode_arc(&bytes).unwrap(), arc);
    for length in 0..bytes.len() {
        assert!(decode_arc(&bytes[..length]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_arc(&trailing).is_err());
    let mut wrong_type = bytes;
    wrong_type[4] = 99;
    assert!(decode_arc(&wrong_type).is_err());
}

#[test]
fn polycurve_codec_preserves_local_domains_and_rejects_malformed_data() {
    let line = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        ],
        vec![4.0, 4.0, 9.0, 9.0],
    )
    .unwrap();
    let curve = PolyCurve3::try_with_segment_domains(vec![line], vec![-3.0, 7.0]).unwrap();
    let bytes = encode_polycurve(&curve).unwrap();
    assert_eq!(decode_polycurve(&bytes).unwrap(), curve);
    for count in 0..bytes.len() {
        assert!(
            decode_polycurve(&bytes[..count]).is_err(),
            "truncation at {count}"
        );
    }
    let mut invalid = bytes.clone();
    invalid.push(0);
    assert!(decode_polycurve(&invalid).is_err());
    for (range, value) in [
        (12..20, 0_u64.to_le_bytes()),
        (12..20, u64::MAX.to_le_bytes()),
        (20..28, f64::NAN.to_le_bytes()),
        (28..36, (-3.0_f64).to_le_bytes()),
    ] {
        let mut invalid = bytes.clone();
        invalid[range].copy_from_slice(&value);
        assert!(decode_polycurve(&invalid).is_err());
    }
}

#[test]
fn versioned_payload_round_trips_exact_solid_topology() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let rectangular_trim = Brep::try_rectangular_surface_face(
        NurbsSurface::try_bilinear([
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 1.0).unwrap(),
            Point3::try_new(10.0, 10.0, 3.0).unwrap(),
            Point3::try_new(0.0, 10.0, -1.0).unwrap(),
        ])
        .unwrap(),
        0.0..=0.4,
        0.0..=0.6,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for brep in [
        Brep::try_box(
            frame,
            [[-1.0, 2.0], [-2.0, 3.0], [-3.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
        Brep::try_cylinder(frame, 2.5, -3.0, 4.0, Tolerance::DEFAULT).unwrap(),
        Brep::try_cone(frame, 2.5, -4.0, Tolerance::DEFAULT).unwrap(),
        rectangular_trim,
    ] {
        let payload = encode_brep(&brep).unwrap();
        assert_eq!(decode_brep(&payload, Tolerance::DEFAULT).unwrap(), brep);
    }
}

#[test]
fn malformed_or_trailing_payload_is_rejected() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let brep = Brep::try_cone(frame, 1.0, 2.0, Tolerance::DEFAULT).unwrap();
    let mut payload = encode_brep(&brep).unwrap();
    payload.push(0);
    assert!(matches!(
        decode_brep(&payload, Tolerance::DEFAULT),
        Err(GeometryCodecError::Malformed)
    ));
    assert!(matches!(
        decode_brep(&payload[..12], Tolerance::DEFAULT),
        Err(GeometryCodecError::Malformed)
    ));
}
