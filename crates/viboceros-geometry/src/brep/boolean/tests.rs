use super::*;
mod native;
fn frame() -> Frame3 {
    Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn cube(intervals: [[Real; 2]; 3]) -> Brep {
    Brep::try_box(frame(), intervals, Tolerance::DEFAULT).unwrap()
}

fn measure(result: Option<Brep>, expected_volume: Real, expected_area: Option<Real>) {
    if expected_volume == 0. {
        assert!(result.is_none());
        return;
    }
    let brep = result.expect("nonempty solid");
    assert!(brep.is_solid());
    assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - expected_volume).abs() < 1e-10);
    if let Some(expected) = expected_area {
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-10);
    }
}

#[test]
fn overlaps_holes_disjoint_nested_equal_and_face_contacts() {
    let a = cube([[0., 2.]; 3]);
    let cases = [
        ("corner", [[1., 3.]; 3], [15., 1., 7.], [42., 6., 24.]),
        (
            "piercing column",
            [[0.5, 1.5], [0.5, 1.5], [-1., 3.]],
            [10., 2., 6.],
            [32., 10., 30.],
        ),
        ("disjoint", [[4., 5.]; 3], [9., 0., 8.], [30., 0., 24.]),
        ("contained", [[0.5, 1.5]; 3], [8., 1., 7.], [24., 6., 30.]),
        ("equal", [[0., 2.]; 3], [8., 8., 0.], [24., 24., 0.]),
        (
            "face contact",
            [[2., 4.], [0., 2.], [0., 2.]],
            [16., 0., 8.],
            [40., 0., 24.],
        ),
        (
            "contained on boundary",
            [[0., 1.]; 3],
            [8., 1., 7.],
            [24., 6., 24.],
        ),
    ];
    for (label, intervals, volumes, areas) in cases {
        let b = cube(intervals);
        for (i, operation) in [
            BrepBooleanOperation::Union,
            BrepBooleanOperation::Intersection,
            BrepBooleanOperation::Difference,
        ]
        .into_iter()
        .enumerate()
        {
            let result = a
                .try_boolean_convex(&b, operation, Tolerance::DEFAULT)
                .unwrap_or_else(|e| panic!("{label} {operation:?}: {e}"));
            measure(result, volumes[i], Some(areas[i]));
        }
    }
}

#[test]
fn inward_operands_are_normalized_and_inputs_unchanged() {
    let a = cube([[0., 2.]; 3]);
    let b = cube([[1., 3.]; 3]);
    for a in [a.clone(), a.reversed()] {
        for b in [b.clone(), b.reversed()] {
            let before = (a.clone(), b.clone());
            for (operation, volume) in [
                (BrepBooleanOperation::Union, 15.),
                (BrepBooleanOperation::Intersection, 1.),
                (BrepBooleanOperation::Difference, 7.),
            ] {
                measure(
                    a.try_boolean_convex(&b, operation, Tolerance::DEFAULT)
                        .unwrap(),
                    volume,
                    None,
                );
            }
            assert_eq!((&a, &b), (&before.0, &before.1));
        }
    }
}

#[test]
fn axis_box_grid_matches_independent_interval_measures() {
    let a = cube([[0., 2.]; 3]);
    for x in [-2., -0.5, 0., 0.5, 1., 1.5, 2.5] {
        for y in [-1., 0., 0.5, 1., 2.5] {
            let bounds = [[x, x + 1.], [y, y + 1.], [0.5, 1.5]];
            let b = cube(bounds);
            let overlap: Real = bounds
                .into_iter()
                .map(|[low, high]| (high.min(2.) - low.max(0.)).max(0.))
                .product();
            for (operation, volume) in [
                (BrepBooleanOperation::Union, 9. - overlap),
                (BrepBooleanOperation::Intersection, overlap),
                (BrepBooleanOperation::Difference, 8. - overlap),
            ] {
                measure(
                    a.try_boolean_convex(&b, operation, Tolerance::DEFAULT)
                        .unwrap(),
                    volume,
                    None,
                );
            }
        }
    }
}

