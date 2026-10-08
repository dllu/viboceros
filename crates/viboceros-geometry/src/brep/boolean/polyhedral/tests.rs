use super::super::tests::{cube, measure, tetra};
use super::*;

fn merged(brep: Brep) -> Brep {
    let groups = vec![0; brep.faces.len()];
    let result = brep
        .try_merge_coplanar_polygon_faces_in_groups(&groups, Tolerance::DEFAULT)
        .unwrap()
        .unwrap_or(brep);
    result.try_merge_all_edges(0., Tolerance::DEFAULT).unwrap()
}

fn source(case: &str) -> (Brep, Brep) {
    if case == "island" {
        return (
            Brep::try_disjoint_union(
                vec![
                    cube([[0., 4.]; 3]),
                    cube([[1., 3.]; 3]).reversed(),
                    cube([[1.5, 2.5]; 3]),
                ],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            cube([[2., 5.]; 3]),
        );
    }
    if case == "disjoint_shells" {
        return (
            Brep::try_disjoint_union(
                vec![cube([[0., 3.]; 3]), cube([[4., 5.]; 3])],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            cube([[2., 4.5]; 3]),
        );
    }
    let block = cube([[0., 3.]; 3]);
    let (operation, tool) = match case {
        "concave" | "coplanar_concave" => (
            BrepBooleanOperation::Union,
            cube([[2., 4.], [1., 2.], [0., 3.]]),
        ),
        "hole" | "two_holes" | "singular_two_holes" | "reversed_hole" => (
            BrepBooleanOperation::Difference,
            cube([[1., 2.], [1., 2.], [-1., 4.]]),
        ),
        "cavity" => (BrepBooleanOperation::Difference, cube([[1., 2.]; 3])),
        "rounded_uv" => (
            BrepBooleanOperation::Intersection,
            cube([[0.5, 2.5], [-1., 4.], [-1., 4.]]),
        ),
        _ => panic!("unknown recipe"),
    };
    let a = merged(
        block
            .try_boolean_convex(&tool, operation, Tolerance::DEFAULT)
            .unwrap()
            .unwrap(),
    );
    let b = if case == "coplanar_concave" {
        cube([[2., 4.], [1., 3.], [0., 3.]])
    } else if case == "two_holes" || case == "singular_two_holes" {
        let interval = if case == "two_holes" {
            [2.25, 3.25]
        } else {
            [2., 3.]
        };
        merged(
            cube([[1., 4.]; 3])
                .try_boolean_convex(
                    &cube([interval, interval, [0., 5.]]),
                    BrepBooleanOperation::Difference,
                    Tolerance::DEFAULT,
                )
                .unwrap()
                .unwrap(),
        )
    } else {
        cube([[1.5, 3.5], [1.5, 3.5], [1., 2.]])
    };
    (
        if case == "reversed_hole" {
            a.reversed()
        } else {
            a
        },
        b,
    )
}

#[test]
fn chained_concave_hole_and_cavity_results_preserve_material_and_original_faces() {
    for (case, volumes) in [
        ("concave", [31.5, 2.5, 27.5]),
        ("hole", [26., 2., 22.]),
        ("cavity", [28., 2., 24.]),
    ] {
        let (a, b) = source(case);
        if case == "hole" {
            assert!(a.faces.iter().any(|f| f.loops.len() > 1));
        }
        let before = (a.clone(), b.clone());
        for (i, operation) in [
            BrepBooleanOperation::Union,
            BrepBooleanOperation::Intersection,
            BrepBooleanOperation::Difference,
        ]
        .into_iter()
        .enumerate()
        {
            let result = boolean_polyhedral_breps(&a, &b, operation, Tolerance::DEFAULT)
                .unwrap_or_else(|e| panic!("{case} {operation:?}: {e}"));
            assert_eq!(result.len(), 1);
            let body = &result[0];
            assert_eq!(body.face_sources.len(), body.brep.faces.len());
            for (face, [owner, index]) in body.brep.faces.iter().zip(&body.face_sources) {
                assert_eq!(face.surface, [&a, &b][*owner].faces[*index].surface);
            }
            measure(Some(body.brep.clone()), volumes[i], None);
            assert_eq!((&a, &b), (&before.0, &before.1));
        }
    }
}

#[test]
fn polyhedral_path_matches_exact_convex_results_including_inward_and_contacts() {
    let a = cube([[0., 2.]; 3]);
    for b in [
        cube([[1., 3.]; 3]),
        cube([[0.5, 1.5]; 3]),
        cube([[4., 5.]; 3]),
        a.clone(),
        cube([[2., 4.], [0., 2.], [0., 2.]]),
    ] {
        for operation in [
            BrepBooleanOperation::Union,
            BrepBooleanOperation::Intersection,
            BrepBooleanOperation::Difference,
        ] {
            let expected = a
                .try_boolean_convex(&b, operation, Tolerance::DEFAULT)
                .unwrap();
            let actual = a
                .reversed()
                .try_boolean_polyhedral(&b.reversed(), operation, Tolerance::DEFAULT)
                .unwrap();
            measure(
                actual,
                expected
                    .as_ref()
                    .map_or(0., |g| g.signed_volume(Tolerance::DEFAULT).unwrap()),
                expected.map(|g| g.area(Tolerance::DEFAULT).unwrap()),
            );
        }
    }
    let a = tetra([0.; 3]);
    let b = tetra([0.5; 3]);
    for (op, volume) in [
        (BrepBooleanOperation::Union, 8.4375),
        (BrepBooleanOperation::Intersection, 0.5625),
        (BrepBooleanOperation::Difference, 3.9375),
    ] {
        measure(
            a.try_boolean_polyhedral(&b, op, Tolerance::DEFAULT)
                .unwrap(),
            volume,
            None,
        );
    }
}

#[test]
fn disjoint_shells_cavity_and_island_follow_odd_even_containment() {
    let outside = cube([[0., 4.]; 3]);
    let cavity = cube([[1., 3.]; 3]).reversed();
    let island = cube([[1.5, 2.5]; 3]);
    let a = Brep::try_disjoint_union(vec![outside, cavity, island], Tolerance::DEFAULT).unwrap();
    let b = cube([[2., 5.], [2., 5.], [2., 5.]]);
    for (operation, volume, components) in [
        (BrepBooleanOperation::Union, 76.875, 1),
        (BrepBooleanOperation::Intersection, 7.125, 2),
        (BrepBooleanOperation::Difference, 49.875, 2),
    ] {
        let result = boolean_polyhedral_breps(&a, &b, operation, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.len(), components, "{operation:?}");
        let total: Real = result
            .iter()
            .map(|r| r.brep.signed_volume(Tolerance::DEFAULT).unwrap())
            .sum();
        assert!((total - volume).abs() < 1e-10, "{operation:?}: {total}");
    }
}

#[test]
fn overlapping_and_touching_input_shells_curves_and_open_inputs_are_rejected() {
    let a = cube([[0., 2.]; 3]);
    for b in [
        Brep::try_disjoint_union(vec![a.clone(), cube([[1., 3.]; 3])], Tolerance::DEFAULT).unwrap(),
        Brep::try_disjoint_union(vec![a.clone(), cube([[2., 4.]; 3])], Tolerance::DEFAULT).unwrap(),
        Brep::try_disjoint_union(vec![a.clone(), a.clone()], Tolerance::DEFAULT).unwrap(),
        Brep::try_cylinder(super::super::tests::frame(), 1., 0., 2., Tolerance::DEFAULT).unwrap(),
        a.sub_brep(&[0], Tolerance::DEFAULT).unwrap(),
    ] {
        let before = b.clone();
        assert!(matches!(
            boolean_polyhedral_breps(&a, &b, BrepBooleanOperation::Union, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
        ));
        assert_eq!(b, before);
    }
    assert!(matches!(
        input::extract(&a, Tolerance::DEFAULT, &mut Budget(0)),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
}

#[test]
fn rounded_uv_is_accepted_without_relaxing_model_plane_or_discrepancy_bounds() {
    let block = cube([[0., 3.]; 3]);
    let first = merged(
        block
            .try_boolean_convex(
                &cube([[0.5, 2.5], [-1., 4.], [-1., 4.]]),
                BrepBooleanOperation::Intersection,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap(),
    );
    assert!(super::super::extract(&first, &mut Budget(EXACT_WORK_LIMIT)).is_err());
    let second = cube([[1.5, 3.5], [1.5, 3.5], [1., 2.]]);
    measure(
        first
            .try_boolean_polyhedral(
                &second,
                BrepBooleanOperation::Intersection,
                Tolerance::DEFAULT,
            )
            .unwrap(),
        1.5,
        None,
    );
    let mut off_plane = first.clone();
    let old = off_plane.vertices[0].point.to_array();
    off_plane.vertices[0].point =
        Point3::try_new(old[0] + 1e-12, old[1] + 1e-12, old[2] + 1e-12).unwrap();
    assert!(matches!(
        input::extract(
            &off_plane,
            Tolerance::DEFAULT,
            &mut Budget(EXACT_WORK_LIMIT)
        ),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
            context: "model vertices are not exactly coplanar"
        })
    ));
    let mut bad_uv = first.clone();
    let trim = &mut bad_uv.faces[0].loops[0].trims[0];
    let endpoints = [
        trim.curve.start_point().unwrap(),
        trim.curve.end_point().unwrap(),
    ];
    let shift = |p: Point2| Point2::try_new(p.x() + 1e-5, p.y() + 1e-5).unwrap();
    trim.curve = NurbsCurve2::try_line(shift(endpoints[0]), shift(endpoints[1])).unwrap();
    assert!(matches!(
        input::extract(&bad_uv, Tolerance::DEFAULT, &mut Budget(EXACT_WORK_LIMIT)),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
    ));
}

#[test]
fn exact_loop_certificate_rejects_crossings_touching_and_nested_holes() {
    let ring = |points: &[[Real; 2]]| {
        points
            .iter()
            .map(|p| point(Point3::try_new(p[0], p[1], 0.).unwrap()))
            .collect::<Vec<_>>()
    };
    let normal = point(Point3::try_new(0., 0., 1.).unwrap());
    let outer = ring(&[[0., 0.], [4., 0.], [4., 4.], [0., 4.]]);
    let hole = ring(&[[1., 1.], [1., 3.], [3., 3.], [3., 1.]]);
    for loops in [
        vec![ring(&[[0., 0.], [3., 3.], [0., 2.], [3., 0.]])],
        vec![
            outer.clone(),
            ring(&[[0., 1.], [0., 2.], [1., 2.], [1., 1.]]),
        ],
        vec![
            outer.clone(),
            hole.clone(),
            ring(&[[1.5, 1.5], [1.5, 2.5], [2.5, 2.5], [2.5, 1.5]]),
        ],
        vec![
            outer.clone(),
            hole.clone(),
            ring(&[[2., 2.], [2., 3.5], [3.5, 3.5], [3.5, 2.]]),
        ],
    ] {
        assert!(matches!(
            input::certify_loops(&loops, &normal, &mut Budget(EXACT_WORK_LIMIT)),
            Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
        ));
    }
    input::certify_loops(&[outer, hole], &normal, &mut Budget(EXACT_WORK_LIMIT)).unwrap();
}

mod native;

mod multiple;

#[test]
fn split_polyhedral_preserves_all_cutter_interfaces_and_original_faces() {
    let target = cube([[0., 4.]; 3]);
    let x = cube([[1., 3.], [-1., 5.], [-1., 5.]]);
    let y = cube([[-1., 5.], [1., 3.], [-1., 5.]]);
    let original = (target.clone(), x.clone(), y.clone());
    let outputs = split_polyhedral_brep(&target, &[&x, &y], Tolerance::DEFAULT).unwrap();
    assert_eq!(outputs.len(), 9);
    let mut counts = BTreeMap::new();
    for piece in outputs {
        *counts.entry(piece.cutter_membership.clone()).or_insert(0) += 1;
        let expected = if piece.cutter_membership.iter().all(|v| *v) {
            16.
        } else if piece.cutter_membership.iter().any(|v| *v) {
            8.
        } else {
            4.
        };
        measure(Some(piece.brep.clone()), expected, None);
        assert_eq!(piece.face_sources.len(), piece.brep.faces.len());
        for (face, [owner, index]) in piece.brep.faces.iter().zip(piece.face_sources) {
            assert_eq!(face.surface, [&target, &x, &y][owner].faces[index].surface);
        }
    }
    assert_eq!(
        counts,
        BTreeMap::from([
            (vec![false, false], 4),
            (vec![false, true], 2),
            (vec![true, false], 2),
            (vec![true, true], 1)
        ])
    );
    assert_eq!((target, x, y), original);
}

#[test]
fn split_polyhedral_preserves_nested_material_and_handles_duplicate_or_absent_cutters() {
    let target = cube([[0., 4.]; 3]);
    let cutter = cube([[1., 3.]; 3]);
    let outputs = split_polyhedral_brep(&target, &[&cutter], Tolerance::DEFAULT).unwrap();
    assert_eq!(outputs.len(), 2);
    for piece in outputs {
        let expected = if piece.cutter_membership[0] { 8. } else { 56. };
        measure(Some(piece.brep), expected, None);
    }
    let outputs = split_polyhedral_brep(&target, &[&cutter, &cutter], Tolerance::DEFAULT).unwrap();
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs
            .iter()
            .all(|p| p.cutter_membership[0] == p.cutter_membership[1])
    );
    let outputs = split_polyhedral_brep(&target, &[], Tolerance::DEFAULT).unwrap();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].cutter_membership.is_empty());
    measure(Some(outputs.into_iter().next().unwrap().brep), 64., None);
    assert!(matches!(
        split_polyhedral_brep(&target, &vec![&cutter; 128], Tolerance::DEFAULT),
        Err(GeometryError::BrepBooleanWorkLimit)
    ));
}

#[test]
fn split_polyhedral_handles_concave_faces_holes_cavities_and_disjoint_shells() {
    for case in ["concave", "hole", "cavity", "disjoint_shells"] {
        let (target, cutter) = source(case);
        let before = (target.clone(), cutter.clone());
        let expected = target.signed_volume(Tolerance::DEFAULT).unwrap().abs();
        let pieces = split_polyhedral_brep(&target, &[&cutter], Tolerance::DEFAULT).unwrap();
        assert!(pieces.len() >= 2, "{case}");
        let volume = pieces
            .iter()
            .map(|p| {
                assert_eq!(p.branch_component_counts.len(), 1);
                assert_eq!(p.cutter_membership.len(), 1);
                assert!(p.brep.is_solid());
                p.brep.signed_volume(Tolerance::DEFAULT).unwrap()
            })
            .sum::<f64>();
        assert!((volume - expected).abs() < 1e-9, "{case}");
        let inside = pieces
            .iter()
            .filter(|p| p.cutter_membership[0])
            .map(|p| p.brep.signed_volume(Tolerance::DEFAULT).unwrap())
            .sum::<f64>();
        let intersection = intersect_polyhedral_breps(&[&target, &cutter], Tolerance::DEFAULT)
            .unwrap()
            .into_iter()
            .map(|p| p.brep.signed_volume(Tolerance::DEFAULT).unwrap())
            .sum::<f64>();
        assert!((inside - intersection).abs() < 1e-9, "{case}");
        assert_eq!((target, cutter), before);
    }
}
