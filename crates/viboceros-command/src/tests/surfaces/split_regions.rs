//! surfaces split regions command regressions.
use super::*;

#[test]
fn cutting_object_split_supports_all_straight_corner_surface_cuts() {
    let registry = CommandRegistry::with_builtins();
    for (endpoints, expected_vertex_counts) in [
        ([[0.0, 0.0], [10.0, 7.0]], [3, 4]),
        ([[0.0, 0.0], [6.0, 10.0]], [3, 4]),
        ([[10.0, 0.0], [4.0, 10.0]], [4, 3]),
        ([[10.0, 0.0], [0.0, 6.0]], [3, 4]),
        ([[10.0, 10.0], [0.0, 3.0]], [4, 3]),
        ([[10.0, 10.0], [4.0, 0.0]], [4, 3]),
        ([[0.0, 10.0], [6.0, 0.0]], [3, 4]),
        ([[0.0, 10.0], [10.0, 4.0]], [4, 3]),
        ([[0.0, 0.0], [10.0, 10.0]], [3, 3]),
        ([[10.0, 0.0], [0.0, 10.0]], [3, 3]),
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
        for (object, expected_vertex_count) in outputs.iter().zip(expected_vertex_counts) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("corner cutting Split must create B-rep faces")
            };
            assert_eq!(brep.vertices().len(), expected_vertex_count);
            assert_eq!(brep.edges().len(), expected_vertex_count);
            assert_eq!(brep.faces()[0].surface(), &source);
            assert_eq!(
                brep.faces()[0].loops()[0]
                    .trims()
                    .iter()
                    .filter(|trim| { trim.iso() == viboceros_geometry::SurfaceIso::NotIso })
                    .count(),
                1
            );
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(2, document.tolerance()).unwrap();
        }
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn cutting_object_split_preserves_curved_parameter_trims_in_all_boundary_arrangements() {
    let registry = CommandRegistry::with_builtins();
    for (points, expected_vertex_counts) in [
        ([[0.0, 2.0], [5.0, 9.0], [10.0, 8.0]], [4, 4]),
        ([[2.0, 0.0], [8.0, 5.0], [8.0, 10.0]], [4, 4]),
        ([[2.0, 0.0], [7.0, 2.0], [10.0, 7.0]], [5, 3]),
        ([[10.0, 2.0], [8.0, 6.0], [7.0, 10.0]], [5, 3]),
        ([[8.0, 10.0], [3.0, 8.0], [0.0, 3.0]], [5, 3]),
        ([[0.0, 8.0], [1.0, 4.0], [3.0, 0.0]], [3, 5]),
        ([[0.0, 0.0], [3.0, 6.0], [10.0, 7.0]], [3, 4]),
        ([[0.0, 0.0], [5.0, 3.0], [6.0, 10.0]], [3, 4]),
        ([[10.0, 0.0], [8.0, 6.0], [4.0, 10.0]], [4, 3]),
        ([[10.0, 0.0], [4.0, 2.0], [0.0, 6.0]], [3, 4]),
        ([[10.0, 10.0], [4.0, 8.0], [0.0, 3.0]], [4, 3]),
        ([[10.0, 10.0], [8.0, 4.0], [4.0, 0.0]], [4, 3]),
        ([[0.0, 10.0], [2.0, 4.0], [6.0, 0.0]], [3, 4]),
        ([[0.0, 10.0], [6.0, 8.0], [10.0, 4.0]], [4, 3]),
        ([[0.0, 0.0], [2.0, 8.0], [10.0, 10.0]], [3, 3]),
        ([[10.0, 0.0], [2.0, 2.0], [0.0, 10.0]], [3, 3]),
    ] {
        let mut document = Document::default();
        let source = planar_intersection_surface();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(source.clone()))
            .unwrap();
        let cutter = curved_cutting_curve(points);
        let cutter_id = document
            .add_geometry(Geometry::NurbsCurve(cutter.clone()))
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
        let mut trim_directions = Vec::new();
        for (object, expected_vertex_count) in outputs.iter().zip(expected_vertex_counts) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("curved cutting Split must create B-rep faces")
            };
            assert_eq!(brep.vertices().len(), expected_vertex_count);
            assert_eq!(brep.faces()[0].surface(), &source);
            let trim = brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .unwrap();
            assert_eq!(trim.curve().degree(), 2);
            assert_eq!(trim.curve().control_points().len(), 3);
            assert_eq!(
                trim.curve()
                    .control_points()
                    .iter()
                    .map(|control| control.weight())
                    .collect::<Vec<_>>(),
                vec![1.0, 0.75, 1.0]
            );
            let expected_domain = if trim.is_reversed_3d() {
                -1.0..=-0.0
            } else {
                0.0..=1.0
            };
            assert_eq!(trim.curve().domain(), expected_domain);
            trim_directions.push(trim.is_reversed_3d());
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(3, document.tolerance()).unwrap();
        }
        trim_directions.sort_unstable();
        assert_eq!(trim_directions, vec![false, true]);
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn cutting_object_split_supports_curved_cuts_ending_on_the_same_side() {
    let registry = CommandRegistry::with_builtins();
    for (points, source_pick) in [
        ([[0.0, 2.0], [5.0, 5.0], [0.0, 8.0]], "8,5,0"),
        ([[2.0, 0.0], [5.0, 5.0], [8.0, 0.0]], "5,8,0"),
        ([[10.0, 2.0], [5.0, 5.0], [10.0, 8.0]], "2,5,0"),
        ([[8.0, 10.0], [5.0, 5.0], [2.0, 10.0]], "5,2,0"),
    ] {
        let mut document = Document::default();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap();
        let cutter = NurbsCurve::try_new(
            2,
            points
                .into_iter()
                .map(|point| Point3::try_new(point[0], point[1], 0.0).unwrap())
                .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let cutter_id = document
            .add_geometry(Geometry::NurbsCurve(cutter.clone()))
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
            "Split the selected surface along 1 complete boundary-to-boundary curve(s) into 2 exact B-rep face(s)"
        );
        assert!(document.object(source_id).is_none());
        assert!(!document.is_selected(cutter_id));
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut edge_counts = Vec::new();
        let mut area = 0.0;
        for object in outputs {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("same-side cutting Split must create B-rep faces")
            };
            edge_counts.push(brep.edges().len());
            let trim = brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .unwrap();
            assert_eq!(brep.edges()[trim.edge().unwrap()].curve(), &cutter);
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(3, document.tolerance()).unwrap();
        }
        edge_counts.sort_unstable();
        assert_eq!(edge_counts, vec![2, 6]);
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn cutting_object_split_supports_same_side_curves_through_corners() {
    let registry = CommandRegistry::with_builtins();
    for (points, source_pick, expected_edge_counts) in [
        ([[0.0, 0.0], [5.0, 4.0], [0.0, 8.0]], "8,5,0", [2, 5]),
        ([[0.0, 0.0], [4.0, 5.0], [8.0, 0.0]], "5,8,0", [2, 5]),
        ([[10.0, 0.0], [6.0, 5.0], [2.0, 0.0]], "5,8,0", [2, 5]),
        ([[10.0, 0.0], [5.0, 4.0], [10.0, 8.0]], "2,5,0", [2, 5]),
        ([[10.0, 10.0], [5.0, 6.0], [10.0, 2.0]], "2,5,0", [2, 5]),
        ([[10.0, 10.0], [6.0, 5.0], [2.0, 10.0]], "5,2,0", [2, 5]),
        ([[0.0, 10.0], [4.0, 5.0], [8.0, 10.0]], "5,2,0", [2, 5]),
        ([[0.0, 10.0], [5.0, 6.0], [0.0, 2.0]], "8,5,0", [2, 5]),
        ([[0.0, 0.0], [5.0, 5.0], [10.0, 0.0]], "5,8,0", [2, 4]),
        ([[10.0, 0.0], [5.0, 5.0], [10.0, 10.0]], "2,5,0", [2, 4]),
        ([[10.0, 10.0], [5.0, 5.0], [0.0, 10.0]], "5,2,0", [2, 4]),
        ([[0.0, 10.0], [5.0, 5.0], [0.0, 0.0]], "8,5,0", [2, 4]),
    ] {
        let mut document = Document::default();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap();
        let cutter = NurbsCurve::try_new(
            2,
            points
                .into_iter()
                .map(|point| Point3::try_new(point[0], point[1], 0.0).unwrap())
                .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let cutter_id = document.add_geometry(Geometry::NurbsCurve(cutter)).unwrap();
        document
            .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
            .unwrap();

        registry
            .execute(
                &mut document,
                &format!("Split CuttingObjects={source_pick}"),
            )
            .unwrap();
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut edge_counts = Vec::new();
        let mut area = 0.0;
        for object in outputs {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("corner same-side cutting Split must create B-rep faces")
            };
            edge_counts.push(brep.edges().len());
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(3, document.tolerance()).unwrap();
        }
        edge_counts.sort_unstable();
        assert_eq!(edge_counts, expected_edge_counts);
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn cutting_object_split_supports_closed_interior_curves() {
    let registry = CommandRegistry::with_builtins();
    let polygon = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(3.0, 3.0, 0.0).unwrap(),
            Point3::try_new(7.0, 3.0, 0.0).unwrap(),
            Point3::try_new(7.0, 7.0, 0.0).unwrap(),
            Point3::try_new(3.0, 7.0, 0.0).unwrap(),
            Point3::try_new(3.0, 3.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
    )
    .unwrap();
    let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
    let circle = NurbsCurve::try_new_rational(
        2,
        [
            ([7.0, 5.0], 1.0),
            ([7.0, 7.0], diagonal_weight),
            ([5.0, 7.0], 1.0),
            ([3.0, 7.0], diagonal_weight),
            ([3.0, 5.0], 1.0),
            ([3.0, 3.0], diagonal_weight),
            ([5.0, 3.0], 1.0),
            ([7.0, 3.0], diagonal_weight),
            ([7.0, 5.0], 1.0),
        ]
        .into_iter()
        .map(|(coordinates, weight)| {
            WeightedPoint3::try_new(
                Point3::try_new(coordinates[0], coordinates[1], 0.0).unwrap(),
                weight,
            )
            .unwrap()
        })
        .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
    )
    .unwrap();

    for (cutter, expected_edge_counts) in [(polygon, [4, 8]), (circle, [1, 5])] {
        let mut document = Document::default();
        let source_id = document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap();
        let cutter_id = document.add_geometry(Geometry::NurbsCurve(cutter)).unwrap();
        document
            .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
            .unwrap();

        let message = registry
            .execute(&mut document, "Split CuttingObjects=1,1,0")
            .unwrap();
        assert!(message.contains("1 closed interior curve"));
        assert!(document.object(source_id).is_none());
        assert!(!document.is_selected(cutter_id));
        let outputs = document.selected_objects().collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut edge_counts = Vec::new();
        let mut loop_counts = Vec::new();
        let mut area = 0.0;
        for object in outputs {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("closed cutting Split must create B-rep faces")
            };
            edge_counts.push(brep.edges().len());
            loop_counts.push(brep.faces()[0].loops().len());
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(4, document.tolerance()).unwrap();
        }
        edge_counts.sort_unstable();
        loop_counts.sort_unstable();
        assert_eq!(edge_counts, expected_edge_counts);
        assert_eq!(loop_counts, vec![1, 2]);
        assert!(document.tolerance().approx_eq(area, 100.0));
    }
}

#[test]
fn cutting_object_split_supports_multiple_closed_interior_curves() {
    let registry = CommandRegistry::with_builtins();
    let polygon = |x0: Real, x1: Real, y0: Real, y1: Real| {
        NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(x0, y0, 0.0).unwrap(),
                Point3::try_new(x1, y0, 0.0).unwrap(),
                Point3::try_new(x1, y1, 0.0).unwrap(),
                Point3::try_new(x0, y1, 0.0).unwrap(),
                Point3::try_new(x0, y0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap()
    };
    let mut document = Document::default();
    let source_id = document
        .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
        .unwrap();
    let cutter_ids = [
        document
            .add_geometry(Geometry::NurbsCurve(polygon(1.0, 4.0, 2.0, 5.0)))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsCurve(polygon(6.0, 9.0, 5.0, 8.0)))
            .unwrap(),
    ];
    document
        .select_objects_direct(
            [source_id, cutter_ids[0], cutter_ids[1]],
            SelectionMode::Replace,
        )
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=5,1,0")
            .unwrap(),
        "Split the selected surface along 2 closed interior curve(s) into 3 exact B-rep face(s)"
    );
    assert!(document.object(source_id).is_none());
    assert!(
        cutter_ids
            .iter()
            .all(|id| document.object(*id).is_some() && !document.is_selected(*id))
    );
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 3);
    let mut edge_counts = Vec::new();
    let mut loop_counts = Vec::new();
    let mut area = 0.0;
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("multiple closed cutting Split must create B-rep faces")
        };
        edge_counts.push(brep.edges().len());
        loop_counts.push(brep.faces()[0].loops().len());
        area += brep.area(document.tolerance()).unwrap();
        brep.tessellate(4, document.tolerance()).unwrap();
    }
    edge_counts.sort_unstable();
    loop_counts.sort_unstable();
    assert_eq!(edge_counts, vec![4, 4, 12]);
    assert_eq!(loop_counts, vec![1, 1, 3]);
    assert!(document.tolerance().approx_eq(area, 100.0));
}
