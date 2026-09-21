use super::*;

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn cube() -> Brep {
    Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap()
}
fn split(source: &Brep, edge: usize) -> Brep {
    let parameters =
        [0.125, 0.375, 0.875].map(|t| source.edges[edge].curve.parameter_at(t).unwrap());
    source
        .try_split_edges_at_parameters(&[(edge, parameters.to_vec())], Tolerance::DEFAULT)
        .unwrap()
}

#[test]
fn split_box_edges_coalesce_without_touching_surfaces_or_branch_vertices() {
    let source = cube();
    for e in 0..source.edges.len() {
        let split = split(&source, e);
        let before = split.clone();
        let result = split
            .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(split, before);
        assert_eq!(result.vertices, source.vertices);
        assert_eq!(result.edges.len(), source.edges.len());
        assert!(result.is_solid());
        assert_eq!(result.edge_use_counts(), vec![2; source.edges.len()]);
        for (a, b) in result.faces.iter().zip(&source.faces) {
            assert_eq!(a.surface, b.surface);
            assert_eq!(a.reversed, b.reversed);
            assert_eq!(a.loops[0].trims.len(), b.loops[0].trims.len());
        }
        assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-10);
        assert_eq!(
            result.try_merge_all_edges(1., Tolerance::DEFAULT).unwrap(),
            result
        );
    }
}

