use super::*;
use viboceros_geometry::{AffineTransform3, CurveSegment3};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn surface() -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(0., 0., 0.), p(2., 0., 0.), p(0., 3., 0.), p(2., 3., 0.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
#[test]
fn exact_native_representatives_are_checked_without_mutating_sources() {
    let t = Tolerance::DEFAULT;
    let line = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), t).unwrap();
    let nurbs = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 2., 0.), 2.).unwrap(),
            WeightedPoint3::try_new(p(2., 0., 0.), 1.).unwrap(),
        ],
        vec![-3., -3., -3., 8., 8., 8.],
    )
    .unwrap();
    let s = surface();
    let b = Brep::try_surface_face(s.clone(), t).unwrap();
    let sources = vec![
        ThreeDmGeometry::Point(p(0., 0., 0.)),
        ThreeDmGeometry::PointCloud(
            PointCloud3::try_new(vec![p(0., 0., 0.), p(1., 2., 3.)]).unwrap(),
        ),
        ThreeDmGeometry::Line(line),
        ThreeDmGeometry::NurbsCurve(nurbs),
        ThreeDmGeometry::Polyline(
            Polyline3::try_new(vec![p(0., 0., 0.), p(1., 0., 0.), p(2., 1., 0.)], t).unwrap(),
        ),
        ThreeDmGeometry::PolyCurve(
            PolyCurve3::try_new(vec![
                CurveSegment3::Line(line),
                CurveSegment3::Line(LineSegment::try_new(p(2., 0., 0.), p(3., 1., 0.), t).unwrap()),
            ])
            .unwrap(),
        ),
        ThreeDmGeometry::NurbsSurface(s),
        ThreeDmGeometry::Brep(b),
        ThreeDmGeometry::Mesh(
            TriangleMesh::try_new(
                vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)],
                vec![[0, 1, 2]],
                t,
            )
            .unwrap(),
        ),
    ];
    for source in sources {
        let before = source.clone();
        let result = check_3dm_geometry(&source);
        assert_eq!(
            result.status,
            GeometryCheckStatus::Valid,
            "{source:?}: {result:?}"
        );
        assert_eq!(source, before);
    }
}
#[test]
fn collapsed_positions_are_bad_but_structural_validity_does_not_require_area() {
    for (vertices, expected) in [
        (
            vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 0., 0.)],
            GeometryCheckStatus::Invalid,
        ),
        (
            vec![p(0., 0., 0.), p(1., 0., 0.), p(2., 0., 0.)],
            GeometryCheckStatus::Valid,
        ),
    ] {
        let mesh =
            TriangleMesh::try_from_face_records(vertices, vec![MeshFace::Triangle([0, 1, 2])])
                .unwrap();
        assert!(mesh.validate_face_geometry(Tolerance::DEFAULT).is_err());
        let result = check_3dm_geometry(&ThreeDmGeometry::Mesh(mesh));
        assert_eq!(result.status, expected, "{result:?}");
        if expected == GeometryCheckStatus::Invalid {
            assert!(!result.diagnostics.is_empty());
        }
    }
}
#[test]
fn instance_context_and_native_reference_validity_are_separate() {
    let transform = AffineTransform3::from_translation(Vector3::try_new(1., 2., 3.).unwrap());
    assert_eq!(
        check_3dm_geometry(&ThreeDmGeometry::InstanceReference {
            definition_index: 0,
            transform
        })
        .status,
        GeometryCheckStatus::NotChecked
    );
    assert_eq!(
        check_3dm_instance_reference("not a UUID", transform).status,
        GeometryCheckStatus::NotChecked
    );
    assert_eq!(
        check_3dm_instance_reference("01234567-89ab-cdef-0123-456789abcdef", transform).status,
        GeometryCheckStatus::Valid
    );
    let invalid = check_3dm_instance_reference("00000000-0000-0000-0000-000000000000", transform);
    assert_eq!(invalid.status, GeometryCheckStatus::Invalid);
    assert!(invalid.diagnostics.contains("nil"));
}
#[test]
fn malformed_native_payloads_are_not_classified_as_bad_geometry() {
    let mut valid = 123;
    let mut text = [0 as c_char; 512];
    let result = unsafe {
        ffi::vibo_geometry_check(
            std::ptr::null(),
            std::ptr::null(),
            &mut valid,
            text.as_mut_ptr(),
            text.len(),
        )
    };
    assert_eq!((result, valid), (0, -1));
    let view = ffi::ViboWriteObject {
        object_type: OBJECT_NURBS_CURVE,
        ..Default::default()
    };
    let result = unsafe {
        ffi::vibo_geometry_check(
            &view,
            std::ptr::null(),
            &mut valid,
            text.as_mut_ptr(),
            text.len(),
        )
    };
    assert_eq!((result, valid), (0, -1));
    assert!(
        unsafe { CStr::from_ptr(text.as_ptr()) }
            .to_string_lossy()
            .contains("dimensions")
    );
}

#[test]
fn remote_mesh_double_positions_do_not_collapse_to_invalid_float_faces() {
    let mesh = TriangleMesh::try_from_face_records(
        vec![p(1e6, 0., 0.), p(1e6 + 1e-4, 0., 0.), p(1e6, 1e-4, 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    let result = check_3dm_geometry(&ThreeDmGeometry::Mesh(mesh));
    assert_eq!(result.status, GeometryCheckStatus::Valid, "{result:?}");
}

#[test]
fn native_validity_failure_is_distinct_from_transfer_rejection_and_buffers_are_bounded() {
    let points = [
        0., 0., 0., 1., 1., 0., 0., 1., 2., 0., 0., 1., 3., 0., 0., 1.,
    ];
    let knots = [0., 0., 2., 1., 3., 3.]; // Inconsistent ordering, with otherwise safe dimensions.
    let view = ffi::ViboWriteObject {
        object_type: OBJECT_NURBS_CURVE,
        degree_u: 1,
        control_point_count_u: 4,
        coordinates: points.as_ptr(),
        coordinate_count: points.len(),
        knots_u: knots.as_ptr(),
        knot_u_count: knots.len(),
        ..Default::default()
    };
    for capacity in [0, 1, 4, 512] {
        let mut valid = -1;
        let mut text = [127 as c_char; 512];
        let result = unsafe {
            ffi::vibo_geometry_check(
                &view,
                std::ptr::null(),
                &mut valid,
                text.as_mut_ptr(),
                capacity,
            )
        };
        assert_eq!((result, valid), (1, 0));
        if capacity > 0 {
            assert!(text[..capacity].contains(&0));
        }
        assert!(text[capacity..].iter().all(|&b| b == 127 as c_char));
        assert_eq!(knots, [0., 0., 2., 1., 3., 3.]);
    }
}
