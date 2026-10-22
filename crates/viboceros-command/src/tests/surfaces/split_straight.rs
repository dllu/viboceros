//! surfaces split straight command regressions.
use super::*;

#[test]
fn cutting_object_split_replaces_surface_source_with_exact_brep_faces() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New CuttingSplitSurfaces")
        .unwrap();
    let source_surface = planar_intersection_surface();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(source_surface.clone()))
        .unwrap();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName CuttingSplitSource")
        .unwrap();
    registry
        .execute(&mut document, "Group CuttingSplitPieces")
        .unwrap();
    let source = document.object(source_id).unwrap().clone();
    let cutter_surface = transverse_surface_at_x(4.0);
    let cutter_id = document
        .add_geometry(Geometry::NurbsSurface(cutter_surface.clone()))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=2,5,0")
            .unwrap(),
        "Split the selected surface at 1 complete isoparametric intersection(s) into 2 exact B-rep face(s)"
    );
    assert!(document.object(source_id).is_none());
    assert_eq!(
        document.object(cutter_id).unwrap().geometry(),
        &Geometry::NurbsSurface(cutter_surface)
    );
    assert!(!document.is_selected(cutter_id));

    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let group_members = document
        .group_by_name("CuttingSplitPieces")
        .unwrap()
        .members()
        .collect::<BTreeSet<_>>();
    let expected_bounds = [[[0.0, 0.4], [0.0, 1.0]], [[0.4, 1.0], [0.0, 1.0]]];
    for (object, expected) in outputs.iter().zip(expected_bounds) {
        assert_eq!(object.attributes(), source.attributes());
        assert!(group_members.contains(&object.id()));
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("surface cutting Split must create B-rep faces")
        };
        assert_eq!(brep.faces()[0].surface(), &source_surface);
        assert_surface_bounds_near(
            brep.faces()[0]
                .rectangular_trim_bounds(document.tolerance())
                .unwrap()
                .unwrap(),
            expected,
            document.tolerance(),
        );
        brep.tessellate(2, document.tolerance()).unwrap();
    }
    let west = match outputs[0].geometry() {
        Geometry::Brep(brep) => brep,
        _ => unreachable!(),
    };
    let east = match outputs[1].geometry() {
        Geometry::Brep(brep) => brep,
        _ => unreachable!(),
    };
    assert_eq!(
        west.edges()
            .iter()
            .map(|edge| edge.vertices())
            .collect::<Vec<_>>(),
        vec![[0, 2], [1, 0], [2, 3], [3, 1]]
    );
    assert_eq!(
        east.edges()
            .iter()
            .map(|edge| edge.vertices())
            .collect::<Vec<_>>(),
        vec![[0, 1], [1, 3], [2, 3], [2, 0]]
    );
    assert_eq!(
        east.faces()[0].loops()[0]
            .trims()
            .iter()
            .map(|trim| (trim.edge(), trim.is_reversed_3d()))
            .collect::<Vec<_>>(),
        vec![
            (Some(3), false),
            (Some(0), false),
            (Some(1), false),
            (Some(2), true)
        ]
    );
    assert_eq!(document.undo_label(), Some("Split"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(source_id).unwrap(), &source);
    assert!(document.object(cutter_id).is_some());
}

