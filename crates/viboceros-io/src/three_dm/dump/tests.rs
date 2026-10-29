use super::*;
use viboceros_geometry::{AffineTransform3, CurveSegment3, SubD};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn native_type_reports_preserve_every_source_without_files() {
    let t = Tolerance::DEFAULT;
    let line = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), t).unwrap();
    let points = vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 3., 0.), p(0., 3., 0.)];
    let surface = NurbsSurface::try_bilinear(points.clone().try_into().unwrap()).unwrap();
    let curve = NurbsCurve::try_new_rational(
        2,
        [p(0., 0., 0.), p(1., 2., 0.), p(3., 0., 0.)]
            .map(|p| WeightedPoint3::try_new(p, -7.).unwrap())
            .to_vec(),
        vec![-3., -3., -3., 8., 8., 8.],
    )
    .unwrap();
    let sources = vec![
        (ThreeDmGeometry::Point(p(1., 2., 3.)), "ON_Point"),
        (
            ThreeDmGeometry::PointCloud(PointCloud3::try_new(points.clone()).unwrap()),
            "ON_PointCloud",
        ),
        (ThreeDmGeometry::Line(line), "ON_LineCurve"),
        (
            ThreeDmGeometry::Arc(
                CircularArc3::try_from_three_points(p(0., 0., 0.), p(1., 1., 0.), p(2., 0., 0.), t)
                    .unwrap(),
            ),
            "ON_ArcCurve",
        ),
        (ThreeDmGeometry::NurbsCurve(curve), "ON_NurbsCurve"),
        (
            ThreeDmGeometry::Polyline(Polyline3::try_new(points.clone(), t).unwrap()),
            "ON_PolylineCurve",
        ),
        (
            ThreeDmGeometry::PolyCurve(
                PolyCurve3::try_new(vec![CurveSegment3::Line(line)]).unwrap(),
            ),
            "ON_PolyCurve",
        ),
        (
            ThreeDmGeometry::NurbsSurface(surface.clone()),
            "ON_NurbsSurface",
        ),
        (
            ThreeDmGeometry::Brep(Brep::try_surface_face(surface, t).unwrap()),
            "ON_Brep",
        ),
        (
            ThreeDmGeometry::Brep(
                Brep::try_box_extrusion(
                    Frame3::try_from_normal(
                        p(0., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        t,
                    )
                    .unwrap(),
                    [[0., 2.], [0., 3.]],
                    4.,
                    t,
                )
                .unwrap(),
            ),
            "ON_Extrusion",
        ),
        (
            ThreeDmGeometry::Mesh(
                TriangleMesh::try_new(points.clone(), vec![[0, 1, 2], [0, 2, 3]], t).unwrap(),
            ),
            "ON_Mesh",
        ),
        (ThreeDmGeometry::EmptyMesh, "ON_Mesh"),
        (
            ThreeDmGeometry::SubD(SubD::try_from_polygons(points, vec![vec![0, 1, 2, 3]]).unwrap()),
            "SubD",
        ),
    ];
    for (source, name) in sources {
        let before = source.clone();
        let result = dump_3dm_geometry(&source).unwrap();
        assert!(result.text.contains(name), "{name}: {}", result.text);
        assert!(!result.truncated);
        assert_eq!(source, before);
    }
}

#[test]
fn instance_report_uses_its_real_definition_uuid() {
    let id = "01234567-89ab-cdef-0123-456789abcdef";
    let transform = AffineTransform3::from_translation(Vector3::try_new(1., 2., 3.).unwrap());
    let result = dump_3dm_instance_reference(id, transform).unwrap();
    assert!(result.text.to_ascii_lowercase().contains(id));
    assert!(!result.truncated);
    assert!(dump_3dm_instance_reference("bad UUID", transform).is_err());
    assert!(
        dump_3dm_geometry(&ThreeDmGeometry::InstanceReference {
            definition_index: 0,
            transform
        })
        .is_err()
    );
}

#[test]
fn large_control_arrays_produce_an_explicit_bounded_report() {
    let count = 10000;
    let points = (0..count)
        .map(|i| p(i as f64, (i % 2) as f64, 0.))
        .collect();
    let knots = [
        vec![0.],
        (0..count).map(|i| i as f64).collect(),
        vec![(count - 1) as f64],
    ]
    .concat();
    let curve = ThreeDmGeometry::NurbsCurve(NurbsCurve::try_new(1, points, knots).unwrap());
    let result = dump_3dm_geometry(&curve).unwrap();
    assert!(result.truncated);
    assert!(result.text.len() < 128 * 1024);
    assert!(result.text.contains("cv_count = 10000"));
}

#[test]
fn malformed_arguments_and_short_output_buffers_do_not_overwrite_canaries() {
    let payload =
        diagnostics::DiagnosticPayload::new(&ThreeDmGeometry::Point(p(1., 2., 3.))).unwrap();
    let view = payload.view();
    for capacity in [0, 1, 2, 8, 512] {
        let mut text = [127 as c_char; 512];
        let mut truncated = -1;
        let status = unsafe {
            ffi::vibo_geometry_dump(
                &view,
                std::ptr::null(),
                &mut truncated,
                text.as_mut_ptr(),
                capacity,
            )
        };
        assert_eq!(status, i32::from(capacity >= 2));
        if capacity > 0 {
            assert!(text[..capacity].contains(&0));
        }
        assert!(text[capacity..].iter().all(|&v| v == 127 as c_char));
        if capacity >= 2 {
            assert_eq!(truncated, i32::from(capacity < 512));
        }
    }
    let mut text = [0 as c_char; 512];
    let mut truncated = -1;
    assert_eq!(
        unsafe {
            ffi::vibo_geometry_dump(
                std::ptr::null(),
                std::ptr::null(),
                &mut truncated,
                text.as_mut_ptr(),
                text.len(),
            )
        },
        0
    );
    assert_eq!(truncated, 0);
}
