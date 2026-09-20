use super::*;

#[test]
fn exact_uv_recovery_keeps_boundary_jets_containment_and_meshing_consistent() {
    let tolerance = Tolerance::DEFAULT;
    let tiny = 1e-200;
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let surface = NurbsSurface::try_bilinear([p(0., 0.), p(1., 0.), p(1., 1.), p(0., 1.)])
        .unwrap()
        .try_reparameterized(0.0..=tiny, 0.4..=1.4)
        .unwrap();
    let mut source = Brep::try_surface_face(surface, tolerance).unwrap();
    let trim = &mut source.faces[0].loops[0].trims[0];
    assert_eq!(trim.iso, SurfaceIso::South);
    trim.curve = NurbsCurve2::try_new_rational(
        1,
        trim.curve
            .control_points()
            .iter()
            .zip([1., tiny])
            .map(|(c, w)| WeightedPoint2::try_new(c.point(), w).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let edge_index = trim.edge.unwrap();
    let edge = &mut source.edges[edge_index];
    edge.curve = NurbsCurve::try_new_rational(
        1,
        edge.curve
            .control_points()
            .iter()
            .zip([1., tiny])
            .map(|(c, w)| WeightedPoint3::try_new(c.point(), w).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    source.validate(tolerance).unwrap();
    let original = source.clone();
    let face = &source.faces[0];
    let trim = &face.loops[0].trims[0];
    let (uv, derivative) = trim.curve.evaluate_with_derivative(1.).unwrap();
    assert_eq!(uv.to_array(), [tiny, 0.4]);
    assert_eq!(derivative, [1., 0.]);
    let (point, su, sv) = face
        .surface
        .evaluate_with_derivatives(uv.x(), uv.y())
        .unwrap();
    let image_derivative: [Real; 3] = std::array::from_fn(|i| {
        su.to_array()[i].mul_add(derivative[0], sv.to_array()[i] * derivative[1])
    });
    let expected = source.edges[edge_index]
        .curve
        .evaluate_with_derivative(1.)
        .unwrap();
    assert_eq!(point, expected.0);
    assert!((image_derivative[0] / expected.1.x() - 1.).abs() < 2e-15);
    assert_eq!(&image_derivative[1..], &[0., 0.]);
    assert!(
        face.contains_parameters(0.5 * tiny, 0.7, tolerance)
            .unwrap()
    );
    assert!(
        face.contains_parameters(0.5 * tiny, 0.4, tolerance)
            .unwrap()
    );
    assert!(
        !face
            .contains_parameters(0.5 * tiny, 0.3, tolerance)
            .unwrap()
    );
    assert!(!face.contains_parameters(2. * tiny, 0.7, tolerance).unwrap());
    for mesh in [
        source.tessellate(2, tolerance).unwrap(),
        source.polygon_mesh(0., false, false, tolerance).unwrap(),
    ] {
        assert!((mesh.area().unwrap() - 1.).abs() < 2e-12);
        assert!(mesh.topology().is_oriented());
        let boundary = mesh
            .filtered_edge_polylines(crate::MeshEdgeFilter::Naked, tolerance)
            .unwrap();
        assert_eq!(boundary.len(), 1);
        assert!(boundary[0].is_closed());
        let perimeter: Real = mesh
            .filtered_edge_lines(crate::MeshEdgeFilter::Naked, tolerance)
            .unwrap()
            .iter()
            .map(|line| line.length().unwrap())
            .sum();
        assert!((perimeter - 4.).abs() < 2e-12);
    }
    assert_eq!(source, original);
}

#[test]
fn anisotropic_uv_domains_preserve_holes_and_constrained_mesh_boundaries() {
    for domains in [
        [[0., 1e-200], [0.4, 1.4]],
        [[0.4, 1.4], [0., 1e-200]],
        [[1e6, 1e6 + 1.], [-2e6, -2e6 + 1.]],
        [[2e200, 3e200], [2e-200, 3e-200]],
    ] {
        check_holed_uv_domain(domains);
    }
}

fn check_holed_uv_domain([u_domain, v_domain]: [[Real; 2]; 2]) {
    let tolerance = Tolerance::DEFAULT;
    let parameter =
        |[start, end]: [Real; 2], fraction: Real| (end - start).mul_add(fraction, start);
    let ring = |points: &[[Real; 2]]| {
        let points = points
            .iter()
            .chain(points.first())
            .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
            .collect();
        crate::Polyline3::try_new(points, tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let outer = ring(&[[0., 0.], [2., 0.], [2., 1.], [0., 1.]]);
    let hole = ring(&[[0.5, 0.25], [1.5, 0.25], [1.5, 0.75], [0.5, 0.75]]);
    let mut source = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
    let original_surface = source.faces[0].surface.clone();
    let face = &mut source.faces[0];
    face.surface = original_surface
        .try_reparameterized(u_domain[0]..=u_domain[1], v_domain[0]..=v_domain[1])
        .unwrap();
    for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
        trim.curve = NurbsCurve2::try_new_rational(
            trim.curve.degree(),
            trim.curve
                .control_points()
                .iter()
                .map(|c| {
                    let [u, v] = original_surface
                        .normalized_parameters(c.point().x(), c.point().y())
                        .unwrap();
                    WeightedPoint2::try_new(
                        Point2::try_new(parameter(u_domain, u), parameter(v_domain, v)).unwrap(),
                        c.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            trim.curve.knots().to_vec(),
        )
        .unwrap();
    }
    source
        .validate(tolerance)
        .unwrap_or_else(|error| panic!("domains {u_domain:?}, {v_domain:?}: {error:?}"));
    let original = source.clone();
    let face = &source.faces[0];
    assert!(
        face.contains_parameters(
            parameter(u_domain, 0.1),
            parameter(v_domain, 0.1),
            tolerance
        )
        .unwrap()
    );
    assert!(
        !face
            .contains_parameters(
                parameter(u_domain, 0.5),
                parameter(v_domain, 0.5),
                tolerance
            )
            .unwrap()
    );
    for mesh in [
        source.tessellate(2, tolerance).unwrap(),
        source.polygon_mesh(0., false, false, tolerance).unwrap(),
    ] {
        assert!((mesh.area().unwrap() - 1.5).abs() < 2e-12);
        assert!(mesh.topology().is_oriented());
        let boundaries = mesh
            .filtered_edge_polylines(crate::MeshEdgeFilter::Naked, tolerance)
            .unwrap();
        assert_eq!(boundaries.len(), 2);
        assert!(boundaries.iter().all(|p| p.is_closed()));
        let perimeter: Real = mesh
            .filtered_edge_lines(crate::MeshEdgeFilter::Naked, tolerance)
            .unwrap()
            .iter()
            .map(|line| line.length().unwrap())
            .sum();
        assert!((perimeter - 9.).abs() < 2e-12);
    }
    assert_eq!(source, original);
}