#[test]
fn cutting_object_split_supports_perpendicular_multiple_and_trimmed_surface_sources() {
    let registry = CommandRegistry::with_builtins();

    let mut perpendicular = Document::default();
    let source_id = perpendicular
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    let cutter_id = perpendicular
        .add_geometry(Geometry::NurbsSurface(transverse_surface_at_y(6.0)))
        .unwrap();
    perpendicular
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut perpendicular, "Split CuttingObjects=5,2,0")
        .unwrap();
    let perpendicular_outputs = perpendicular.selected_objects().collect::<Vec<_>>();
    assert_eq!(perpendicular_outputs.len(), 2);
    let expected_v = [[[0.0, 1.0], [0.0, 0.6]], [[0.0, 1.0], [0.6, 1.0]]];
    for (object, expected) in perpendicular_outputs.iter().zip(expected_v) {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("perpendicular cutting Split must create B-rep faces")
        };
        assert_surface_bounds_near(
            brep.faces()[0]
                .rectangular_trim_bounds(perpendicular.tolerance())
                .unwrap()
                .unwrap(),
            expected,
            perpendicular.tolerance(),
        );
    }

    let mut curve_cutter = Document::default();
    let source_id = curve_cutter
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    registry
        .execute(&mut curve_cutter, "Line 4,0,0 4,10,0")
        .unwrap();
    let cutter_id = curve_cutter
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap()
        .id();
    curve_cutter
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut curve_cutter, "Split CuttingObjects=2,5,0")
        .unwrap();
    assert_eq!(curve_cutter.selected_object_count(), 2);
    assert!(curve_cutter.selected_objects().all(|object| {
        matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
    }));

    let mut multiple = Document::default();
    let source_id = multiple
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    let cutter_ids = [3.0, 7.0, 7.0].map(|x| {
        multiple
            .add_geometry(Geometry::NurbsSurface(transverse_surface_at_x(x)))
            .unwrap()
    });
    multiple
        .select_objects_direct(
            std::iter::once(source_id).chain(cutter_ids),
            SelectionMode::Replace,
        )
        .unwrap();
    registry
        .execute(&mut multiple, "Split CuttingObjects=1,5,0")
        .unwrap();
    let bounds = multiple
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Brep(brep) => brep.faces()[0]
                .rectangular_trim_bounds(multiple.tolerance())
                .unwrap()
                .unwrap(),
            geometry => panic!("multiple cutting Split selected {geometry:?}"),
        })
        .collect::<Vec<_>>();
    for (actual, expected) in bounds.into_iter().zip([
        [[0.0, 0.3], [0.0, 1.0]],
        [[0.3, 0.7], [0.0, 1.0]],
        [[0.7, 1.0], [0.0, 1.0]],
    ]) {
        assert_surface_bounds_near(actual, expected, multiple.tolerance());
    }

    let mut trimmed = Document::default();
    let source_surface = planar_intersection_surface();
    let source_id = trimmed
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face_with_orientation(
                source_surface.clone(),
                0.2..=0.8,
                0.1..=0.9,
                true,
                trimmed.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let cutter_id = trimmed
        .add_geometry(Geometry::Brep(
            Brep::try_surface_face(transverse_surface_at_x(4.0), trimmed.tolerance()).unwrap(),
        ))
        .unwrap();
    trimmed
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut trimmed, "Split CuttingObjects=3,5,0")
        .unwrap();
    let expected_trimmed = [[[0.2, 0.4], [0.1, 0.9]], [[0.4, 0.8], [0.1, 0.9]]];
    for (object, expected) in trimmed.selected_objects().zip(expected_trimmed) {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("trimmed cutting Split must create B-rep faces")
        };
        assert!(brep.faces()[0].is_reversed());
        assert_eq!(brep.faces()[0].surface(), &source_surface);
        assert_surface_bounds_near(
            brep.faces()[0]
                .rectangular_trim_bounds(trimmed.tolerance())
                .unwrap()
                .unwrap(),
            expected,
            trimmed.tolerance(),
        );
    }
}

