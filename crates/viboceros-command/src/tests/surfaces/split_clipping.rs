//! surfaces split clipping command regressions.
use super::*;

#[test]
fn cutting_object_split_clips_curved_nurbs_to_trimmed_surface_sources() {
    let registry = CommandRegistry::with_builtins();
    for (bounds, source_pick, expected_vertex_counts, boundary_coordinates) in [
        (
            [[0.0, 0.4], [0.0, 0.6]],
            "1,1,0",
            [4, 4],
            [(0, 0.0), (0, 4.0)],
        ),
        (
            [[0.4, 1.0], [0.6, 1.0]],
            "8,9,0",
            [5, 3],
            [(1, 6.0), (0, 10.0)],
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
        let cutter = curved_cutting_curve([[0.0, 2.0], [5.0, 7.0], [10.0, 8.0]]);
        let cutter_id = document.add_geometry(Geometry::NurbsCurve(cutter)).unwrap();
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
        let mut cut_domains = Vec::new();
        let mut trim_directions = Vec::new();
        for (object, expected_vertex_count) in outputs.iter().zip(expected_vertex_counts) {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("curved trim-clipped cutting Split must create B-rep faces")
            };
            assert!(brep.faces()[0].is_reversed());
            assert_eq!(brep.faces()[0].surface(), &source);
            assert_eq!(brep.vertices().len(), expected_vertex_count);
            let trim = brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
                .unwrap();
            let cut_edge = &brep.edges()[trim.edge().unwrap()];
            assert_eq!(cut_edge.curve().degree(), 2);
            let domain = cut_edge.curve().domain();
            assert!(*domain.start() >= 0.0 && *domain.end() <= 1.0);
            assert!(*domain.start() > 0.0 || *domain.end() < 1.0);
            let endpoints = [
                cut_edge.curve().evaluate(*domain.start()).unwrap(),
                cut_edge.curve().evaluate(*domain.end()).unwrap(),
            ];
            for (point, (coordinate, expected)) in endpoints.into_iter().zip(boundary_coordinates) {
                assert!(
                    document
                        .tolerance()
                        .approx_eq(point.to_array()[coordinate], expected)
                );
            }
            assert_eq!(trim.curve().degree(), 2);
            let trim_controls = trim.curve().control_points();
            assert_eq!(trim_controls[0].weight(), 1.0);
            assert_eq!(trim_controls[trim_controls.len() - 1].weight(), 1.0);
            cut_domains.push(domain);
            trim_directions.push(trim.is_reversed_3d());
            area += brep.area(document.tolerance()).unwrap();
            brep.tessellate(3, document.tolerance()).unwrap();
        }
        assert_eq!(cut_domains[0], cut_domains[1]);
        trim_directions.sort_unstable();
        assert_eq!(trim_directions, vec![false, true]);
        assert!(document.tolerance().approx_eq(area, 24.0));
    }
}

#[test]
fn cutting_object_split_clips_nonmonotone_cubic_control_polygons() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = planar_intersection_surface();
    let source_id = document
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face_with_orientation(
                source.clone(),
                0.0..=0.4,
                0.0..=0.6,
                true,
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let cutter = NurbsCurve::try_new(
        3,
        [[0.0, 2.0], [2.0, 1.0], [2.0, 5.0], [6.0, 4.0]]
            .into_iter()
            .map(|point| Point3::try_new(point[0], point[1], 0.0).unwrap())
            .collect(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
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
            .execute(&mut document, "Split CuttingObjects=1,5,0")
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut area = 0.0;
    let mut edge_domains = Vec::new();
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("nonmonotone-control cutting Split must create B-rep faces")
        };
        assert!(brep.faces()[0].is_reversed());
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        let edge = &brep.edges()[trim.edge().unwrap()];
        assert_eq!(edge.curve().degree(), 3);
        assert_eq!(*edge.curve().domain().start(), 0.0);
        assert!(*edge.curve().domain().end() < 1.0);
        assert!(
            document.tolerance().approx_eq(
                edge.curve()
                    .evaluate(*edge.curve().domain().end())
                    .unwrap()
                    .x(),
                4.0,
            )
        );
        edge_domains.push(edge.curve().domain());
        area += brep.area(document.tolerance()).unwrap();
        brep.tessellate(3, document.tolerance()).unwrap();
    }
    assert_eq!(edge_domains[0], edge_domains[1]);
    assert!(document.tolerance().approx_eq(area, 24.0));
}

