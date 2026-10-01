use super::*;

#[test]
fn selected_hole_removal_preserves_outer_geometry_and_discards_unused_topology() {
    for reversed in [false, true] {
        let original = source(reversed);
        let saved = original.clone();
        let result = original
            .try_remove_holes(&[(0, 1), (0, 1)], Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(original, saved);
        assert_eq!(result.vertices, original.vertices[..1]);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].curve, original.edges[1].curve);
        assert_eq!(result.edges[0].tolerance, original.edges[1].tolerance);
        assert_eq!(result.faces[0].surface, original.faces[0].surface);
        assert_eq!(result.faces[0].reversed, reversed);
        let mut expected = original.faces[0].loops[0].clone();
        expected.trims[0].edge = Some(0);
        assert_eq!(result.faces[0].loops, [expected]);
        assert!((result.area(Tolerance::DEFAULT).unwrap() - 64.).abs() < 1e-10);
        assert!(
            result.faces[0]
                .contains_parameters(4., 4., Tolerance::DEFAULT)
                .unwrap()
        );
        assert_eq!(
            result.try_remove_all_holes(Tolerance::DEFAULT).unwrap(),
            Some(result.clone())
        );
        assert_eq!(
            result.try_remove_holes(&[], Tolerance::DEFAULT).unwrap(),
            None
        );
        assert_eq!(
            original
                .try_remove_holes(&[(0, 0)], Tolerance::DEFAULT)
                .unwrap(),
            None
        );
        for indices in [vec![(0, 1), (0, 2)], vec![(0, 1), (1, 0)]] {
            assert!(
                original
                    .try_remove_holes(&indices, Tolerance::DEFAULT)
                    .is_err()
            );
            assert_eq!(original, saved);
        }
    }
}

