use super::*;

fn surface() -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [0., 3.]
            .into_iter()
            .flat_map(|z| [[0., 0., z], [10., 0., z], [20., 1., z]])
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn knot_partition_preserves_source_geometry_and_existing_topology() {
    for pre_split in [false, true] {
        for reversed in [false, true] {
            let mut source = Brep::try_surface_face(surface(), Tolerance::DEFAULT).unwrap();
            if pre_split {
                source = source
                    .try_split_edges_at_parameters(&[(0, vec![1.])], Tolerance::DEFAULT)
                    .unwrap();
            }
            if reversed {
                source = source.reversed();
            }
            let before = source.clone();
            let result = source
                .try_split_face_at_knot(0, SurfaceKnotDirection::U, 1., Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            assert_eq!(source, before);
            assert_eq!(result.faces.len(), 2);
            assert_eq!(result.edges.len(), 7);
            assert_eq!(result.vertices.len(), 6);
            assert_eq!(&result.vertices[..source.vertices.len()], source.vertices);
            assert!(result.faces.iter().all(|f| f.reversed == reversed));
            assert_eq!(
                result.edge_use_counts().iter().filter(|&&n| n == 2).count(),
                1
            );
            assert!(
                (source.area(Tolerance::DEFAULT).unwrap()
                    - result.area(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-10
            );
            for face in &result.faces {
                for i in 0..=8 {
                    for j in 0..=8 {
                        let u = face.surface.parameter_at_u(i as Real / 8.).unwrap();
                        let v = face.surface.parameter_at_v(j as Real / 8.).unwrap();
                        assert_eq!(
                            face.surface.evaluate(u, v).unwrap(),
                            source.faces[0].surface.evaluate(u, v).unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn splitting_one_face_updates_its_unsplit_neighbor() {
    let source = Brep::try_surface_grid(&surface(), &[], &[0.5], Tolerance::DEFAULT).unwrap();
    let result = source
        .try_split_face_at_knot(0, SurfaceKnotDirection::U, 1., Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(result.faces.len(), 3);
    assert_eq!(result.faces[1].surface, source.faces[1].surface);
    assert_eq!(result.faces[1].loops[0].trims.len(), 5);
    assert!(result.edge_use_counts().iter().all(|&n| n == 1 || n == 2));
    assert_eq!(&result.vertices[..source.vertices.len()], source.vertices);
}

#[test]
fn invalid_partition_requests_and_exhausted_work_are_atomic() {
    let source = Brep::try_surface_face(surface(), Tolerance::DEFAULT).unwrap();
    let before = source.clone();
    for (face, direction, cut) in [
        (1, SurfaceKnotDirection::U, 1.),
        (0, SurfaceKnotDirection::Both, 1.),
        (0, SurfaceKnotDirection::U, 0.),
        (0, SurfaceKnotDirection::U, 2.),
        (0, SurfaceKnotDirection::U, 0.5),
        (0, SurfaceKnotDirection::U, Real::NAN),
    ] {
        assert!(
            source
                .try_split_face_at_knot(face, direction, cut, Tolerance::DEFAULT)
                .is_err()
        );
    }
    assert!(surface::split(&source.faces[0].surface, 0, 1., &mut Budget(0)).is_err());
    assert_eq!(source, before);
}

#[test]
fn v_partition_and_closed_seams_keep_oriented_incidence() {
    let transposed = surface().try_swapped_uv().unwrap();
    let source = Brep::try_surface_face(transposed, Tolerance::DEFAULT).unwrap();
    let result = source
        .try_split_face_at_knot(0, SurfaceKnotDirection::V, 1., Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(result.faces.len(), 2);
    assert_eq!(result.edges.len(), 7);
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cylinder = Brep::try_cylinder(frame, 2., 0., 3., Tolerance::DEFAULT).unwrap();
    let side = cylinder
        .faces
        .iter()
        .position(|f| {
            f.loops
                .iter()
                .flat_map(|l| &l.trims)
                .any(|t| t.trim_type == BrepTrimType::Seam)
        })
        .unwrap();
    let source = cylinder.sub_brep(&[side], Tolerance::DEFAULT).unwrap();
    let cut = source.faces[0]
        .surface
        .knots_u()
        .iter()
        .copied()
        .find(|&k| k > *source.faces[0].surface.domain_u().start())
        .unwrap();
    let result = source
        .try_split_face_at_knot(0, SurfaceKnotDirection::U, cut, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(result.faces.len(), 2);
    assert_eq!(
        result.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        2
    );
    assert!(
        result
            .trim_uses()
            .iter()
            .all(|t| t.trim.trim_type != BrepTrimType::Seam)
    );
    assert!(
        (source.area(Tolerance::DEFAULT).unwrap() - result.area(Tolerance::DEFAULT).unwrap()).abs()
            < 1e-9
    );
}

fn planar(rings: &[&[[Real; 2]]]) -> Brep {
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [0., 4.]
            .into_iter()
            .flat_map(|y| [0., 2., 4.].map(|x| Point3::try_new(x, y, 0.).unwrap()))
            .collect(),
        vec![0., 0., 2., 4., 4.],
        vec![0., 0., 4., 4.],
    )
    .unwrap();
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut loops = Vec::new();
    for (ring_index, ring) in rings.iter().enumerate() {
        let base = vertices.len();
        for &[x, y] in *ring {
            vertices.push(BrepVertex::try_new(Point3::try_new(x, y, 0.).unwrap(), 0.).unwrap());
        }
        let mut trims = Vec::new();
        for i in 0..ring.len() {
            let indices = [base + i, base + (i + 1) % ring.len()];
            let points = indices.map(|v| vertices[v].point);
            let edge = edges.len();
            edges.push(
                BrepEdge::try_new(
                    indices,
                    NurbsCurve::try_new(1, points.to_vec(), vec![0., 0., 1., 1.]).unwrap(),
                    0.,
                )
                .unwrap(),
            );
            trims.push(
                BrepTrim::try_new(
                    indices,
                    Some(edge),
                    false,
                    NurbsCurve2::try_line(
                        Point2::try_new(points[0].x(), points[0].y()).unwrap(),
                        Point2::try_new(points[1].x(), points[1].y()).unwrap(),
                    )
                    .unwrap(),
                    BrepTrimType::Boundary,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )
                .unwrap(),
            );
        }
        loops.push(
            BrepLoop::try_new(
                if ring_index == 0 {
                    BrepLoopType::Outer
                } else {
                    BrepLoopType::Inner
                },
                trims,
            )
            .unwrap(),
        );
    }
    Brep::try_new(
        vertices,
        edges,
        vec![BrepFace::try_new(surface, false, loops).unwrap()],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn holes_and_disconnected_halfplane_regions_are_preserved() {
    let rectangle = [[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
    let hole_left = [[0.25, 1.], [0.25, 2.], [0.75, 2.], [0.75, 1.]];
    let hole_crossing = [[1., 1.], [1., 3.], [3., 3.], [3., 1.]];
    let concave = [
        [0., 0.],
        [4., 0.],
        [4., 1.],
        [1., 1.],
        [1., 3.],
        [4., 3.],
        [4., 4.],
        [0., 4.],
    ];
    for (source, count, holes) in [
        (planar(&[&rectangle, &hole_left]), 2, 1),
        (planar(&[&rectangle, &hole_crossing]), 2, 0),
        (planar(&[&concave]), 3, 0),
    ] {
        let result = source
            .try_split_face_at_knot(0, SurfaceKnotDirection::U, 2., Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(result.faces.len(), count);
        assert_eq!(
            result
                .faces
                .iter()
                .map(|f| f.loops.len() - 1)
                .sum::<usize>(),
            holes
        );
        assert_eq!(&result.vertices[..source.vertices.len()], source.vertices);
        assert!(
            (source.area(Tolerance::DEFAULT).unwrap() - result.area(Tolerance::DEFAULT).unwrap())
                .abs()
                < 1e-9
        );
    }
}

#[test]
fn curved_uv_crossings_require_exact_restrictions_and_full_hull_sides() {
    let mut source = planar(&[&[[0., 0.], [4., 0.], [4., 4.], [0., 4.]]]);
    source.edges[0].curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0.5, 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    source.faces[0].loops[0].trims[0].curve = NurbsCurve2::try_new(
        2,
        vec![
            Point2::try_new(0., 0.).unwrap(),
            Point2::try_new(2., 0.5).unwrap(),
            Point2::try_new(4., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    source.validate(Tolerance::DEFAULT).unwrap();
    let result = source
        .try_split_face_at_knot(0, SurfaceKnotDirection::U, 2., Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        result
            .edges
            .iter()
            .filter(|e| e.curve.degree() == 2)
            .count(),
        2
    );
    assert!(
        (source.area(Tolerance::DEFAULT).unwrap() - result.area(Tolerance::DEFAULT).unwrap()).abs()
            < 1e-9
    );
    let before = source.clone();
    assert!(curves::crossings(&source, 0, 0, 2., Tolerance::DEFAULT, &mut Budget(1)).is_err());
    assert_eq!(source, before);
}

#[test]
fn repeated_crease_splits_share_topology_in_both_directions() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    Point3::try_new(
                        u as Real,
                        v as Real,
                        Real::from(u == 2) + Real::from(v == 2),
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    assert!(
        source
            .try_split_kinky_faces(std::f64::consts::PI, Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    let result = source
        .try_split_kinky_faces(0.1, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            result.vertices.len(),
            result.edges.len(),
            result.faces.len()
        ),
        (9, 12, 4)
    );
    assert_eq!(
        result.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        4
    );
    assert!(
        result
            .try_split_kinky_faces(0.1, Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    assert!(
        (source.area(Tolerance::DEFAULT).unwrap() - result.area(Tolerance::DEFAULT).unwrap()).abs()
            < 1e-9
    );
    for angle in [-1., Real::NAN, Real::INFINITY] {
        assert!(
            source
                .try_split_kinky_faces(angle, Tolerance::DEFAULT)
                .is_err()
        );
    }
    assert!(
        source
            .split_face_at_knot_with_budget(
                0,
                SurfaceKnotDirection::U,
                1.,
                Tolerance::DEFAULT,
                &mut Budget(1)
            )
            .is_err()
    );
}

#[test]
fn knot_outside_trimmed_region_does_not_change_or_replace_geometry() {
    let source =
        Brep::try_rectangular_surface_face(surface(), 0.0..=0.5, 0.0..=1.0, Tolerance::DEFAULT)
            .unwrap();
    let before = source.clone();
    assert!(
        source
            .try_split_face_at_knot(0, SurfaceKnotDirection::U, 1., Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    assert!(
        source
            .try_split_kinky_faces(1e-10, Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    assert_eq!(source, before);
}

#[test]
fn translated_and_tiny_uv_domains_preserve_original_parameter_coordinates() {
    for (origin, scale) in [
        (2.0_f64.powi(40), 1.),
        (-2.0_f64.powi(40), 1.),
        (0., 2.0_f64.powi(-30)),
    ] {
        let mut source = Brep::try_surface_face(surface(), Tolerance::DEFAULT).unwrap();
        let face = &mut source.faces[0];
        face.surface = face
            .surface
            .try_reparameterized(origin..=origin + 2. * scale, origin..=origin + scale)
            .unwrap();
        for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
            trim.curve = NurbsCurve2::try_new_rational(
                trim.curve.degree(),
                trim.curve
                    .control_points()
                    .iter()
                    .map(|cp| {
                        WeightedPoint2::try_new(
                            Point2::try_new(
                                origin + scale * cp.point().x(),
                                origin + scale * cp.point().y(),
                            )
                            .unwrap(),
                            cp.weight(),
                        )
                        .unwrap()
                    })
                    .collect(),
                trim.curve.knots().to_vec(),
            )
            .unwrap();
        }
        source.validate(Tolerance::DEFAULT).unwrap();
        let before = source.clone();
        let result = source
            .try_split_face_at_knot(
                0,
                SurfaceKnotDirection::U,
                origin + scale,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .unwrap();
        assert_eq!(source, before);
        assert_eq!(result.faces[0].surface.domain_u(), origin..=origin + scale);
        assert_eq!(
            result.faces[1].surface.domain_u(),
            origin + scale..=origin + 2. * scale
        );
        assert_eq!(
            result.faces[0].surface.domain_v(),
            source.faces[0].surface.domain_v()
        );
        assert_eq!(result.vertices.len(), 6);
        assert_eq!(result.edges.len(), 7);
        assert!(result.faces.iter().all(|f| {
            f.surface
                .control_points()
                .iter()
                .all(|cp| source.faces[0].surface.control_points().contains(cp))
        }));
    }
}

#[test]
fn source_uncertainties_survive_subdivision_in_every_incident_trim() {
    let mut source = Brep::try_surface_grid(&surface(), &[], &[0.5], Tolerance::DEFAULT).unwrap();
    for vertex in &mut source.vertices {
        vertex.tolerance = 0.0125;
    }
    for edge in &mut source.edges {
        edge.tolerance = 0.025;
    }
    for trim in source
        .faces
        .iter_mut()
        .flat_map(|f| &mut f.loops)
        .flat_map(|l| &mut l.trims)
    {
        trim.tolerance = [0.01, 0.02];
    }
    source.validate(Tolerance::DEFAULT).unwrap();
    let result = source
        .try_split_face_at_knot(0, SurfaceKnotDirection::U, 1., Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(&result.vertices[..source.vertices.len()], source.vertices);
    assert!(
        result.vertices[source.vertices.len()..]
            .iter()
            .all(|v| v.tolerance == 0.025)
    );
    assert!(
        result.edges[..result.edges.len() - 1]
            .iter()
            .all(|e| e.tolerance == 0.025)
    );
    let connector = result.edges.len() - 1;
    assert_eq!(result.edges[connector].tolerance, 0.);
    for trim in result
        .faces
        .iter()
        .flat_map(|f| &f.loops)
        .flat_map(|l| &l.trims)
    {
        assert_eq!(
            trim.tolerance,
            if trim.edge == Some(connector) {
                [0.; 2]
            } else {
                [0.01, 0.02]
            }
        );
    }
}

#[test]
fn singular_trim_partition_reuses_the_pole_and_preserves_a_closed_cone() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cone = Brep::try_cone(frame, 2., 3., Tolerance::DEFAULT).unwrap();
    let side = cone
        .faces
        .iter()
        .position(|f| {
            f.loops
                .iter()
                .flat_map(|l| &l.trims)
                .any(|t| t.trim_type == BrepTrimType::Singular)
        })
        .unwrap();
    let surface = &cone.faces[side].surface;
    let cut = *surface
        .knots_u()
        .iter()
        .find(|&&k| k > *surface.domain_u().start())
        .unwrap();
    let before = cone.clone();
    let result = cone
        .try_split_face_at_knot(side, SurfaceKnotDirection::U, cut, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(cone, before);
    assert!(result.is_solid());
    assert_eq!(result.faces.len(), cone.faces.len() + 1);
    assert_eq!(result.vertices.len(), cone.vertices.len() + 1); // rim only, no duplicate pole
    assert_eq!(&result.vertices[..cone.vertices.len()], cone.vertices);
    assert!(
        (result.area(Tolerance::DEFAULT).unwrap() - cone.area(Tolerance::DEFAULT).unwrap()).abs()
            < 1e-9
    );
    assert!(
        (result.signed_volume(Tolerance::DEFAULT).unwrap()
            - cone.signed_volume(Tolerance::DEFAULT).unwrap())
        .abs()
            < 1e-9
    );
    let count = |b: &Brep| {
        b.trim_uses()
            .iter()
            .filter(|t| t.trim.trim_type == BrepTrimType::Singular)
            .count()
    };
    assert_eq!(count(&result), count(&cone) + 1);
    assert!(
        result
            .trim_uses()
            .iter()
            .filter(|t| t.trim.edge.is_none())
            .all(|t| t.trim.vertices[0] == t.trim.vertices[1])
    );
}

#[test]
fn discontinuous_tensor_knots_are_rejected() {
    let disconnected = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| [0., 1., 2., 3.].map(|x| Point3::try_new(x, y, 0.).unwrap()))
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        surface::split(&disconnected, 0, 1., &mut Budget(MAX_WORK)),
        Err(GeometryError::InvalidBrepTopology {
            context: "face partition requires a continuous full-degree knot"
        })
    ));
}

#[test]
fn smooth_clamped_poles_are_neutral_but_regular_samples_detect_real_creases() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for original in [
        NurbsSurface::try_sphere(frame, 2.).unwrap(),
        NurbsSurface::try_cone(frame, 2., 3.).unwrap(),
    ] {
        for transpose in [false, true] {
            let original = if transpose {
                original.try_swapped_uv().unwrap()
            } else {
                original.clone()
            };
            for gauge in [1., -1., 1e-200, 1e200] {
                let surface = NurbsSurface::try_new_rational(
                    original.degree_u(),
                    original.degree_v(),
                    original.control_point_count_u(),
                    original.control_point_count_v(),
                    original
                        .control_points()
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                        .collect(),
                    original.knots_u().to_vec(),
                    original.knots_v().to_vec(),
                )
                .unwrap();
                assert_eq!(
                    surface.sampled_kink_parameters(1e-10).unwrap(),
                    [vec![], vec![]]
                );
                let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
                assert!(
                    source
                        .try_split_kinky_faces(1e-10, Tolerance::DEFAULT)
                        .unwrap()
                        .is_none()
                );
            }
        }
    }
    for radius in [0., 1e-12] {
        let surface = NurbsSurface::try_new(
            1,
            1,
            3,
            2,
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [2., 1., 0.],
                [0., 0., 3.],
                [radius, 0., 3.],
                [2. * radius, radius, 3.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![0., 0., 1., 2., 2.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        assert_eq!(
            surface.sampled_kink_parameters(0.1).unwrap(),
            [vec![1.], vec![]]
        );
    }
}

#[test]
fn hull_authorization_rejects_mixed_weights_and_ambiguous_cut_loci() {
    let weighted = NurbsCurve2::try_new_rational(
        2,
        vec![
            WeightedPoint2::try_new(Point2::try_new(0., 0.).unwrap(), 1.).unwrap(),
            WeightedPoint2::try_new(Point2::try_new(0.5, 0.).unwrap(), -0.1).unwrap(),
            WeightedPoint2::try_new(Point2::try_new(1., 0.).unwrap(), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(rings::side(&weighted, 0, 2.).is_err());
    let reversing = NurbsCurve2::try_new(
        2,
        vec![
            Point2::try_new(2., 0.).unwrap(),
            Point2::try_new(2., 2.).unwrap(),
            Point2::try_new(2., 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(rings::side(&reversing, 0, 2.).is_err());
    let straddling = NurbsCurve2::try_new(
        2,
        vec![
            Point2::try_new(0., 0.).unwrap(),
            Point2::try_new(3., 1.).unwrap(),
            Point2::try_new(0., 2.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    // This curve never reaches x=2, but its hull does: reject classification
    // rather than treat endpoint/midpoint samples as a whole-curve proof.
    assert_eq!(rings::side(&straddling, 0, 2.).unwrap(), None);
}

#[test]
fn angular_scan_cost_accounts_for_cross_product_of_knots_and_transverse_samples() {
    let mut budget = Budget(100);
    assert!(charge_kink_search(&surface(), &mut budget).is_err());
    let mut budget = Budget(MAX_WORK);
    charge_kink_search(&surface(), &mut budget).unwrap();
    assert!(MAX_WORK - budget.0 > surface().control_points().len() * 8);
}

#[test]
fn tensor_patches_copy_controls_weights_and_unclamped_knots_exactly() {
    for gauge in [1e-200, -1., 1e200] {
        let surface = NurbsSurface::try_new_rational(
            2,
            1,
            5,
            2,
            (0..2)
                .flat_map(|v| {
                    (0..5).map(move |u| {
                        WeightedPoint3::try_new(
                            Point3::try_new(u as Real, v as Real, (u % 2) as Real).unwrap(),
                            gauge * (u + 1) as Real,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![-2., -1., 0., 2., 2., 4., 5., 6.],
            vec![-3., -3., 7., 7.],
        )
        .unwrap();
        let parts = surface::split(&surface, 0, 2., &mut Budget(MAX_WORK)).unwrap();
        for (part, offset) in parts.iter().zip([0, 2]) {
            assert_eq!(part.control_point_count_u(), 3);
            assert_eq!(part.knots_v(), surface.knots_v());
            for v in 0..2 {
                for u in 0..3 {
                    assert_eq!(
                        part.control_point(u, v),
                        surface.control_point(u + offset, v)
                    );
                }
            }
        }
        assert_eq!(parts[0].knots_u(), &[-2., -1., 0., 2., 2., 2.]);
        assert_eq!(parts[1].knots_u(), &[2., 2., 2., 4., 5., 6.]);
    }
}
