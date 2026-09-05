use super::*;

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn boundaries(weights: [Real; 4]) -> Vec<NurbsCurve> {
    let corners = [
        [0.0, 0.0, 0.0],
        [4.0, 0.0, 1.0],
        [4.0, 3.0, 2.0],
        [0.0, 3.0, -1.0],
    ];
    let mids = [
        [2.0, -1.0, 2.0],
        [5.0, 1.5, 3.0],
        [2.0, 4.0, -1.0],
        [-1.0, 1.5, 2.0],
    ];
    (0..4)
        .map(|i| {
            NurbsCurve::try_new_rational(
                2,
                [corners[i], mids[i], corners[(i + 1) % 4]]
                    .into_iter()
                    .zip([1.0, weights[i], 1.0])
                    .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap()
        })
        .collect()
}

fn hpoint(c: WeightedPoint3) -> [Real; 4] {
    let p = c.point();
    [
        p.x() * c.weight(),
        p.y() * c.weight(),
        p.z() * c.weight(),
        c.weight(),
    ]
}
fn mix(a: [Real; 4], b: [Real; 4], t: Real) -> [Real; 4] {
    std::array::from_fn(|j| (1.0 - t) * a[j] + t * b[j])
}
fn hcurve(c: &NurbsCurve, t: Real) -> [Real; 4] {
    let p = c.control_points();
    mix(
        mix(hpoint(p[0]), hpoint(p[1]), t),
        mix(hpoint(p[1]), hpoint(p[2]), t),
        t,
    )
}
fn direct_coons(curves: &[NurbsCurve], u: Real, v: Real) -> Point3 {
    let s = hcurve(&curves[3], 1.0 - u);
    let n = hcurve(&curves[1], u);
    let w = hcurve(&curves[0], v);
    let e = hcurve(&curves[2], 1.0 - v);
    let p: Vec<_> = curves
        .iter()
        .map(|c| hpoint(c.control_points()[0]))
        .collect();
    let a = mix(s, n, v);
    let b = mix(w, e, u);
    let corner = mix(mix(p[0], p[3], u), mix(p[1], p[2], u), v);
    let h: [Real; 4] = std::array::from_fn(|i| a[i] + b[i] - corner[i]);
    point([h[0] / h[3], h[1] / h[3], h[2] / h[3]])
}

#[test]
fn homogeneous_coons_matches_direct_blending_including_a_control_at_infinity() {
    for weights in [[1.0; 4], [0.3, 0.7, 2.0, 0.5], [0.5; 4], [0.2; 4], [0.1; 4]] {
        let source = boundaries(weights);
        let brep = Brep::try_edge_surface(&source, Tolerance::DEFAULT).unwrap();
        let s = brep.faces()[0].surface();
        assert_eq!((brep.vertices().len(), brep.edges().len()), (4, 4));
        assert_eq!(s.degree_u(), if weights == [0.5; 4] { 3 } else { 2 });
        for i in 0..=24 {
            for j in 0..=26 {
                let (u, v) = (i as Real / 24.0, j as Real / 26.0);
                assert!(
                    s.evaluate(u, v)
                        .unwrap()
                        .distance_to(direct_coons(&source, u, v))
                        .unwrap()
                        < 2e-12,
                    "{weights:?}, {u}, {v}"
                );
            }
        }
        let mesh = brep
            .polygon_mesh(0.0, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(mesh.topology().is_manifold());
        assert!(!mesh.topology().is_closed());
    }
}

#[test]
fn triangle_keeps_the_first_boundary_opposite_an_exact_singular_side() {
    let mut source = boundaries([0.3, 0.7, 2.0, 0.5]);
    source.truncate(3);
    source[2] = NurbsCurve::try_new_rational(
        2,
        [
            ([4.0, 3.0, 2.0], 1.0),
            ([1.0, 1.0, 3.0], 0.4),
            ([0.0, 0.0, 0.0], 1.0),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
        .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let b = Brep::try_edge_surface(&source, Tolerance::DEFAULT).unwrap();
    assert_eq!((b.vertices().len(), b.edges().len()), (3, 3));
    let s = b.faces()[0].surface();
    for i in 0..=64 {
        let t = i as Real / 64.0;
        assert_eq!(s.evaluate(t, 0.0).unwrap(), point([4.0, 3.0, 2.0]));
        assert!(
            s.evaluate(t, 1.0)
                .unwrap()
                .distance_to(source[0].evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn two_curve_rulings_preserve_the_input_homogeneous_scales() {
    let source = boundaries([0.3, 0.7, 2.0, 0.5]);
    let a = NurbsCurve::try_new_rational(
        2,
        source[0]
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * 7.0).unwrap())
            .collect(),
        source[0].knots().to_vec(),
    )
    .unwrap();
    let s =
        NurbsSurface::try_edge_curves(&[a.clone(), source[2].clone()], Tolerance::DEFAULT).unwrap();
    for j in 0..3 {
        assert_eq!(s.control_points()[j * 2], a.control_points()[j]);
    }
    for i in 0..=32 {
        let t = i as Real / 32.0;
        assert!(
            s.evaluate(0.0, t)
                .unwrap()
                .distance_to(a.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
        assert!(
            s.evaluate(1.0, t)
                .unwrap()
                .distance_to(source[2].evaluate(1.0 - t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn invalid_input_counts_closed_boundaries_and_resource_limits_are_rejected() {
    let source = boundaries([1.0; 4]);
    for curves in [vec![], vec![source[0].clone()], vec![source[0].clone(); 5]] {
        assert!(matches!(
            Brep::try_edge_surface(&curves, Tolerance::DEFAULT),
            Err(GeometryError::InvalidEdgeSurfaceBoundaries)
        ));
    }
    let closed = NurbsCurve::try_new(
        1,
        vec![
            point([0.0, 0.0, 0.0]),
            point([1.0, 0.0, 0.0]),
            point([1.0, 1.0, 0.0]),
            point([0.0, 1.0, 0.0]),
            point([0.0, 0.0, 0.0]),
        ],
        vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        Brep::try_edge_surface(&[closed, source[0].clone()], Tolerance::DEFAULT),
        Err(GeometryError::InvalidEdgeSurfaceBoundaries)
    ));
    let broken = NurbsCurve::try_new(
        1,
        vec![
            point([0.0, 0.0, 0.0]),
            point([1.0, 0.0, 0.0]),
            point([2.0, 0.0, 0.0]),
            point([3.0, 0.0, 0.0]),
        ],
        vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        Brep::try_edge_surface(&[broken, source[0].clone()], Tolerance::DEFAULT),
        Err(GeometryError::InvalidEdgeSurfaceBoundaries)
    ));
    let huge = NurbsCurve::try_clamped_uniform(
        1,
        (0..=MAX_EDGE_CONTROLS)
            .map(|i| point([i as Real, 0.0, 0.0]))
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        Brep::try_edge_surface(&[huge, source[0].clone()], Tolerance::DEFAULT),
        Err(GeometryError::EdgeSurfaceResourceLimit { .. })
    ));
    // Each input is within budget, but their disjoint knot union is not.
    let a = NurbsCurve::try_clamped_uniform(
        1,
        (0..300).map(|i| point([i as Real, 0.0, 0.0])).collect(),
    )
    .unwrap();
    let mut knots = a.knots().to_vec();
    let offset = (knots[2] - knots[1]) * 0.25;
    for knot in &mut knots[2..300] {
        *knot += offset;
    }
    let b = NurbsCurve::try_new_rational(1, a.control_points().to_vec(), knots).unwrap();
    assert!(matches!(
        basis::compatible(&a, &b),
        Err(GeometryError::EdgeSurfaceResourceLimit { .. })
    ));
}

#[test]
fn pair_matching_does_not_collapse_nearby_knots_within_one_boundary() {
    let a = NurbsCurve::try_new(
        2,
        [
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, -2.0, 1.0],
            [3.0, 2.0, 0.0],
            [4.0, 0.0, 0.0],
        ]
        .into_iter()
        .map(point)
        .collect(),
        vec![0.0, 0.0, 0.0, 0.3, 0.3 + 1e-11, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let b = boundaries([1.0; 4])[0].clone();
    let [p, q] = basis::compatible(&a, &b).unwrap();
    assert_eq!(p.control_points().len(), 5);
    assert_eq!(q.control_points().len(), 5);
    assert_eq!(p.knots(), q.knots());
    assert!(p.knots()[3] < p.knots()[4]);
    for t in [0.0, 0.15, 0.3, 0.3 + 5e-12, 0.3 + 1e-11, 0.7, 1.0] {
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(p.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
        assert!(
            b.evaluate(t)
                .unwrap()
                .distance_to(q.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}