#[test]
fn cutting_object_split_uses_every_in_bounds_interval_of_one_cutter() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = planar_intersection_surface();
    let source_id = document
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face(source, 0.0..=0.4, 0.0..=0.6, document.tolerance())
                .unwrap(),
        ))
        .unwrap();
    let cutter = NurbsCurve::try_new(
        1,
        [[0.0, 2.0], [8.0, 2.0], [8.0, 5.0], [0.0, 5.0]]
            .into_iter()
            .map(|point| Point3::try_new(point[0], point[1], 0.0).unwrap())
            .collect(),
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
    )
    .unwrap();
    let cutter_id = document.add_geometry(Geometry::NurbsCurve(cutter)).unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=2,3.5,0")
            .unwrap(),
        "Split the selected surface at 2 complete isoparametric intersection(s) into 3 exact B-rep face(s)"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 3);
    let mut bounds = outputs
        .iter()
        .map(|object| {
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("multi-interval cutting Split must create B-rep faces")
            };
            brep.tessellate(2, document.tolerance()).unwrap();
            brep.faces()[0]
                .rectangular_trim_bounds(document.tolerance())
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    bounds.sort_by(|left, right| left[1][0].total_cmp(&right[1][0]));
    assert_eq!(
        bounds,
        vec![
            [[0.0, 0.4], [0.0, 0.2]],
            [[0.0, 0.4], [0.2, 0.5]],
            [[0.0, 0.4], [0.5, 0.6]],
        ]
    );
    let north = outputs
        .iter()
        .find_map(|object| {
            let Geometry::Brep(brep) = object.geometry() else {
                return None;
            };
            let bounds = brep.faces()[0]
                .rectangular_trim_bounds(document.tolerance())
                .ok()
                .flatten()?;
            document
                .tolerance()
                .approx_eq(bounds[1][0], 0.5)
                .then_some(brep)
        })
        .unwrap();
    assert_eq!(
        north.faces()[0].loops()[0]
            .trims()
            .iter()
            .map(|trim| (trim.iso(), trim.is_reversed_3d()))
            .collect::<Vec<_>>(),
        vec![
            (viboceros_geometry::SurfaceIso::InteriorUConstant, false),
            (viboceros_geometry::SurfaceIso::InteriorVConstant, false),
            (viboceros_geometry::SurfaceIso::West, false),
            (viboceros_geometry::SurfaceIso::InteriorVConstant, true),
        ]
    );
}

#[test]
fn cutting_object_split_clips_multispan_rational_curve_exactly() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source = planar_intersection_surface();
    let source_id = document
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face_with_orientation(
                source.clone(),
                0.2..=0.8,
                0.0..=1.0,
                false,
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let cutter = NurbsCurve::try_new_rational(
        2,
        [
            ([0.0, 2.0], 1.0),
            ([2.0, 3.0], 0.8),
            ([5.0, 6.0], 1.2),
            ([8.0, 7.5], 0.9),
            ([10.0, 8.0], 1.0),
        ]
        .into_iter()
        .map(|(point, weight)| {
            WeightedPoint3::try_new(Point3::try_new(point[0], point[1], 0.0).unwrap(), weight)
                .unwrap()
        })
        .collect(),
        vec![2.0, 2.0, 2.0, 3.0, 5.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let cutter_id = document.add_geometry(Geometry::NurbsCurve(cutter)).unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Split CuttingObjects=3,1,0")
            .unwrap(),
        "Split the selected surface along 1 complete boundary-to-boundary curve into 2 exact B-rep faces"
    );
    assert!(document.object(source_id).is_none());
    assert!(!document.is_selected(cutter_id));
    let outputs = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    let mut area = 0.0;
    let mut edge_domains = Vec::new();
    for object in outputs {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("multispan cutting Split must create B-rep faces")
        };
        let trim = brep.faces()[0].loops()[0]
            .trims()
            .iter()
            .find(|trim| trim.iso() == viboceros_geometry::SurfaceIso::NotIso)
            .unwrap();
        let edge = &brep.edges()[trim.edge().unwrap()];
        let domain = edge.curve().domain();
        assert!(*domain.start() > 2.0 && *domain.end() < 6.0);
        assert!(edge.curve().spans().count() >= 2);
        assert!(trim.curve().spans().count() >= 2);
        let controls = trim.curve().control_points();
        assert_eq!(controls[0].weight(), 1.0);
        assert_eq!(controls[controls.len() - 1].weight(), 1.0);
        edge_domains.push(domain);
        area += brep.area(document.tolerance()).unwrap();
        brep.tessellate(3, document.tolerance()).unwrap();
    }
    assert_eq!(edge_domains[0], edge_domains[1]);
    assert!(document.tolerance().approx_eq(area, 60.0));
}

#[test]
fn cutting_object_split_rejects_folded_curved_clipping_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let source_id = document
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face(
                planar_intersection_surface(),
                0.0..=0.4,
                0.0..=0.6,
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let cutter_id = document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                3,
                [[0.0, 2.0], [4.0, 6.0], [0.0, 6.0], [4.0, 2.0]]
                    .into_iter()
                    .map(|point| Point3::try_new(point[0], point[1], 0.0).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([source_id, cutter_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();

    let result = registry.execute(&mut document, "Split CuttingObjects=1,1,0");
    assert!(
        matches!(
            &result,
            Err(CommandError::UnsupportedSurfaceCuttingIntersection)
        ),
        "unexpected folded-curve Split result: {result:?}"
    );
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert!(document.is_selected(source_id));
    assert!(document.is_selected(cutter_id));
}
