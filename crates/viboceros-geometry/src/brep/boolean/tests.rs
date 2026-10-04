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