#[test]
fn singular_edge_and_point_contacts_fail_explicitly() {
    let a = cube([[0., 2.]; 3]);
    for b in [cube([[2., 4.], [2., 4.], [0., 2.]]), cube([[2., 4.]; 3])] {
        assert!(matches!(
            a.try_boolean_convex(&b, BrepBooleanOperation::Union, Tolerance::DEFAULT),
            Err(GeometryError::UnrepresentableBrepBoolean)
        ));
        measure(
            a.try_boolean_convex(&b, BrepBooleanOperation::Intersection, Tolerance::DEFAULT)
                .unwrap(),
            0.,
            None,
        );
        measure(
            a.try_boolean_convex(&b, BrepBooleanOperation::Difference, Tolerance::DEFAULT)
                .unwrap(),
            8.,
            Some(24.),
        );
    }
}

#[test]
fn unsupported_inputs_are_rejected_without_changes() {
    let a = cube([[0., 2.]; 3]);
    let cylinder = Brep::try_cylinder(frame(), 1., 0., 2., Tolerance::DEFAULT).unwrap();
    let open = Brep::try_surface_face(a.faces[0].surface.clone(), Tolerance::DEFAULT).unwrap();
    let disjoint =
        Brep::try_disjoint_union(vec![a.clone(), cube([[4., 5.]; 3])], Tolerance::DEFAULT).unwrap();
    let nonconvex = a
        .try_boolean_convex(
            &cube([[1., 3.]; 3]),
            BrepBooleanOperation::Union,
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
    for b in [cylinder, open, disjoint, nonconvex] {
        let before = b.clone();
        assert!(matches!(
            a.try_boolean_convex(&b, BrepBooleanOperation::Union, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedConvexBrepBoolean { .. })
        ));
        assert_eq!(b, before);
    }
}

#[test]
fn exact_work_and_rational_size_limits_are_errors() {
    let a = cube([[0., 2.]; 3]);
    assert!(matches!(
        extract(&a, &mut Budget(0)),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
    let huge = Rational::from_integer(num_bigint::BigInt::from(1) << MAX_RATIONAL_BITS);
    assert_eq!(
        check_point(&[huge, Rational::zero(), Rational::zero()]),
        Err(GeometryError::BrepBooleanWorkLimit)
    );
}

fn tetra(offset: [Real; 3]) -> Brep {
    let points = [[0., 0., 0.], [3., 0., 0.], [0., 3., 0.], [0., 0., 3.]]
        .map(|p| Point3::try_from(std::array::from_fn(|i| p[i] + offset[i])).unwrap());
    let vertices = points
        .into_iter()
        .map(|p| BrepVertex::try_new(p, 0.).unwrap())
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    let mut map = BTreeMap::new();
    let mut faces = Vec::new();
    for indices in [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] {
        let [a, b, c] = indices.map(|i| points[i]);
        let fourth = Point3::try_from(std::array::from_fn(|i| {
            b.to_array()[i] + c.to_array()[i] - a.to_array()[i]
        }))
        .unwrap();
        let surface = NurbsSurface::try_bilinear([a, b, fourth, c]).unwrap();
        let uv = [[0., 0.], [1., 0.], [0., 1.]].map(|p| Point2::try_new(p[0], p[1]).unwrap());
        let mut trims = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let pair = [indices[i], indices[j]];
            let key = [pair[0].min(pair[1]), pair[0].max(pair[1])];
            let edge = *map.entry(key).or_insert_with(|| {
                let id = edges.len();
                edges.push(
                    BrepEdge::try_new(
                        key,
                        NurbsCurve::try_new(
                            1,
                            key.map(|i| points[i]).to_vec(),
                            vec![0., 0., 1., 1.],
                        )
                        .unwrap(),
                        0.,
                    )
                    .unwrap(),
                );
                id
            });
            trims.push(
                BrepTrim::try_new(
                    pair,
                    Some(edge),
                    pair != key,
                    NurbsCurve2::try_line(uv[i], uv[j]).unwrap(),
                    BrepTrimType::Mated,
                    SurfaceIso::NotIso,
                    [0., 0.],
                )
                .unwrap(),
            );
        }
        faces.push(BrepFace::try_from_polygon_boundaries(surface, false, vec![trims]).unwrap());
    }
    Brep::try_new(vertices, edges, faces, Tolerance::DEFAULT).unwrap()
}

#[test]
fn general_convex_faces_rational_intersections_and_affine_covariance() {
    let a = tetra([0.; 3]);
    let b = tetra([0.5; 3]);
    for linear in [
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        [[1., 1., 0.], [0., 1., 0.5], [0., 0., 1.]],
        [[-1., 1., 0.], [0., 1., 0.5], [0., 0., 1.]],
    ] {
        let transform =
            AffineTransform3::try_new(linear, Vector3::try_new(10., -4., 3.).unwrap()).unwrap();
        let left = a.transformed(transform, Tolerance::DEFAULT).unwrap();
        let right = b.transformed(transform, Tolerance::DEFAULT).unwrap();
        for (op, volume) in [
            (BrepBooleanOperation::Union, 8.4375),
            (BrepBooleanOperation::Intersection, 0.5625),
            (BrepBooleanOperation::Difference, 3.9375),
        ] {
            measure(
                left.try_boolean_convex(&right, op, Tolerance::DEFAULT)
                    .unwrap(),
                volume,
                None,
            );
        }
    }
    // A rational cut point need not be representable exactly in binary64.
    let stretch = AffineTransform3::try_new(
        [[3., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let left = a.transformed(stretch, Tolerance::DEFAULT).unwrap();
    let cut = cube([[0.5, 10.], [0., 4.], [0., 4.]]);
    let result = left
        .try_boolean_convex(&cut, BrepBooleanOperation::Intersection, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!(
        result
            .vertices
            .iter()
            .any(|v| (v.point.y() - 17. / 6.).abs() < 1e-14)
    );
    measure(Some(result), 4913. / 432., None);
}

fn merged_component(component: BrepUnionComponent, merge_all: bool) -> Brep {
    let mut labels = BTreeMap::new();
    let groups = component
        .face_sources
        .iter()
        .map(|source| {
            let next = labels.len();
            if merge_all {
                0
            } else {
                *labels.entry(*source).or_insert(next)
            }
        })
        .collect::<Vec<_>>();
    let brep = component
        .brep
        .try_merge_coplanar_polygon_faces_in_groups(&groups, Tolerance::DEFAULT)
        .unwrap()
        .unwrap_or(component.brep);
    brep.try_merge_all_edges(0., Tolerance::DEFAULT).unwrap()
}

#[test]
fn multiple_original_operands_retain_nonconvex_results_and_source_faces() {
    let cases = [
        (
            vec![
                cube([[0., 2.]; 3]),
                cube([[1., 3.]; 3]),
                cube([[2., 4.]; 3]),
            ],
            22.,
            18,
            vec![[0, 1], [1, 2]],
        ),
        (
            vec![
                cube([[0., 2.]; 3]),
                cube([[1., 3.], [0., 2.], [0., 2.]]),
                cube([[2., 4.], [0., 2.], [0., 2.]]),
            ],
            16.,
            14,
            vec![[0, 1], [1, 2], [0, 2]],
        ),
    ];
    for (inputs, volume, face_count, mut pairs) in cases {
        let before = inputs.clone();
        let refs = inputs.iter().collect::<Vec<_>>();
        pairs.sort();
        assert_eq!(convex_brep_boundary_interactions(&refs).unwrap(), pairs);
        let mut components = union_convex_breps(&refs, Tolerance::DEFAULT).unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].source_indices, [0, 1, 2]);
        assert_eq!(
            components[0].face_sources.len(),
            components[0].brep.faces.len()
        );
        let merged = merged_component(components.remove(0), false);
        assert_eq!(merged.faces.len(), face_count);
        measure(Some(merged), volume, None);
        assert_eq!(inputs, before);
    }
}

#[test]
fn interaction_evidence_distinguishes_crossings_contacts_nesting_and_equality() {
    let a = cube([[0., 2.]; 3]);
    for (b, expected) in [
        (cube([[1., 3.]; 3]), true),
        (cube([[2., 4.], [0., 2.], [0., 2.]]), true),
        (cube([[0., 1.]; 3]), true),
        (cube([[4., 5.]; 3]), false),
        (cube([[0.5, 1.5]; 3]), false),
        (a.clone(), false),
        (cube([[2., 4.], [2., 4.], [0., 2.]]), false),
    ] {
        assert_eq!(
            !convex_brep_boundary_interactions(&[&a, &b])
                .unwrap()
                .is_empty(),
            expected
        );
    }
    let inputs = [a, cube([[1., 3.]; 3]), cube([[10., 11.]; 3])];
    let refs = inputs.iter().collect::<Vec<_>>();
    assert_eq!(convex_brep_boundary_interactions(&refs).unwrap(), [[0, 1]]);
    let components = union_convex_breps(&refs, Tolerance::DEFAULT).unwrap();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].source_indices, [0, 1]);
    assert_eq!(components[1].source_indices, [2]);
}

#[test]
fn coplanar_regions_merge_by_original_face_or_across_operands_and_keep_holes() {
    for (bounds, no_merge_count, merged_count) in [
        ([[1., 3.]; 3], 12, 12),
        ([[1., 3.], [0., 2.], [0.5, 1.5]], 12, 10),
        ([[2., 4.], [0., 2.], [0., 2.]], 10, 6),
    ] {
        let a = cube([[0., 2.]; 3]);
        let b = cube(bounds);
        let component = union_convex_breps(&[&a, &b], Tolerance::DEFAULT)
            .unwrap()
            .remove(0);
        let no_merge = merged_component(component.clone(), false);
        let merged = merged_component(component, true);
        assert_eq!(no_merge.faces.len(), no_merge_count);
        assert_eq!(merged.faces.len(), merged_count);
        assert!(
            (no_merge.signed_volume(Tolerance::DEFAULT).unwrap()
                - merged.signed_volume(Tolerance::DEFAULT).unwrap())
            .abs()
                < 1e-10
        );
    }
    let a = cube([[0., 2.]; 3]);
    let b = cube([[0.5, 1.5], [0.5, 1.5], [-1., 3.]]);
    let difference = a
        .try_boolean_convex(&b, BrepBooleanOperation::Difference, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    let labels = vec![0; difference.faces.len()];
    let result = difference
        .try_merge_coplanar_polygon_faces_in_groups(&labels, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(result.faces.len(), 10);
    assert_eq!(
        result.faces.iter().filter(|f| f.loops.len() == 2).count(),
        2
    );
    measure(Some(result), 6., Some(30.));
}

fn walls(low: Real, high: Real, inner_low: Real, inner_high: Real) -> Vec<Brep> {
    let full = [low, high];
    let middle = [inner_low, inner_high];
    vec![
        cube([full, full, [low, inner_low]]),
        cube([full, full, [inner_high, high]]),
        cube([[low, inner_low], full, middle]),
        cube([[inner_high, high], full, middle]),
        cube([middle, [low, inner_low], middle]),
        cube([middle, [inner_high, high], middle]),
    ]
}

#[test]
fn multiple_body_union_keeps_cavities_and_nested_islands_with_their_owners() {
    let mut inputs = walls(0., 4., 1., 3.);
    inputs.extend(walls(1.5, 2.5, 1.75, 2.25));
    let before = inputs.clone();
    let refs = inputs.iter().collect::<Vec<_>>();
    let components = union_convex_breps(&refs, Tolerance::DEFAULT).unwrap();
    assert_eq!(components.len(), 2);
    assert_eq!(components[0].source_indices, (0..6).collect::<Vec<_>>());
    assert_eq!(components[1].source_indices, (6..12).collect::<Vec<_>>());
    for (component, volume, area) in components
        .into_iter()
        .zip([56., 0.875])
        .zip([120., 7.5])
        .map(|((c, v), a)| (c, v, a))
    {
        assert_eq!(component.brep.edge_connected_face_components().len(), 2);
        let merged = merged_component(component, true);
        assert_eq!(merged.faces.len(), 12);
        measure(Some(merged), volume, Some(area));
    }
    assert_eq!(inputs, before);
}

#[test]
fn boundary_sources_include_owned_coplanar_patches_but_exclude_consumed_interior_bodies() {
    let outer = cube([[0., 2.]; 3]);
    let touching = cube([[0., 1.]; 3]);
    let report = union_convex_breps(&[&outer, &touching], Tolerance::DEFAULT)
        .unwrap()
        .remove(0);
    assert_eq!(report.source_indices, [0, 1]);
    assert_eq!(report.boundary_source_indices, [0, 1]);
    assert!(report.face_sources.iter().all(|source| source[0] == 0));
    let overlapping = cube([[1., 3.]; 3]);
    let inside = cube([[0.5, 1.5]; 3]);
    let report = union_convex_breps(&[&outer, &overlapping, &inside], Tolerance::DEFAULT)
        .unwrap()
        .remove(0);
    assert_eq!(report.source_indices, [0, 1, 2]);
    assert_eq!(report.boundary_source_indices, [0, 1]);
    assert!(report.face_sources.iter().all(|source| source[0] != 2));
}