#[test]
fn seam_merges_update_both_ring_uses_and_closed_edges_keep_their_seam_vertex() {
    let source = Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap();
    for e in 0..source.edges.len() {
        let split = split(&source, e);
        let result = split
            .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(result.edges.len(), source.edges.len(), "edge {e}");
        assert_eq!(result.vertices, source.vertices, "edge {e}");
        assert!(result.is_solid());
        assert_eq!(
            result
                .faces
                .iter()
                .flat_map(|f| &f.loops)
                .map(|l| l.trims.len())
                .collect::<Vec<_>>(),
            source
                .faces
                .iter()
                .flat_map(|f| &f.loops)
                .map(|l| l.trims.len())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn reversed_spatial_edges_preserve_trim_orientation_and_curve_loci() {
    let source = cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap();
    for mask in 0..16 {
        let mut split = split(&source, 0);
        let indices = [
            0,
            source.edges.len(),
            source.edges.len() + 1,
            source.edges.len() + 2,
        ];
        for (i, e) in indices.into_iter().enumerate() {
            if mask & (1 << i) == 0 {
                continue;
            }
            split.edges[e].vertices.reverse();
            split.edges[e].curve = split.edges[e].curve.reversed().unwrap();
            for trim in split
                .faces
                .iter_mut()
                .flat_map(|f| &mut f.loops)
                .flat_map(|l| &mut l.trims)
                .filter(|t| t.edge == Some(e))
            {
                trim.reversed_3d = !trim.reversed_3d;
            }
        }
        split.validate(Tolerance::DEFAULT).unwrap();
        let result = split
            .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(result.edges.len(), 4, "mask {mask}");
        assert_eq!(result.vertices, source.vertices);
        assert!(
            (result.area(Tolerance::DEFAULT).unwrap() - source.area(Tolerance::DEFAULT).unwrap())
                .abs()
                < 1e-12
        );
    }
}

#[test]
fn exact_uv_geometry_is_required_and_displacement_accumulates_outward() {
    let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let a =
        NurbsCurve::try_new(1, vec![point(0., 0.), point(1., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let b = NurbsCurve::try_new(
        1,
        vec![point(1., 1e-5), point(2., 1e-5)],
        vec![20., 20., 21., 21.],
    )
    .unwrap();
    let mut budget = Budget(MAX_WORK);
    assert!(
        curves::append(&a, &b, 0., [0.; 2], &mut budget)
            .unwrap()
            .is_none()
    );
    let joined = curves::append(&a, &b, 1e-4, [0.; 2], &mut budget)
        .unwrap()
        .unwrap();
    assert!(joined.bounds.iter().all(|v| (0.5e-5..0.501e-5).contains(v)));
    assert!(
        curves::append(&a, &b, 1e-4, [1e-4; 2], &mut budget)
            .unwrap()
            .is_none()
    );
    assert_eq!(joined.curve.domain(), 0.0..=2.0);
}

#[test]
fn invalid_angles_and_exhausted_budgets_never_mutate_sources() {
    let source = split(&cube(), 0);
    let before = source.clone();
    for angle in [
        -1.,
        Real::NAN,
        Real::INFINITY,
        std::f64::consts::PI.next_up(),
    ] {
        assert!(
            source
                .try_merge_all_edges(angle, Tolerance::DEFAULT)
                .is_err()
        );
    }
    for work in [0, 16, 100, 1000] {
        assert!(merge(&source, 1e-10, Tolerance::DEFAULT, &mut Budget(work)).is_err());
    }
    assert_eq!(source, before);
}

#[test]
fn genuine_tiny_kinks_are_not_lost_when_cosine_rounds_to_one() {
    let angle: Real = 4e-9;
    assert_eq!(angle.cos(), 1.);
    let points = [
        [0., 0., 0.],
        [1., 0., 0.],
        [2., angle, 0.],
        [2., 2., 0.],
        [0., 2., 0.],
        [0., 0., 0.],
    ]
    .map(|p| Point3::try_from(p).unwrap())
    .to_vec();
    let curve = NurbsCurve::try_new(1, points, vec![0., 0., 1., 2., 3., 4., 5., 5.]).unwrap();
    let source = Brep::try_planar_face(&curve, Tolerance::DEFAULT)
        .unwrap()
        .try_split_edges_at_parameters(&[(0, vec![1.])], Tolerance::DEFAULT)
        .unwrap();
    let rejected = source
        .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(rejected, source);
    let accepted = source
        .try_merge_all_edges(1e-8, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(accepted.edges.len(), 1);
    assert_eq!(accepted.vertices.len(), 1);
    assert!((accepted.area(Tolerance::DEFAULT).unwrap() - (4. - angle * 0.5)).abs() < 1e-12);
}

#[test]
fn singular_trim_vertices_are_retained_even_if_surrounding_edges_are_smooth() {
    let mut source = split(&cube().sub_brep(&[0], Tolerance::DEFAULT).unwrap(), 0);
    let trims = &mut source.faces[0].loops[0].trims;
    // Loop order need not start at edge-table entry zero. Select an actual
    // inserted split vertex, not an existing nonsmooth rectangle corner.
    let slot = trims.iter().position(|t| t.vertices[1] >= 4).unwrap();
    let v = trims[slot].vertices[1];
    let uv = trims[slot].curve.end_point().unwrap();
    let singular = BrepTrim::try_new(
        [v, v],
        None,
        false,
        NurbsCurve2::try_new(1, vec![uv, uv], vec![0., 0., 1., 1.]).unwrap(),
        BrepTrimType::Singular,
        SurfaceIso::NotIso,
        [0.; 2],
    )
    .unwrap();
    trims.insert(slot + 1, singular);
    source.validate(Tolerance::DEFAULT).unwrap();
    let result = source
        .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(result.edges.len(), 5);
    let singular = result
        .trim_uses()
        .into_iter()
        .find(|u| u.trim.trim_type == BrepTrimType::Singular)
        .unwrap()
        .trim;
    assert_eq!(
        result.vertices[singular.vertices[0]].point,
        source.vertices[v].point
    );
}

#[test]
fn original_uncertainty_survives_exact_surface_recomputation() {
    let mut source = split(&cube(), 0);
    source.edges[0].tolerance = 0.1;
    let result = source
        .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(result.edges.last().unwrap().tolerance, 0.1);
}

#[test]
fn curve_certificates_cover_degree_domain_gauge_and_uv_changes() {
    let point = |x| Point3::try_new(x, 0., 0.).unwrap();
    let a = NurbsCurve::try_new_rational(
        1,
        [0., 1.]
            .map(|x| WeightedPoint3::try_new(point(x), 1e-300).unwrap())
            .to_vec(),
        vec![1e300, 1e300, 1e300_f64.next_up(), 1e300_f64.next_up()],
    )
    .unwrap();
    let b = NurbsCurve::try_new_rational(
        2,
        [1., 1.5, 2.]
            .map(|x| WeightedPoint3::try_new(point(x), -1e300).unwrap())
            .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let mut budget = Budget(MAX_WORK);
    let result = curves::append(&a, &b, 1e-9, [0.; 2], &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(result.curve.degree(), 2);
    assert_eq!(result.curve.domain(), 0.0..=2.0);
    for i in 0..=64 {
        let t = i as Real / 32.;
        assert!(
            result
                .curve
                .evaluate(t)
                .unwrap()
                .distance_to(point(t))
                .unwrap()
                < 1e-14
        );
    }
    let uv = |a, b| {
        NurbsCurve2::try_line(
            Point2::try_new(a, 0.).unwrap(),
            Point2::try_new(b, 0.).unwrap(),
        )
        .unwrap()
    };
    assert!(
        curves::append_uv(&uv(0., 1.), &uv(1. + 1e-15, 2.), &mut budget)
            .unwrap()
            .is_none()
    );
    let b =
        NurbsCurve::try_clamped_uniform(2, (0..1000).map(|i| point(i as Real)).collect()).unwrap();
    let a = NurbsCurve::try_new(
        3,
        [0., 0.25, 0.75, 1.].map(point).to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(curves::append(&a, &b, 1e-9, [0.; 2], &mut Budget(MAX_WORK)).is_err());
}
