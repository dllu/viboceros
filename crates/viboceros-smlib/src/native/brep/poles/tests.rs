use super::*;
use viboceros_geometry::{Frame3, Vector3};

fn sphere() -> NurbsSurface {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    NurbsSurface::try_sphere(frame, 2.).unwrap()
}

#[test]
fn pole_reconstruction_is_independent_of_axis_scales_origins_and_face_sense() {
    let tolerance = Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap();
    for [u, v] in [
        [0. ..=2_f64.powi(-360), 0. ..=2_f64.powi(360)],
        [0. ..=2_f64.powi(360), 0. ..=2_f64.powi(-360)],
        [0. ..=1e-4, 0. ..=1e-4],
        [1e12..=1e12 + 4., -2e12..=-2e12 + 8.],
    ] {
        let surface = sphere().try_reparameterized(u, v).unwrap();
        for swapped in [false, true] {
            let surface = if swapped {
                surface.try_swapped_uv().unwrap()
            } else {
                surface.clone()
            };
            let source = Brep::try_surface_face(surface, tolerance).unwrap();
            for source in [source.clone(), source.reversed()] {
                let face = &source.faces()[0];
                let native = face.loops()[0]
                    .trims()
                    .iter()
                    .filter(|trim| trim.edge().is_some())
                    .cloned()
                    .collect();
                let restored =
                    close_pole_gaps(face.surface(), source.vertices(), native, tolerance).unwrap();
                assert_eq!(restored.len(), face.loops()[0].trims().len());
                assert_eq!(
                    restored.iter().filter(|trim| trim.edge().is_none()).count(),
                    2
                );
                let output = Brep::try_new(
                    source.vertices().to_vec(),
                    source.edges().to_vec(),
                    vec![
                        BrepFace::try_new(
                            face.surface().clone(),
                            face.is_reversed(),
                            vec![BrepLoop::try_new(BrepLoopType::Outer, restored).unwrap()],
                        )
                        .unwrap(),
                    ],
                    tolerance,
                )
                .unwrap();
                assert!(output.is_solid());
                assert_eq!(output.edges(), source.edges());
                assert_eq!(output.vertices(), source.vertices());
                assert_eq!(output.faces()[0].surface(), face.surface());
            }
        }
    }
}

#[test]
fn tiny_uv_gaps_cannot_authorize_noncollapsed_spatial_boundaries() {
    let tolerance = Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap();
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let surface = NurbsSurface::try_bilinear([p(0., 0.), p(2., 0.), p(2., 3.), p(0., 3.)])
        .unwrap()
        .try_reparameterized(0. ..=1e-4, 0. ..=1e-4)
        .unwrap();
    let vertex = BrepVertex::try_new(p(0., 0.), 0.).unwrap();
    assert!(
        pole_connector(
            &surface,
            vertex,
            Point2::try_new(0., 0.).unwrap(),
            Point2::try_new(1e-4, 0.).unwrap(),
            tolerance
        )
        .unwrap()
        .is_none()
    );
    assert!(
        pole_connector(
            &surface,
            vertex,
            Point2::try_new(0., 0.).unwrap(),
            Point2::try_new(1e-16, 0.).unwrap(),
            tolerance
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn pole_collapse_proof_keeps_model_units_and_inherited_vertex_bounds() {
    let surface = sphere()
        .try_reparameterized(0. ..=1e-4, 0. ..=1e-4)
        .unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let end = Point2::try_new(0., 0.).unwrap();
    let start = Point2::try_new(1e-4, 0.).unwrap();
    let point = surface.evaluate(0., 0.).unwrap();
    let point = Point3::try_new(point.x() + 1e-4, point.y(), point.z()).unwrap();
    let vertex = BrepVertex::try_new(point, 0.).unwrap();
    assert!(
        pole_connector(&surface, vertex, end, start, tolerance)
            .unwrap()
            .is_none()
    );
    let vertex = BrepVertex::try_new(point, 2e-4).unwrap();
    assert!(
        pole_connector(&surface, vertex, end, start, tolerance)
            .unwrap()
            .is_some()
    );
}

#[test]
fn fitted_pole_endpoints_allow_domain_scaled_roundoff_without_model_unit_padding() {
    let surface = sphere().try_reparameterized(0. ..=1., 0. ..=1.).unwrap();
    let vertex = BrepVertex::try_new(surface.evaluate(0., 1.).unwrap(), 0.).unwrap();
    for tolerance in [
        Tolerance::DEFAULT,
        Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap(),
    ] {
        assert!(
            pole_connector(
                &surface,
                vertex,
                Point2::try_new(0., 1.).unwrap(),
                Point2::try_new(1., 1. - 50. * f64::EPSILON).unwrap(),
                tolerance
            )
            .unwrap()
            .is_some()
        );
    }
}