#[test]
fn one_joined_opening_removes_its_wall_and_both_cap_holes_exactly() {
    let frame = Frame3::try_from_directions(
        point(0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let tube = Brep::try_tube(frame, [2., 5.], 8., Tolerance::DEFAULT)
        .unwrap()
        .reordered_edges(&[5, 1, 3, 0, 4, 2], Tolerance::DEFAULT)
        .unwrap();
    let result = tube
        .try_remove_holes(&[(2, 1)], Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        tube.try_remove_holes(&[(3, 1)], Tolerance::DEFAULT)
            .unwrap(),
        Some(result.clone())
    );
    assert_eq!(
        tube.try_remove_all_holes(Tolerance::DEFAULT).unwrap(),
        Some(result.clone())
    );
    assert!(result.is_closed());
    assert!(result.is_solid());
    assert_eq!(result.vertices.len(), 2);
    assert_eq!(result.edges.len(), 3);
    assert_eq!(result.faces.len(), 3);
    for (remaining, index) in result.faces.iter().zip([0, 2, 3]) {
        assert_eq!(remaining.surface, tube.faces[index].surface);
        assert_eq!(remaining.reversed, tube.faces[index].reversed);
        assert_eq!(remaining.loops.len(), 1);
        for (a, b) in remaining.loops[0]
            .trims
            .iter()
            .zip(&tube.faces[index].loops[0].trims)
        {
            assert_eq!(a.curve, b.curve);
            assert_eq!(a.tolerance, b.tolerance);
            assert_eq!(a.trim_type, b.trim_type);
            assert_eq!(a.reversed_3d, b.reversed_3d);
            assert_eq!(a.iso, b.iso);
            assert_eq!(
                result.edges[a.edge.unwrap()].curve,
                tube.edges[b.edge.unwrap()].curve
            );
        }
    }
    let open = tube.sub_brep(&[0, 1, 2], Tolerance::DEFAULT).unwrap();
    let closed_hole = open
        .try_remove_holes(&[(2, 1)], Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(closed_hole.faces.len(), 2);
    assert!(!closed_hole.is_closed());
    assert!(closed_hole.faces.iter().all(|face| face.loops.len() == 1));
    let caps = tube.sub_brep(&[2, 3], Tolerance::DEFAULT).unwrap();
    let one_cap = caps
        .try_remove_holes(&[(0, 1)], Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(one_cap.faces[0].loops.len(), 1);
    assert_eq!(one_cap.faces[1].loops.len(), 2);
    assert_eq!(
        one_cap.faces[1].loops[1].trims[0].curve,
        caps.faces[1].loops[1].trims[0].curve
    );
}

#[test]
fn hole_traversal_stays_within_each_disconnected_component() {
    let original =
        Brep::try_combine(vec![source(false), source(true)], Tolerance::DEFAULT).unwrap();
    let selected = original
        .try_remove_holes(&[(0, 1)], Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(selected.faces.len(), 2);
    assert_eq!(selected.faces[0].loops.len(), 1);
    assert_eq!(selected.faces[1].loops.len(), 2);
    assert_eq!(selected.faces[1].surface, original.faces[1].surface);
    assert!(selected.faces[1].reversed);
    assert!((selected.area(Tolerance::DEFAULT).unwrap() - 124.).abs() < 1e-10);
    let all = original
        .try_remove_all_holes(Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!((all.area(Tolerance::DEFAULT).unwrap() - 128.).abs() < 1e-10);
    assert_eq!(all.edges.len(), 2);
    assert_eq!(all.vertices.len(), 2);
}

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}

fn surface() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        point(0., 0.),
        point(10., 0.),
        point(10., 10.),
        point(0., 10.),
    ])
    .unwrap()
    .try_reparameterized(0.0..=10., 0.0..=10.)
    .unwrap()
}

fn source(reversed: bool) -> Brep {
    let outer = crate::Polyline3::try_new(
        vec![
            point(1., 1.),
            point(9., 1.),
            point(9., 9.),
            point(1., 9.),
            point(1., 1.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_native_nurbs()
    .unwrap();
    let mut vertices = vec![BrepVertex::try_new(point(1., 1.), 1e-7).unwrap()];
    let mut edges = vec![BrepEdge::try_new([0, 0], outer, 1e-8).unwrap()];
    let outer = BrepLoop::try_new(
        BrepLoopType::Outer,
        vec![
            BrepTrim::try_new(
                [0, 0],
                Some(0),
                false,
                NurbsCurve2::try_new(
                    1,
                    vec![
                        Point2::try_new(1., 1.).unwrap(),
                        Point2::try_new(9., 1.).unwrap(),
                        Point2::try_new(9., 9.).unwrap(),
                        Point2::try_new(1., 9.).unwrap(),
                        Point2::try_new(1., 1.).unwrap(),
                    ],
                    vec![0., 0., 8., 16., 24., 32., 32.],
                )
                .unwrap(),
                BrepTrimType::Boundary,
                SurfaceIso::NotIso,
                [1e-8, 2e-8],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let corners = [point(3., 3.), point(3., 5.), point(5., 5.), point(5., 3.)];
    vertices.extend(corners.map(|p| BrepVertex::try_new(p, 3e-8).unwrap()));
    // This unreferenced vertex must not leak into the compacted result.
    vertices.push(BrepVertex::try_new(point(30., 30.), 0.).unwrap());
    let mut trims = Vec::new();
    for i in 0..4 {
        let a = i + 1;
        let b = (i + 1) % 4 + 1;
        let reverse = i % 2 == 0;
        let curve = NurbsCurve::try_new(
            1,
            vec![vertices[a].point, vertices[b].point],
            vec![-2., -2., 7., 7.],
        )
        .unwrap();
        edges.push(
            BrepEdge::try_new(
                if reverse { [b, a] } else { [a, b] },
                if reverse {
                    curve.reversed().unwrap()
                } else {
                    curve
                },
                4e-8,
            )
            .unwrap(),
        );
        trims.push(
            BrepTrim::try_new(
                [a, b],
                Some(i + 1),
                reverse,
                NurbsCurve2::try_new(
                    1,
                    vec![
                        Point2::try_new(corners[i].x(), corners[i].y()).unwrap(),
                        Point2::try_new(corners[(i + 1) % 4].x(), corners[(i + 1) % 4].y())
                            .unwrap(),
                    ],
                    vec![11., 11., 15., 15.],
                )
                .unwrap(),
                BrepTrimType::Boundary,
                SurfaceIso::NotIso,
                [5e-8, 6e-8],
            )
            .unwrap(),
        );
    }
    Brep::try_new(
        vertices,
        edges,
        vec![
            BrepFace::try_new(
                surface(),
                reversed,
                vec![
                    outer,
                    BrepLoop::try_new(BrepLoopType::Inner, trims).unwrap(),
                ],
            )
            .unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .reordered_edges(&[4, 0, 3, 1, 2], Tolerance::DEFAULT)
    .unwrap()
}

#[test]
fn outer_restoration_retains_exact_hole_geometry_and_compacts_permuted_topology() {
    for reversed in [false, true] {
        let original = source(reversed);
        let restored = original
            .try_untrim_outer_boundary(Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(restored.faces[0].surface, original.faces[0].surface);
        assert_eq!(restored.faces[0].reversed, reversed);
        assert_eq!(restored.vertices.len(), 8);
        assert_eq!(restored.edges.len(), 8);
        assert_eq!(restored.faces[0].loops.len(), 2);
        assert_eq!(&restored.vertices[..4], &original.vertices[1..5]);
        let mut used = vec![false; original.edges.len()];
        for trim in &original.faces[0].loops[1].trims {
            used[trim.edge.unwrap()] = true;
        }
        let retained = original
            .edges
            .iter()
            .zip(used)
            .filter(|(_, used)| *used)
            .map(|(edge, _)| edge);
        for (a, b) in restored.edges[..4].iter().zip(retained) {
            assert_eq!(a.curve, b.curve);
            assert_eq!(a.tolerance, b.tolerance);
            assert_eq!(a.vertices, b.vertices.map(|v| v - 1));
        }
        for (a, b) in restored.faces[0].loops[1]
            .trims
            .iter()
            .zip(&original.faces[0].loops[1].trims)
        {
            assert_eq!(a.curve, b.curve);
            assert_eq!(a.tolerance, b.tolerance);
            assert_eq!(a.iso, b.iso);
            assert_eq!(a.reversed_3d, b.reversed_3d);
            assert_eq!(
                restored.edges[a.edge.unwrap()].curve,
                original.edges[b.edge.unwrap()].curve
            );
        }
        assert!((restored.area(Tolerance::DEFAULT).unwrap() - 96.).abs() < 1e-10);
        assert!(
            restored.faces[0]
                .contains_parameters(0.5, 0.5, Tolerance::DEFAULT)
                .unwrap()
        );
        assert!(
            !original.faces[0]
                .contains_parameters(0.5, 0.5, Tolerance::DEFAULT)
                .unwrap()
        );
        assert!(
            !restored.faces[0]
                .contains_parameters(4., 4., Tolerance::DEFAULT)
                .unwrap()
        );
        assert_eq!(
            restored
                .try_untrim_outer_boundary(Tolerance::DEFAULT)
                .unwrap(),
            restored
        );
    }
}

#[test]
fn natural_singular_boundary_is_rebuilt_without_fitting_and_joined_faces_are_rejected() {
    let triangle =
        NurbsSurface::try_bilinear([point(0., 0.), point(6., 0.), point(0., 4.), point(0., 0.)])
            .unwrap();
    let natural = Brep::try_surface_face(triangle, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        natural
            .try_untrim_outer_boundary(Tolerance::DEFAULT)
            .unwrap(),
        natural
    );
    assert!(
        natural.faces[0].loops[0]
            .trims
            .iter()
            .any(|t| t.trim_type == BrepTrimType::Singular)
    );
    let combined =
        Brep::try_combine(vec![source(false), source(true)], Tolerance::DEFAULT).unwrap();
    assert!(combined.try_untrim_all(Tolerance::DEFAULT).is_err());
    assert!(
        combined
            .try_untrim_outer_boundary(Tolerance::DEFAULT)
            .is_err()
    );
}

#[test]
fn closed_surface_hole_survives_restoration_with_shared_seam_and_native_intervals() {
    let frame = Frame3::try_from_normal(
        point(0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame, 2., 0., 5.)
        .unwrap()
        .try_reparameterized(2.0..=6., -1.0..=4.)
        .unwrap();
    let mut source = Brep::try_surface_face(cylinder.clone(), Tolerance::DEFAULT).unwrap();
    let corners = [
        Point2::try_new(3., 0.).unwrap(),
        Point2::try_new(3., 3.).unwrap(),
        Point2::try_new(4., 3.).unwrap(),
        Point2::try_new(4., 0.).unwrap(),
    ];
    let offset = source.vertices.len();
    for p in corners {
        source
            .vertices
            .push(BrepVertex::try_new(cylinder.evaluate(p.x(), p.y()).unwrap(), 0.).unwrap());
    }
    let curves = [
        cylinder
            .isocurve_v(3.)
            .unwrap()
            .try_trimmed(0.0..=3.)
            .unwrap(),
        cylinder
            .isocurve_u(3.)
            .unwrap()
            .try_trimmed(3.0..=4.)
            .unwrap(),
        cylinder
            .isocurve_v(4.)
            .unwrap()
            .try_trimmed(0.0..=3.)
            .unwrap()
            .reversed()
            .unwrap(),
        cylinder
            .isocurve_u(0.)
            .unwrap()
            .try_trimmed(3.0..=4.)
            .unwrap()
            .reversed()
            .unwrap(),
    ];
    let mut trims = Vec::new();
    for (side, curve) in curves.into_iter().enumerate() {
        let ends = [offset + side, offset + (side + 1) % 4];
        let edge = source.edges.len();
        source
            .edges
            .push(BrepEdge::try_new(ends, curve, 0.).unwrap());
        trims.push(
            BrepTrim::try_new(
                ends,
                Some(edge),
                false,
                NurbsCurve2::try_new(
                    1,
                    vec![corners[side], corners[(side + 1) % 4]],
                    vec![0., 0., 1., 1.],
                )
                .unwrap(),
                BrepTrimType::Boundary,
                if side % 2 == 0 {
                    SurfaceIso::InteriorUConstant
                } else {
                    SurfaceIso::InteriorVConstant
                },
                [0., 0.],
            )
            .unwrap(),
        );
    }
    source.faces[0]
        .loops
        .push(BrepLoop::try_new(BrepLoopType::Inner, trims).unwrap());
    let source = Brep::try_new(
        source.vertices,
        source.edges,
        source.faces,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let output = source
        .try_untrim_outer_boundary(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(output.vertices.len(), 6);
    assert_eq!(output.edges.len(), 7);
    assert_eq!(output.faces[0].surface, cylinder);
    assert_eq!(
        output.faces[0].loops[0]
            .trims
            .iter()
            .filter(|t| t.trim_type == BrepTrimType::Seam)
            .count(),
        2
    );
    for (a, b) in output.faces[0].loops[1]
        .trims
        .iter()
        .zip(&source.faces[0].loops[1].trims)
    {
        assert_eq!(a.curve, b.curve);
        assert_eq!(
            output.edges[a.edge.unwrap()].curve,
            source.edges[b.edge.unwrap()].curve
        );
    }
    assert!(
        !output.faces[0]
            .contains_parameters(3.5, 1.5, Tolerance::DEFAULT)
            .unwrap()
    );
    assert!(
        output.faces[0]
            .contains_parameters(2.5, 1.5, Tolerance::DEFAULT)
            .unwrap()
    );
    assert_eq!(
        output
            .try_untrim_outer_boundary(Tolerance::DEFAULT)
            .unwrap(),
        output
    );
    let full = source.try_untrim_all(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        full,
        Brep::try_surface_face_with_native_edge_parameters(cylinder, Tolerance::DEFAULT).unwrap()
    );
    assert_eq!(full.faces[0].loops.len(), 1);
    assert!(
        full.faces[0]
            .contains_parameters(3.5, 1.5, Tolerance::DEFAULT)
            .unwrap()
    );
}
