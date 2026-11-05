use super::*;
#[test]
fn apex_kink_partitions_retain_exact_poles_and_native_edge_charts() {
    let tolerance = Tolerance::DEFAULT;
    let point = |p| Point3::try_from(p).unwrap();
    for [start, end] in [[0., 4.], [0., 1.], [100., 104.], [1e6, 1e6 + 1.]] {
        let points = [
            [3., 0., 0.],
            [4., 0., 1.],
            [3., 0., 2.],
            [2., 0., 1.],
            [3., 0., 0.],
        ];
        let mut knots = vec![start];
        knots.extend((0..=4).map(|i| start + (end - start) * i as f64 / 4.));
        knots.push(end);
        let curve = NurbsCurve::try_new(1, points.into_iter().map(point).collect(), knots).unwrap();
        let apex = point([1., -4., 3.]);
        let surface = NurbsSurface::try_extruded_curve_to_point(&curve, apex).unwrap();
        let source =
            Brep::try_surface_face_with_native_edge_parameters(surface, tolerance).unwrap();
        let result = source
            .try_split_kinky_faces(0.1_f64.to_radians(), tolerance)
            .unwrap()
            .unwrap();
        assert_eq!(result.faces().len(), 4);
        assert_eq!(
            result
                .vertices()
                .iter()
                .filter(|v| v.point() == apex)
                .count(),
            1
        );
        assert!(!result.is_solid());
        assert!(result.is_manifold());
    }
}
