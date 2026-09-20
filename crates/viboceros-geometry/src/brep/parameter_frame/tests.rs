use super::*;

pub(in crate::brep) fn translated_face(source: &BrepFace, offset: [Real; 2]) -> BrepFace {
    let mut face = source.clone();
    face.surface = NurbsSurface::try_new_rational(
        source.surface.degree_u(),
        source.surface.degree_v(),
        source.surface.control_point_count_u(),
        source.surface.control_point_count_v(),
        source.surface.control_points().to_vec(),
        source
            .surface
            .knots_u()
            .iter()
            .map(|k| k + offset[0])
            .collect(),
        source
            .surface
            .knots_v()
            .iter()
            .map(|k| k + offset[1])
            .collect(),
    )
    .unwrap();
    for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
        trim.curve = NurbsCurve2::try_new_rational(
            trim.curve.degree(),
            trim.curve
                .control_points()
                .iter()
                .map(|c| {
                    WeightedPoint2::try_new(
                        Point2::try_new(c.point().x() + offset[0], c.point().y() + offset[1])
                            .unwrap(),
                        c.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            trim.curve.knots().to_vec(),
        )
        .unwrap();
    }
    face
}

fn unit_face() -> BrepFace {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .faces
    .remove(0)
}

#[test]
fn parameter_frames_are_lossless_source_preserving_and_idempotent() {
    use crate::nurbs::exact::rational;
    let unit = unit_face();
    let frame = unit.local_parameter_frame().unwrap();
    assert!(matches!(frame.face, Cow::Borrowed(_)));
    assert!(std::ptr::eq(frame.face.as_ref(), &unit));
    for offset in [[1e12, -2e12], [-1e12, 2e12], [1e12, 0.], [0., -2e12]] {
        let source = translated_face(&unit, offset);
        let original = source.clone();
        let frame = source.local_parameter_frame().unwrap();
        assert!(matches!(frame.face, Cow::Owned(_)));
        assert_eq!(
            frame.face.surface.control_points(),
            source.surface.control_points()
        );
        for axis in 0..2 {
            let original_knots = [source.surface.knots_u(), source.surface.knots_v()][axis];
            let local_knots = [frame.face.surface.knots_u(), frame.face.surface.knots_v()][axis];
            for (&before, &after) in original_knots.iter().zip(local_knots) {
                assert_eq!(
                    rational(before),
                    rational(after) + rational(frame.origin[axis])
                );
            }
        }
        for (original_loop, local_loop) in source.loops.iter().zip(&frame.face.loops) {
            assert_eq!(original_loop.loop_type, local_loop.loop_type);
            for (before, after) in original_loop.trims.iter().zip(&local_loop.trims) {
                assert_eq!(before.vertices, after.vertices);
                assert_eq!(before.edge, after.edge);
                assert_eq!(before.reversed_3d, after.reversed_3d);
                assert_eq!(before.iso, after.iso);
                assert_eq!(before.trim_type, after.trim_type);
                assert_eq!(before.tolerance, after.tolerance);
                assert_eq!(before.curve.knots(), after.curve.knots());
                for (a, b) in before
                    .curve
                    .control_points()
                    .iter()
                    .zip(after.curve.control_points())
                {
                    assert_eq!(a.weight().to_bits(), b.weight().to_bits());
                    for axis in 0..2 {
                        assert_eq!(
                            rational(a.point().to_array()[axis]),
                            rational(b.point().to_array()[axis]) + rational(frame.origin[axis])
                        );
                    }
                }
            }
        }
        assert!(matches!(
            frame.face.local_parameter_frame().unwrap().face,
            Cow::Borrowed(_)
        ));
        assert_eq!(source, original);
    }
}

#[test]
fn parameter_frames_decline_lossy_axes_without_altering_other_axes() {
    let source = translated_face(&unit_face(), [1e12, -2e12]);
    for alter_knot in [false, true] {
        let mut face = source.clone();
        if alter_knot {
            let s = &face.surface;
            let mut knots = s.knots_u().to_vec();
            knots[0] = Real::from_bits(1);
            face.surface = NurbsSurface::try_new_rational(
                s.degree_u(),
                s.degree_v(),
                s.control_point_count_u(),
                s.control_point_count_v(),
                s.control_points().to_vec(),
                knots,
                s.knots_v().to_vec(),
            )
            .unwrap();
        } else {
            let trim = &mut face.loops[0].trims[0];
            let mut controls = trim.curve.control_points().to_vec();
            controls[0] = WeightedPoint2::try_new(
                Point2::try_new(Real::from_bits(1), controls[0].point().y()).unwrap(),
                controls[0].weight(),
            )
            .unwrap();
            trim.curve = NurbsCurve2::try_new_rational(
                trim.curve.degree(),
                controls,
                trim.curve.knots().to_vec(),
            )
            .unwrap();
        }
        let original = face.clone();
        let frame = face.local_parameter_frame().unwrap();
        assert_eq!(frame.origin[0], 0.);
        assert_ne!(frame.origin[1], 0.);
        assert_eq!(frame.face.surface.knots_u(), face.surface.knots_u());
        for (a, b) in face.loops[0].trims[0]
            .curve
            .control_points()
            .iter()
            .zip(frame.face.loops[0].trims[0].curve.control_points())
        {
            assert_eq!(a.point().x().to_bits(), b.point().x().to_bits());
        }
        assert_eq!(face, original);
    }
}

#[test]
fn lossless_parameter_differences_agree_with_exact_rationals() {
    use crate::nurbs::exact::rational;
    let mut values = vec![
        0.,
        -0.,
        Real::MAX,
        -Real::MAX,
        Real::MIN_POSITIVE,
        -Real::MIN_POSITIVE,
        Real::from_bits(1),
        -Real::from_bits(1),
        0.4,
        1.4,
        1e12,
        1e12 + 1.,
    ];
    let mut bits = 0x1234_5678_90ab_cdef_u64;
    for _ in 0..128 {
        bits ^= bits << 13;
        bits ^= bits >> 7;
        bits ^= bits << 17;
        let value = Real::from_bits(bits);
        if value.is_finite() {
            values.extend([value, value.next_up(), value.next_down()]);
        }
    }
    for &a in &values {
        for &b in &values {
            let result = exact_difference(a, b);
            let exact = rational(a) - rational(b);
            let expected = (a - b).is_finite() && exact == rational(a - b);
            assert_eq!(result.is_some(), expected, "a={a:?}, b={b:?}");
        }
    }
}
