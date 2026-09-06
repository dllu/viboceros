use super::*;
use crate::construction_plane::WorldPlane;

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}

fn cloud(points: &[[f64; 3]]) -> Geometry {
    Geometry::PointCloud(
        viboceros_geometry::PointCloud3::try_new(points.iter().copied().map(point).collect())
            .unwrap(),
    )
}

fn selected(geometry: Geometry) -> Document {
    let mut document = Document::default();
    document.add_geometry(geometry).unwrap();
    document.select_all();
    document
}

fn near(actual: Point3, expected: Point3) {
    assert!(
        actual.distance_to(expected).unwrap() < 1e-8,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn all_plane_orientations_create_actual_world_corners_and_ignore_distant_plane_origins() {
    let oblique = Frame3::try_from_directions(
        point([3., 4., 5.]),
        Vector3::try_new(1., 2., 3.).unwrap(),
        Vector3::try_new(-4., 8., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    for axes in WorldPlane::ALL
        .map(WorldPlane::frame)
        .into_iter()
        .chain([oblique])
    {
        let placement = axes.with_origin(point([3., 4., 5.]));
        let a = placement.point_at([1., 2., 3.]).unwrap();
        let b = placement.point_at([4., 6., 9.]).unwrap();
        let expected = bounding_box_corners(
            BoundingBox3::from_points([point([1., 2., 3.]), point([4., 6., 9.])]).unwrap(),
        )
        .unwrap()
        .map(|p| placement.point_at(p.to_array()).unwrap());
        for origin in [[0.; 3], [10., 20., 30.], [1e100, -1e100, 1e100]] {
            for output in ["Solids", "Meshes", "Curves"] {
                let mut document = selected(cloud(&[a.to_array(), b.to_array()]));
                let source = document.objects().next().unwrap().clone();
                CommandRegistry::with_builtins()
                    .execute_in_context(
                        &mut document,
                        &format!("BoundingBox _CoordinateSystem _CPlane _Output _{output}"),
                        CommandContext {
                            construction_plane: axes.with_origin(point(origin)),
                        },
                    )
                    .unwrap();
                for object in document.objects().skip(1) {
                    let points = match object.geometry() {
                        Geometry::Brep(b) => {
                            b.vertices().iter().map(|v| v.point()).collect::<Vec<_>>()
                        }
                        Geometry::Mesh(m) => m.vertices().to_vec(),
                        Geometry::Polyline(p) => p.vertices().to_vec(),
                        _ => panic!("unexpected enclosure"),
                    };
                    for p in points {
                        assert!(
                            expected.iter().any(|q| p.distance_to(*q).unwrap() < 1e-8),
                            "{p:?}"
                        );
                    }
                }
                assert_eq!(document.object(source.id()).unwrap(), &source);
                assert!(document.is_selected(source.id()));
                document.undo().unwrap();
                assert_eq!(document.objects().len(), 1);
                assert_eq!(document.groups().len(), 0);
                document.redo().unwrap();
                assert_eq!(
                    document.objects().len(),
                    if output == "Curves" { 7 } else { 2 }
                );
            }
        }
    }
}

#[test]
fn cplane_reports_local_coordinates_and_world_option_ignores_context() {
    let geometry = cloud(&[[12., 17., 34.], [15., 11., 38.]]);
    let context = CommandContext {
        construction_plane: WorldPlane::Front
            .frame()
            .with_origin(point([10., 20., 30.])),
    };
    for (system, expected) in [
        (
            "CPlane",
            "min 2.000000,4.000000,3.000000 max 5.000000,8.000000,9.000000 size 3.000000,4.000000,6.000000",
        ),
        (
            "World",
            "min 12.000000,11.000000,34.000000 max 15.000000,17.000000,38.000000 size 3.000000,6.000000,4.000000",
        ),
    ] {
        let mut document = selected(geometry.clone());
        let history = document.undo_label().map(str::to_owned);
        let report = CommandRegistry::with_builtins()
            .execute_in_context(
                &mut document,
                &format!("BBox CoordinateSystem={system} Output=None"),
                context,
            )
            .unwrap();
        assert!(report.contains(expected), "{report}");
        assert_eq!(document.objects().len(), 1);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn quadratic_surface_and_trimmed_face_boxes_use_extrema_not_control_geometry() {
    let surface = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    point([
                        u as f64 / 2.,
                        v as f64 / 2.,
                        [0., 2., 0.][u] + [0., 4., 0.][v],
                    ])
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let trimmed = Brep::try_rectangular_surface_face(
        surface.clone(),
        0.2..=0.8,
        0.25..=0.75,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for (source, min, max) in [
        (Geometry::NurbsSurface(surface), [0., 0., 0.], [1., 1., 3.]),
        (Geometry::Brep(trimmed), [0.2, 0.25, 2.14], [0.8, 0.75, 3.]),
    ] {
        let mut document = selected(source.clone());
        CommandRegistry::with_builtins()
            .execute(&mut document, "BoundingBox")
            .unwrap();
        let Geometry::Brep(b) = document.objects().nth(1).unwrap().geometry() else {
            panic!("box")
        };
        near(b.bounds().min(), point(min));
        near(b.bounds().max(), point(max));
        assert_eq!(document.objects().next().unwrap().geometry(), &source);
    }
}

#[test]
fn signed_curve_enclosure_contains_projective_extrema_and_is_gauge_invariant() {
    let mut previous = None;
    for gauge in [1., -1., 1e-200, 1e200] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [[0., 0., 0.], [1., 10., 0.], [2., 0., 0.]]
                .into_iter()
                .zip([1., -1., 2.])
                .map(|(p, w)| WeightedPoint3::try_new(point(p), w * gauge).unwrap())
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let mut document = selected(Geometry::NurbsCurve(curve.clone()));
        CommandRegistry::with_builtins()
            .execute(&mut document, "BoundingBox")
            .unwrap();
        let Geometry::Polyline(rectangle) = document.objects().nth(1).unwrap().geometry() else {
            panic!("planar rectangle")
        };
        let bounds = rectangle.bounds();
        assert!((bounds.min().y() + 10. * (1. + 2_f64.sqrt())).abs() < 1e-8);
        for i in 0..=1000 {
            let p = curve.evaluate(i as f64 / 1000.).unwrap().to_array();
            for (axis, value) in p.into_iter().enumerate() {
                assert!(
                    value >= bounds.min().to_array()[axis] - 1e-8
                        && value <= bounds.max().to_array()[axis] + 1e-8
                );
            }
        }
        if let Some(old) = previous {
            let old: BoundingBox3 = old;
            near(old.min(), bounds.min());
            near(old.max(), bounds.max());
        }
        previous = Some(bounds);
    }
}

#[test]
fn rank_uses_document_tolerance_without_discarding_resolved_thin_solids() {
    for (thickness, solid) in [(5e-10, false), (1e-8, true)] {
        for output in ["Solids", "Meshes", "Curves", "None"] {
            let mut document = selected(cloud(&[[0.; 3], [3., 4., thickness]]));
            CommandRegistry::with_builtins()
                .execute(&mut document, &format!("BoundingBox Output={output}"))
                .unwrap();
            if output == "None" {
                assert_eq!(document.objects().len(), 1);
            } else if !solid {
                let Geometry::Polyline(p) = document.objects().nth(1).unwrap().geometry() else {
                    panic!("flat rectangle")
                };
                assert!(p.vertices().iter().all(|p| p.z() == 0.));
            } else {
                assert_eq!(
                    document.objects().len(),
                    if output == "Curves" { 7 } else { 2 }
                );
            }
        }
    }
}

#[test]
fn failed_individual_bounds_leave_no_partial_geometry_groups_selection_or_history() {
    let pole = NurbsCurve::try_new_rational(
        2,
        [[0., 0., 0.], [1., 10., 0.], [2., 0., 0.]]
            .into_iter()
            .zip([1., -1., 1.])
            .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    for bad in [
        Geometry::Point(point([1., 2., 3.])),
        Geometry::NurbsCurve(pole),
    ] {
        for reverse in [false, true] {
            for output in ["Solids", "Meshes", "Curves", "None"] {
                let mut document = Document::default();
                let mut sources = [cloud(&[[1., 2., 3.], [4., 6., 9.]]), bad.clone()];
                if reverse {
                    sources.reverse();
                }
                for geometry in sources {
                    document.add_geometry(geometry).unwrap();
                }
                document.select_all();
                let originals = document.objects().cloned().collect::<Vec<_>>();
                let history = document.undo_label().map(str::to_owned);
                assert!(
                    CommandRegistry::with_builtins()
                        .execute(
                            &mut document,
                            &format!("BoundingBox Cumulative=No Output={output}")
                        )
                        .is_err()
                );
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), originals);
                assert_eq!(document.selected_objects().count(), 2);
                assert_eq!(document.groups().len(), 0);
                assert_eq!(document.undo_label(), history.as_deref());
            }
        }
    }
}

#[test]
fn bounding_box_creates_solid_mesh_and_grouped_curve_enclosures_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 1,2,3 2").unwrap();
    registry.execute(&mut document, "Point 5,-2,9").unwrap();
    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Layer New Bounds").unwrap();
    let output_layer = document.current_layer_id();
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();

    let message = registry.execute(&mut document, "BBox").unwrap();
    assert!(message.contains("#1 min -1.000000,-2.000000,3.000000"));
    assert!(message.contains("max 5.000000,4.000000,9.000000"));
    assert_eq!(document.objects().len(), 3);
    let solid = document.objects().last().unwrap();
    assert_eq!(solid.attributes().layer_id(), output_layer);
    assert!(!document.is_selected(solid.id()));
    let Geometry::Brep(solid) = solid.geometry() else {
        panic!("BoundingBox must create a solid B-rep by default")
    };
    assert!(solid.is_solid());
    assert_eq!(
        solid.bounds().min(),
        Point3::try_new(-1.0, -2.0, 3.0).unwrap()
    );
    assert_eq!(
        solid.bounds().max(),
        Point3::try_new(5.0, 4.0, 9.0).unwrap()
    );
    assert!(source_ids.iter().all(|id| document.is_selected(*id)));
    assert_eq!(document.undo_label(), Some("BoundingBox"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    let history = document.undo_label().map(str::to_owned);
    let report = registry
        .execute(&mut document, "BoundingBox Output=None")
        .unwrap();
    assert!(report.contains("size 6.000000,6.000000,6.000000"));
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.undo_label(), history.as_deref());

    registry
        .execute(
            &mut document,
            "BoundingBox CoordinateSystem=CPlane Output=Meshes",
        )
        .unwrap();
    let Geometry::Mesh(mesh) = document.objects().last().unwrap().geometry() else {
        panic!("BoundingBox Output=Meshes must create a mesh")
    };
    assert_eq!(mesh.vertices().len(), 24);
    assert_eq!(mesh.faces().len(), 6);
    assert_eq!(mesh.triangles().len(), 12);
    assert!(mesh.topology().is_solid());
    registry.execute(&mut document, "Undo").unwrap();

    registry
        .execute(&mut document, "BoundingBox Output=Curves")
        .unwrap();
    assert_eq!(document.objects().len(), 8);
    assert_eq!(document.groups().len(), 1);
    let group = document.groups().next().unwrap();
    assert_eq!(group.members().len(), 6);
    assert!(group.members().all(|id| {
        matches!(document.object(id).unwrap().geometry(), Geometry::Polyline(polyline) if polyline.is_closed())
            && !document.is_selected(id)
    }));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.groups().len(), 0);

    for invalid in [
        "BoundingBox Output=SubD",
        "BoundingBox Cumulative=Maybe",
        "BoundingBox CoordinateSystem=Object",
        "BoundingBox Output=Solids Output=Meshes",
        "BoundingBox extra",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn bounding_box_handles_planar_individual_outputs_and_rejects_points() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Rectangle 0,0,2 3,4,2")
        .unwrap();
    registry
        .execute(&mut document, "Rectangle 10,20,7 12,25,7")
        .unwrap();
    let source_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_objects_direct(source_ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "BoundingBox Cumulative=No Output=Meshes CoordinateSystem=World",
        )
        .unwrap();
    let outputs = document.objects().skip(2).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for output in outputs {
        let Geometry::Polyline(rectangle) = output.geometry() else {
            panic!("a planar bounding box must be a rectangle even for Output=Meshes")
        };
        assert_eq!(rectangle.vertices().len(), 5);
        assert!(rectangle.is_closed());
        assert!(!document.is_selected(output.id()));
    }

    let mut point_document = Document::default();
    registry
        .execute(&mut point_document, "Point 1,2,3")
        .unwrap();
    registry.execute(&mut point_document, "SelAll").unwrap();
    let history = point_document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut point_document, "BoundingBox"),
        Err(CommandError::DegenerateBoundingBox)
    ));
    assert_eq!(point_document.objects().len(), 1);
    assert_eq!(point_document.undo_label(), history.as_deref());
    assert!(matches!(
        registry.execute(&mut point_document, "BBox Output=None"),
        Err(CommandError::DegenerateBoundingBox)
    ));
    assert_eq!(point_document.undo_label(), history.as_deref());
}