#[test]
fn cutting_object_split_supports_straight_opposite_boundary_surface_cuts() {
    let registry = CommandRegistry::with_builtins();
    for (cutter, source_pick, expected_edges, expected_trim_reversals) in [
        (
            west_east_diagonal_cutting_surface(),
            "5,1,0",
            [
                vec![[0, 1], [1, 3], [2, 3], [2, 0]],
                vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            ],
            [true, false],
        ),
        (
            south_north_diagonal_cutting_surface(),
            "1,5,0",
            [
                vec![[0, 2], [1, 0], [2, 3], [3, 1]],
                vec![[0, 1], [1, 3], [2, 3], [2, 0]],
            ],
            [false, true],
        ),
    ] {
        let mut document = Document::default();
        let source = planar_intersection_surface();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(source.clone()))
            .unwrap();
        let cutter_id = document
            .add_geometry(Geometry::NurbsSurface(cutter))
            .unwrap();
        document
            .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            registry
                .execute(
                    &mut document,
                    &format!("Split CuttingObjects={source_pick}"),
                )
                .unwrap(),
            "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
        );
        assert!(document.object(source_id).is_none());
        assert!(!document.is_selected(cutter_id));
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut area = 0.0;
        for ((object, expected_edges), expected_reversed_trim) in outputs
            .iter()
            .zip(expected_edges)
            .zip(expected_trim_reversals)
        {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("diagonal cutting Split must create B-rep faces")
            };
            let face = &brep.faces()[0];
            assert_eq!(face.surface(), &source);
            assert_eq!(
                face.rectangular_trim_bounds(document.tolerance()).unwrap(),
                None
            );
            let nonisoparametric = face.loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .unwrap();
            assert_eq!(nonisoparametric.is_reversed_3d(), expected_reversed_trim);
            assert_eq!(
                brep.edges()
                    .iter()
                    .map(|edge| edge.vertices())
                    .collect::<Vec<_>>(),
                expected_edges
            );
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(2, document.tolerance()).unwrap();
        }
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn surface_cut_parameter_paths_must_be_admissible() {
    let epsilon = 1.0e-9;
    assert!(surface_cut_parameters_form_collinear_segment(
        &[[0.0, 0.0], [0.5, 0.25], [1.0, 0.5]],
        epsilon,
        epsilon,
    ));
    assert!(!surface_cut_parameters_form_collinear_segment(
        &[[0.0, 0.0], [0.75, 0.375], [0.5, 0.25], [1.0, 0.5]],
        epsilon,
        epsilon,
    ));
    assert!(!surface_cut_parameters_form_collinear_segment(
        &[[0.0, 0.0], [0.5, 0.3], [1.0, 0.5]],
        epsilon,
        epsilon,
    ));

    let parameter_curve = |points: &[[Real; 2]], weights: &[Real]| {
        let degree = points.len() - 1;
        NurbsCurve2::try_new_rational(
            degree,
            points
                .iter()
                .zip(weights)
                .map(|(point, weight)| {
                    viboceros_geometry::WeightedPoint2::try_new(
                        Point2::try_new(point[0], point[1]).unwrap(),
                        *weight,
                    )
                    .unwrap()
                })
                .collect(),
            std::iter::repeat_n(0.0, degree + 1)
                .chain(std::iter::repeat_n(1.0, degree + 1))
                .collect(),
        )
        .unwrap()
    };
    let bounds = [[0.0, 10.0], [0.0, 10.0]];
    assert!(surface_cut_parameter_curve_is_simple_in_bounds(
        &parameter_curve(&[[0.0, 2.0], [5.0, 9.0], [10.0, 8.0]], &[1.0, 0.75, 1.0]),
        bounds,
        [epsilon, epsilon],
    ));
    assert!(!surface_cut_parameter_curve_is_simple_in_bounds(
        &parameter_curve(&[[0.0, 2.0], [5.0, 11.0], [10.0, 8.0]], &[1.0, 0.75, 1.0],),
        bounds,
        [epsilon, epsilon],
    ));
    assert!(!surface_cut_parameter_curve_is_simple_in_bounds(
        &parameter_curve(
            &[[0.0, 2.0], [7.0, 8.0], [3.0, 3.0], [10.0, 8.0]],
            &[1.0, 1.0, 1.0, 1.0],
        ),
        bounds,
        [epsilon, epsilon],
    ));
    let backtracking_vertical_span = NurbsCurve2::try_new(
        1,
        [[0.0, 2.0], [5.0, 2.0], [5.0, 8.0], [5.0, 4.0], [10.0, 8.0]]
            .into_iter()
            .map(|point| Point2::try_new(point[0], point[1]).unwrap())
            .collect(),
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
    )
    .unwrap();
    assert!(!surface_cut_parameter_curve_is_simple_in_bounds(
        &backtracking_vertical_span,
        bounds,
        [epsilon, epsilon],
    ));
    assert!(surface_cut_parameter_curve_is_simple_in_bounds(
        &parameter_curve(
            &[
                [0.0, 0.0],
                [10.0 / 3.0, 0.0],
                [20.0 / 3.0, 10.0 / 3.0],
                [10.0, 10.0],
            ],
            &[1.0, 1.0, 1.0, 1.0],
        ),
        bounds,
        [epsilon, epsilon],
    ));
    let internal_boundary_touch = NurbsCurve2::try_new(
        1,
        [[0.0, 2.0], [5.0, 0.0], [10.0, 2.0]]
            .into_iter()
            .map(|point| Point2::try_new(point[0], point[1]).unwrap())
            .collect(),
        vec![0.0, 0.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    assert!(!surface_cut_parameter_curve_is_simple_in_bounds(
        &internal_boundary_touch,
        bounds,
        [epsilon, epsilon],
    ));
    assert!(!surface_cut_parameter_curve_is_simple_in_bounds(
        &parameter_curve(&[[0.0, 2.0], [5.0, 9.0], [10.0, 8.0]], &[1.0, -0.75, 1.0],),
        bounds,
        [epsilon, epsilon],
    ));
}

#[test]
fn curved_surface_cut_clipping_partitions_nonmonotone_control_polygons_exactly() {
    let surface = planar_intersection_surface();
    let tolerance = Tolerance::default();
    let curve = NurbsCurve::try_new(
        3,
        [[0.0, 2.0], [2.0, 1.0], [2.0, 5.0], [6.0, 4.0]]
            .into_iter()
            .map(|point| Point3::try_new(point[0], point[1], 0.0).unwrap())
            .collect(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let parameter_curve = surface
        .try_pullback_bilinear_curve(&curve, tolerance)
        .unwrap();
    let bounds = [[0.0, 0.4], [0.0, 0.6]];
    let epsilon = [
        surface_split_parameter_epsilon(bounds[0], tolerance),
        surface_split_parameter_epsilon(bounds[1], tolerance),
    ];
    assert!(
        (0..2).all(|coordinate| {
            surface_cut_parameter_curve_strict_direction(
                &parameter_curve,
                coordinate,
                epsilon[coordinate],
            )
            .is_none()
        }),
        "the untrimmed control polygon must exercise generalized clipping"
    );

    let clipped =
        clip_curved_surface_cut_to_bounds(&curve, &parameter_curve, bounds, epsilon, tolerance)
            .unwrap();
    assert_eq!(clipped.len(), 1);
    let clipped = &clipped[0];
    assert_eq!(clipped.degree(), 3);
    assert_eq!(*clipped.domain().start(), 0.0);
    assert!(*clipped.domain().end() < 1.0);
    let endpoints = [
        clipped.evaluate(*clipped.domain().start()).unwrap(),
        clipped.evaluate(*clipped.domain().end()).unwrap(),
    ];
    assert!(tolerance.approx_eq(endpoints[0].x(), 0.0));
    assert!(tolerance.approx_eq(endpoints[0].y(), 2.0));
    assert!(tolerance.approx_eq(endpoints[1].x(), 4.0));
    let controls = clipped.control_points();
    assert_eq!(controls[0].weight(), 1.0);
    assert_eq!(controls[controls.len() - 1].weight(), 1.0);

    let clipped_parameter_curve = surface
        .try_pullback_bilinear_curve(clipped, tolerance)
        .unwrap();
    assert!(surface_cut_parameter_curve_is_simple_in_bounds(
        &clipped_parameter_curve,
        bounds,
        epsilon,
    ));
}

#[test]
fn cutting_object_split_supports_all_straight_adjacent_boundary_surface_cuts() {
    let registry = CommandRegistry::with_builtins();
    for (endpoints, expected_edges, expected_trim_reversals) in [
        (
            [[2.0, 0.0], [10.0, 7.0]],
            [
                vec![[0, 3], [1, 2], [2, 0], [3, 4], [4, 1]],
                vec![[0, 2], [1, 2], [1, 0]],
            ],
            [false, true],
        ),
        (
            [[10.0, 2.0], [7.0, 10.0]],
            [
                vec![[0, 1], [1, 3], [2, 0], [3, 4], [4, 2]],
                vec![[0, 2], [1, 2], [1, 0]],
            ],
            [false, true],
        ),
        (
            [[8.0, 10.0], [0.0, 3.0]],
            [
                vec![[0, 1], [1, 2], [2, 3], [3, 4], [4, 0]],
                vec![[0, 2], [1, 2], [1, 0]],
            ],
            [false, true],
        ),
        (
            [[0.0, 8.0], [3.0, 0.0]],
            [
                vec![[0, 2], [1, 2], [1, 0]],
                vec![[0, 1], [1, 2], [2, 3], [3, 4], [4, 0]],
            ],
            [true, false],
        ),
    ] {
        let mut document = Document::default();
        let source = planar_intersection_surface();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(source.clone()))
            .unwrap();
        let cutter_id = document
            .add_geometry(Geometry::NurbsSurface(straight_cutting_surface(
                endpoints[0],
                endpoints[1],
            )))
            .unwrap();
        document
            .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            registry
                .execute(&mut document, "Split CuttingObjects=5,5,0")
                .unwrap(),
            "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
        );
        assert!(document.object(source_id).is_none());
        assert!(!document.is_selected(cutter_id));
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut area = 0.0;
        for ((object, expected_edges), expected_reversed_trim) in outputs
            .iter()
            .zip(expected_edges)
            .zip(expected_trim_reversals)
        {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("adjacent-side cutting Split must create B-rep faces")
            };
            let face = &brep.faces()[0];
            assert_eq!(face.surface(), &source);
            assert_eq!(
                face.rectangular_trim_bounds(document.tolerance()).unwrap(),
                None
            );
            let nonisoparametric = face.loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .unwrap();
            assert_eq!(nonisoparametric.is_reversed_3d(), expected_reversed_trim);
            assert_eq!(
                brep.edges()
                    .iter()
                    .map(|edge| edge.vertices())
                    .collect::<Vec<_>>(),
                expected_edges
            );
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(2, document.tolerance()).unwrap();
        }
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn cutting_object_split_clips_diagonal_curves_to_trimmed_surface_sources() {
    let registry = CommandRegistry::with_builtins();
    let diagonal_length = 136.0_f64.sqrt();
    for (bounds, source_pick, expected_edges, expected_cut_domain) in [
        (
            [[0.0, 0.4], [0.0, 0.6]],
            "1,1,0",
            [
                vec![[0, 1], [1, 3], [2, 3], [2, 0]],
                vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            ],
            [0.0, diagonal_length * 0.4],
        ),
        (
            [[0.4, 1.0], [0.6, 1.0]],
            "8,9,0",
            [
                vec![[0, 3], [1, 2], [2, 0], [3, 4], [4, 1]],
                vec![[0, 2], [1, 2], [1, 0]],
            ],
            [diagonal_length * (2.0 / 3.0), diagonal_length],
        ),
    ] {
        let mut document = Document::default();
        let source = planar_intersection_surface();
        let source_id = document
            .add_geometry(Geometry::Brep(
                Brep::try_rectangular_surface_face_with_orientation(
                    source.clone(),
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    true,
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let cutter_id = document
            .add_geometry(Geometry::NurbsSurface(west_east_diagonal_cutting_surface()))
            .unwrap();
        document
            .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
            .unwrap();

        assert_eq!(
            registry
                .execute(
                    &mut document,
                    &format!("Split CuttingObjects={source_pick}"),
                )
                .unwrap(),
            "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
        );
        assert!(document.object(source_id).is_none());
        assert!(!document.is_selected(cutter_id));
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut area = 0.0;
        for (object, expected_edges) in outputs.iter().zip(expected_edges) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("trim-clipped cutting Split must create B-rep faces")
            };
            assert!(brep.faces()[0].is_reversed());
            assert_eq!(brep.faces()[0].surface(), &source);
            assert_eq!(
                brep.edges()
                    .iter()
                    .map(|edge| edge.vertices())
                    .collect::<Vec<_>>(),
                expected_edges
            );
            let cut_edge = brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .and_then(viboceros_geometry::BrepTrim::edge)
                .unwrap();
            let actual_domain = brep.edges()[cut_edge].curve().domain();
            assert!(
                document
                    .tolerance()
                    .approx_eq(*actual_domain.start(), expected_cut_domain[0])
            );
            assert!(
                document
                    .tolerance()
                    .approx_eq(*actual_domain.end(), expected_cut_domain[1])
            );
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(2, document.tolerance()).unwrap();
        }
        assert!(document.tolerance().approx_eq(area, 24.0));
    }
}
