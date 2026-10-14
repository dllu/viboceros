use super::*;
#[path = "project_infinite_tests.rs"]
mod infinite;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(10., 0., 0.),
        point(10., 10., 0.),
        point(0., 10., 0.),
    ])
    .unwrap()
}
fn setup(source: Geometry, target: Geometry) -> (Document, CommandRegistry, ObjectId, ObjectId) {
    let mut d = Document::default();
    let a = d.add_geometry(source).unwrap();
    let b = d.add_geometry(target).unwrap();
    d.clear_history().unwrap();
    d.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    (d, CommandRegistry::with_builtins(), a, b)
}
fn line(a: Point3, b: Point3) -> Geometry {
    Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}
fn outputs(d: &Document) -> Vec<Geometry> {
    d.selected_objects().map(|o| o.geometry().clone()).collect()
}
fn curve_ends(g: &Geometry) -> [Point3; 2] {
    let c = g.nurbs_curve_representation().unwrap().unwrap();
    [
        c.evaluate(*c.domain().start()).unwrap(),
        c.evaluate(*c.domain().end()).unwrap(),
    ]
}
#[test]
fn project_clips_to_finite_target_and_undoes_as_one_edit() {
    let (mut d, r, a, b) = setup(
        line(point(-5., 5., 7.), point(15., 5., 7.)),
        Geometry::NurbsSurface(plane()),
    );
    r.execute(&mut d, "Project DeleteInput=Yes").unwrap();
    assert!(d.object(a).is_none());
    assert!(d.object(b).is_some());
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let mut ends = curve_ends(&out[0]);
    ends.sort_by(|a, b| a.x().total_cmp(&b.x()));
    assert!(ends[0].distance_to(point(0., 5., 0.)).unwrap() < 1e-8);
    assert!(ends[1].distance_to(point(10., 5., 0.)).unwrap() < 1e-8);
    assert_eq!(d.undo_label(), Some("Project"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
    assert!(d.is_selected(a));
    assert!(!d.is_selected(b));
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.selected_object_count(), 1);
    assert!(d.object(a).is_none());
}
#[test]
fn project_custom_direction_is_bidirectional_and_uses_input_layer() {
    for z in [-3., 3.] {
        let (mut d, r, a, b) = setup(
            Geometry::Point(point(5., 2., z)),
            Geometry::NurbsSurface(plane()),
        );
        let layer = d.add_layer("input", ColorRgb::BLACK).unwrap();
        d.set_objects_layer([a], layer).unwrap();
        r.execute(
            &mut d,
            &format!("Project Sources={a} Targets={b} Direction=1,0,1 OutputLayer=Input"),
        )
        .unwrap();
        let o = d.selected_objects().next().unwrap();
        assert_eq!(o.attributes().layer_id(), layer);
        let Geometry::Point(p) = o.geometry() else {
            panic!()
        };
        assert!(p.distance_to(point(5. - z, 2., 0.)).unwrap() < 1e-8);
    }
}
#[test]
fn project_miss_and_invalid_later_target_preserve_history_selection_and_inputs() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(20., 20., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    let bad = d.add_geometry(Geometry::Point(point(1., 1., 1.))).unwrap();
    let before = d.clone();
    assert!(
        r.execute(&mut d, "Project DeleteInput=Yes")
            .unwrap()
            .contains("missed")
    );
    assert_eq!(format!("{d:?}"), format!("{before:?}"));
    for option in [
        "Direction=0,0,0",
        "DeleteInput=Maybe",
        "Direction=View",
        "Loose=Yes",
    ] {
        assert!(r.execute(&mut d, &format!("Project {option}")).is_err());
        assert_eq!(format!("{d:?}"), format!("{before:?}"));
    }
    assert!(
        r.execute(
            &mut d,
            &format!("Project Sources={a} Targets={b},{bad} DeleteInput=Yes")
        )
        .is_err()
    );
    assert_eq!(format!("{d:?}"), format!("{before:?}"));
}
#[test]
fn project_view_direction_requires_camera_data_and_cplane_uses_context() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(2., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    r.execute(&mut d, "Project Direction=View ViewDirection=0,0,-2")
        .unwrap();
    let output = outputs(&d);
    assert_eq!(output.len(), 1);
    let Geometry::Point(p) = output[0] else {
        panic!()
    };
    assert!(p.distance_to(point(2., 5., 0.)).unwrap() < 1e-8);
    r.execute(&mut d, "Undo").unwrap();
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., -1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    r.execute_in_context(
        &mut d,
        &format!("Project Sources={a} Targets={b}"),
        CommandContext {
            construction_plane: frame,
        },
    )
    .unwrap();
    let Geometry::Point(p) = outputs(&d)[0] else {
        panic!()
    };
    assert!(p.distance_to(point(2., 2., 0.)).unwrap() < 1e-8);
}
#[test]
fn project_respects_hole_and_target_output_layer() {
    let outer = Polyline3::try_new(
        vec![
            point(0., 0., 0.),
            point(10., 0., 0.),
            point(10., 10., 0.),
            point(0., 10., 0.),
            point(0., 0., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let hole = Polyline3::try_new(
        vec![
            point(4., 4., 0.),
            point(6., 4., 0.),
            point(6., 6., 0.),
            point(4., 6., 0.),
            point(4., 4., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let face = Brep::try_planar_face_with_holes(
        &outer.to_nurbs().unwrap(),
        &[hole.to_nurbs().unwrap()],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut d, r, _, b) = setup(
        line(point(-2., 5., 3.), point(12., 5., 3.)),
        Geometry::Brep(face),
    );
    let layer = d.add_layer("target", ColorRgb::BLACK).unwrap();
    d.set_objects_layer([b], layer).unwrap();
    r.execute(&mut d, "Project OutputLayer=TargetObject")
        .unwrap();
    assert_eq!(d.selected_object_count(), 2);
    for o in d.selected_objects() {
        assert_eq!(o.attributes().layer_id(), layer);
        let c = o.geometry().nurbs_curve_representation().unwrap().unwrap();
        for i in 0..=32 {
            let p = c
                .parameter_sampler()
                .unwrap()
                .evaluate(i as f64 / 32.)
                .unwrap();
            assert!(p.x() <= 4. + 1e-8 || p.x() >= 6. - 1e-8);
            assert!(p.z().abs() < 1e-8);
        }
    }
}
#[cfg(feature = "native-smlib")]
#[test]
fn project_native_curved_graph_preserves_direction_and_surface_relation() {
    let target = NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(10., 0., 0.),
        point(10., 10., 4.),
        point(0., 10., 0.),
    ])
    .unwrap();
    let (mut d, r, _, _) = setup(
        line(point(1., 2., 7.), point(9., 8., 7.)),
        Geometry::NurbsSurface(target),
    );
    r.execute(&mut d, "Project").unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let c = out[0].nurbs_curve_representation().unwrap().unwrap();
    for i in 0..=128 {
        let p = c
            .parameter_sampler()
            .unwrap()
            .evaluate(i as f64 / 128.)
            .unwrap();
        assert!((p.z() - 0.04 * p.x() * p.y()).abs() < d.tolerance().absolute());
        assert!((p.y() - (2. + (p.x() - 1.) * 0.75)).abs() < d.tolerance().absolute());
        assert!(p.x() >= 1. - 1e-8 && p.x() <= 9. + 1e-8);
    }
}

#[test]
fn project_polylines_remain_connected_on_planar_targets() {
    let c = Polyline3::try_new(
        vec![point(1., 2., 3.), point(5., 7., 3.), point(9., 2., 3.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut d, r, _, _) = setup(Geometry::Polyline(c), Geometry::NurbsSurface(plane()));
    r.execute(&mut d, "Project").unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let c = out[0].nurbs_curve_representation().unwrap().unwrap();
    assert_eq!(c.degree(), 1);
    assert_eq!(c.control_points().len(), 3);
    assert!(
        c.control_points()
            .iter()
            .all(|p| p.point().z().abs() < 1e-8)
    );
}
#[test]
fn project_points_keep_multiple_solid_hits() {
    let frame = CommandContext::default().construction_plane;
    let b = Brep::try_box(frame, [[0., 10.], [0., 10.], [0., 10.]], Tolerance::DEFAULT).unwrap();
    let (mut d, r, _, _) = setup(Geometry::Point(point(5., 5., 20.)), Geometry::Brep(b));
    r.execute(&mut d, "Project").unwrap();
    let mut z = outputs(&d)
        .into_iter()
        .map(|g| {
            if let Geometry::Point(p) = g {
                p.z()
            } else {
                panic!()
            }
        })
        .collect::<Vec<_>>();
    z.sort_by(f64::total_cmp);
    assert_eq!(z.len(), 2);
    assert!(z[0].abs() < 1e-8);
    assert!((z[1] - 10.).abs() < 1e-8);
}
#[cfg(feature = "native-smlib")]
#[test]
fn project_native_curved_partial_face_clips_in_original_uv() {
    let surface = NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(10., 0., 0.),
        point(10., 10., 4.),
        point(0., 10., 0.),
    ])
    .unwrap();
    let mid = 0.5 * (surface.domain_u().start() + surface.domain_u().end());
    let [_, upper] = Brep::try_split_rectangular_surface_face_u(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        mid,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut d, r, _, _) = setup(
        line(point(1., 2., 7.), point(9., 8., 7.)),
        Geometry::Brep(upper),
    );
    r.execute(&mut d, "Project").unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let c = out[0].nurbs_curve_representation().unwrap().unwrap();
    for i in 0..=128 {
        let p = c
            .parameter_sampler()
            .unwrap()
            .evaluate(i as f64 / 128.)
            .unwrap();
        assert!(p.x() >= 5. - 1e-7);
        assert!((p.z() - 0.04 * p.x() * p.y()).abs() < d.tolerance().absolute());
    }
    let mut ends = curve_ends(&out[0]);
    ends.sort_by(|a, b| a.x().total_cmp(&b.x()));
    assert!((ends[0].x() - 5.).abs() < 1e-6);
    assert!((ends[1].x() - 9.).abs() < 1e-6);
}
#[test]
fn project_rejects_mixed_weight_extent_bounds_atomically() {
    use viboceros_geometry::WeightedPoint3;
    let c = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1., 1., 3.), 1.).unwrap(),
            WeightedPoint3::try_new(point(5., 5., 3.), -0.1).unwrap(),
            WeightedPoint3::try_new(point(9., 1., 3.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let (mut d, r, _, _) = setup(Geometry::NurbsCurve(c), Geometry::NurbsSurface(plane()));
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "Project DeleteInput=Yes").is_err());
    assert_eq!(before, format!("{d:?}"));
}

#[test]
fn project_collapsed_curve_and_parallel_miss_leave_document_unchanged() {
    let (mut d, r, _, _) = setup(
        line(point(5., 5., 2.), point(5., 5., 7.)),
        Geometry::NurbsSurface(plane()),
    );
    let before = format!("{d:?}");
    assert!(r.execute(&mut d, "Project DeleteInput=Yes").is_err());
    assert_eq!(before, format!("{d:?}"));
    assert!(
        r.execute(&mut d, "Project Direction=1,0,0 DeleteInput=Yes")
            .unwrap()
            .contains("missed")
    );
    assert_eq!(before, format!("{d:?}"));
}

#[test]
fn project_nearly_parallel_direction_keeps_finite_planar_image() {
    let (mut d, r, _, _) = setup(
        line(point(1., 5., 1e-9), point(2., 5., 1e-9)),
        Geometry::NurbsSurface(plane()),
    );
    r.execute(&mut d, "Project Direction=1,0,1e-9").unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let mut ends = curve_ends(&out[0]);
    ends.sort_by(|a, b| a.x().total_cmp(&b.x()));
    assert!(ends[0].distance_to(point(0., 5., 0.)).unwrap() < 1e-7);
    assert!(ends[1].distance_to(point(1., 5., 0.)).unwrap() < 1e-7);
}

fn mesh_target(corners: [Point3; 4]) -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new_faces(
            corners.to_vec(),
            vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}
#[test]
fn project_mesh_points_are_bidirectional_and_shared_faces_deduplicate() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(0.5, 0.5, 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    r.execute(&mut d, "Project DeleteInput=Yes").unwrap();
    assert!(d.object(a).is_none());
    assert!(d.object(b).is_some());
    assert_eq!(outputs(&d), [Geometry::Point(point(0.5, 0.5, 0.))]);
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.selected_object_count(), 1);
}
#[test]
fn project_mesh_line_clips_and_stitches_a_shared_triangle_crossing() {
    let (mut d, r, _, _) = setup(
        line(point(-1., 0.25, 3.), point(2., 0.25, 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    r.execute(&mut d, "Project").unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1, "{out:?}");
    let mut ends = curve_ends(&out[0]);
    ends.sort_by(|a, b| a.x().total_cmp(&b.x()));
    assert!(ends[0].distance_to(point(0., 0.25, 0.)).unwrap() < 1e-8);
    assert!(ends[1].distance_to(point(1., 0.25, 0.)).unwrap() < 1e-8);
}
#[test]
fn project_mesh_warped_quad_uses_triangles_and_not_a_bilinear_patch() {
    let (mut d, r, _, _) = setup(
        Geometry::Point(point(0.25, 0.75, 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 1.),
            point(0., 1., 0.),
        ]),
    );
    r.execute(&mut d, "Project").unwrap();
    assert_eq!(outputs(&d), [Geometry::Point(point(0.25, 0.75, 0.))]);
    let (mut d, r, _, _) = setup(
        line(point(0.25, 0., 3.), point(0.25, 1., 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 1.),
            point(0., 1., 0.),
        ]),
    );
    r.execute(&mut d, "Project").unwrap();
    for g in outputs(&d) {
        let c = g.nurbs_curve_representation().unwrap().unwrap();
        for i in 0..=64 {
            let p = c
                .parameter_sampler()
                .unwrap()
                .evaluate(i as f64 / 64.)
                .unwrap();
            assert!((p.z() - (p.x() + p.y() - 1.).max(0.)).abs() < 1e-8);
        }
    }
}
#[test]
fn project_mesh_rational_curve_keeps_exact_planar_source_image() {
    use viboceros_geometry::WeightedPoint3;
    let c = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0.1, 0.1, 3.), 1.).unwrap(),
            WeightedPoint3::try_new(point(0.5, 0.9, 3.), 0.75).unwrap(),
            WeightedPoint3::try_new(point(0.9, 0.1, 3.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let (mut d, r, _, _) = setup(
        Geometry::NurbsCurve(c.clone()),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    r.execute(&mut d, "Project").unwrap();
    assert!(d.selected_object_count() > 0);
    for g in outputs(&d) {
        let out = g.nurbs_curve_representation().unwrap().unwrap();
        for i in 0..=64 {
            let p = out
                .parameter_sampler()
                .unwrap()
                .evaluate(i as f64 / 64.)
                .unwrap();
            let lifted = point(p.x(), p.y(), 3.);
            let parameter = c.closest_parameter(lifted, Tolerance::DEFAULT).unwrap();
            assert!(c.evaluate(parameter).unwrap().distance_to(lifted).unwrap() < 1e-8);
            assert!(p.z().abs() < 1e-8);
        }
    }
}
#[test]
fn project_mesh_miss_and_ambiguous_later_target_preserve_redo_and_inputs() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(2., 2., 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    d.add_geometry(Geometry::Point(point(9., 9., 9.))).unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "Project DeleteInput=Yes")
            .unwrap()
            .contains("missed")
    );
    assert_eq!(before, format!("{d:?}"));
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(0.5, 0.5, 0.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    let valid = d
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(0., 0., 0.),
                point(0., 1., 0.),
                point(0., 1., 1.),
                point(0., 0., 1.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(
            &mut d,
            &format!("Project Sources={a} Targets={valid},{b} Direction=1,0,0 DeleteInput=Yes")
        )
        .is_err()
    );
    assert_eq!(before, format!("{d:?}"));
}

#[test]
fn project_mesh_curve_vertex_touch_produces_no_zero_dimensional_object() {
    let (mut d, r, _, _) = setup(
        line(point(-1., 1., 3.), point(1., -1., 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, "Project DeleteInput=Yes")
            .unwrap()
            .contains("missed")
    );
    assert_eq!(before, format!("{d:?}"));
}
#[test]
fn project_mesh_shared_diagonal_does_not_duplicate_the_same_curve() {
    let (mut d, r, _, _) = setup(
        line(point(0.1, 0.1, 3.), point(0.9, 0.9, 3.)),
        mesh_target([
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 1., 0.),
            point(0., 1., 0.),
        ]),
    );
    r.execute(&mut d, "Project").unwrap();
    assert_eq!(d.selected_object_count(), 1);
}

#[test]
fn project_two_point_custom_direction_matches_world_vector_and_handles_overflow() {
    let (mut d, r, a, b) = setup(
        Geometry::Point(point(5., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    r.execute(&mut d,&format!("Project Sources={a} Targets={b} Direction=Custom DirectionStart=9,7,5 DirectionEnd=10,7,6")).unwrap();
    let Geometry::Point(p) = outputs(&d)[0] else {
        panic!()
    };
    assert!(p.distance_to(point(2., 5., 0.)).unwrap() < 1e-8);
    let (mut d, r, _, _) = setup(
        Geometry::Point(point(2., 3., 4.)),
        Geometry::NurbsSurface(plane()),
    );
    r.execute(
        &mut d,
        "Project _Direction=_Custom DirectionStart=-1e308,0,1e308 DirectionEnd=1e308,1e308,-1e308",
    )
    .unwrap();
    let Geometry::Point(p) = outputs(&d)[0] else {
        panic!()
    };
    assert!(p.distance_to(point(6., 5., 0.)).unwrap() < 1e-8);
}
#[test]
fn project_custom_point_validation_preserves_document_and_redo() {
    let (mut d, r, _, _) = setup(
        Geometry::Point(point(5., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    d.add_geometry(Geometry::Point(point(99., 99., 99.)))
        .unwrap();
    r.execute(&mut d, "Undo").unwrap();
    let before = format!("{d:?}");
    for options in [
        "Direction=Custom",
        "Direction=Custom DirectionStart=0,0,0",
        "Direction=Custom DirectionStart=0,0,0 DirectionEnd=0,0,0",
        "DirectionStart=0,0,0 DirectionEnd=1,1,1",
        "Direction=View DirectionStart=0,0,0 DirectionEnd=1,1,1 ViewDirection=0,0,1",
        "Direction=Custom DirectionStart=0,0,0 DirectionStart=1,1,1 DirectionEnd=2,2,2",
    ] {
        assert!(
            r.execute(&mut d, &format!("Project {options} DeleteInput=Yes"))
                .is_err()
        );
        assert_eq!(before, format!("{d:?}"));
    }
}

#[test]
fn project_temporary_range_preserves_source_attributes_and_undo_redo() {
    let (mut d, r, a, b) = setup(
        line(point(1., 5., 3.), point(9., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    let layer = d.add_layer("range-input", ColorRgb::BLACK).unwrap();
    d.set_objects_layer([a], layer).unwrap();
    d.select_objects_direct([b], SelectionMode::Replace)
        .unwrap();
    let original = d.object(a).unwrap().clone();
    r.execute(
        &mut d,
        &format!("Project Targets={b} SubCrv={a},2,6 DeleteInput=Yes OutputLayer=Input"),
    )
    .unwrap();
    assert_eq!(d.object(a), Some(&original));
    assert_eq!(d.objects().len(), 3);
    let out = d.selected_objects().next().unwrap();
    assert_eq!(out.attributes().layer_id(), layer);
    assert_eq!(
        curve_ends(out.geometry()),
        [point(3., 5., 0.), point(7., 5., 0.)]
    );
    assert_eq!(d.undo_label(), Some("Project"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().len(), 2);
    assert_eq!(d.object(a), Some(&original));
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.selected_object_count(), 1);
    assert_eq!(d.object(a), Some(&original));
}

#[test]
fn project_repeated_ranges_signed_lengths_and_whole_points_share_atomic_admission() {
    let (mut d, r, a, b) = setup(
        line(point(1., 5., 3.), point(9., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    let p = d.add_geometry(Geometry::Point(point(5., 8., 2.))).unwrap();
    r.execute(
        &mut d,
        &format!(
            "Project Sources={p} Targets={b} SubCrv={a},2,4 SubCrvLength={a},6,-2 DeleteInput=Yes"
        ),
    )
    .unwrap();
    assert!(d.object(p).is_none());
    assert!(d.object(a).is_some());
    assert_eq!(d.selected_object_count(), 3);
    let out = outputs(&d);
    assert_eq!(out[0], Geometry::Point(point(5., 8., 0.)));
    assert_eq!(curve_ends(&out[1]), [point(3., 5., 0.), point(5., 5., 0.)]);
    assert_eq!(curve_ends(&out[2]), [point(7., 5., 0.), point(5., 5., 0.)]);
    r.execute(&mut d, "Undo").unwrap();
    assert!(d.object(p).is_some());
    assert_eq!(d.objects().len(), 3);
}

#[test]
fn project_closed_subcurve_wraps_the_seam_and_preserves_its_kink() {
    let polyline = Polyline3::try_new(
        vec![
            point(2., 2., 3.),
            point(4., 2., 3.),
            point(4., 4., 3.),
            point(2., 4., 3.),
            point(2., 2., 3.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut d, r, a, b) = setup(
        Geometry::Polyline(polyline),
        Geometry::NurbsSurface(plane()),
    );
    d.select_objects_direct([b], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, &format!("Project SubCrv={a},3.5,0.5"))
        .unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let curve = out[0].curve_ref().unwrap();
    assert_eq!(curve.start_point().unwrap(), point(2., 3., 0.));
    assert_eq!(curve.end_point().unwrap(), point(3., 2., 0.));
    assert!((curve.length(d.tolerance()).unwrap() - 2.).abs() < 1e-8);
    assert!(
        out[0]
            .nurbs_curve_representation()
            .unwrap()
            .unwrap()
            .control_points()
            .iter()
            .any(|p| p.point() == point(2., 2., 0.))
    );
}

#[test]
fn project_rational_subcurve_uses_the_existing_mesh_producer() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(2., 2., 3.), 1.).unwrap(),
            WeightedPoint3::try_new(point(5., 8., 3.), 2.).unwrap(),
            WeightedPoint3::try_new(point(8., 2., 3.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let (mut d, r, a, b) = setup(
        Geometry::NurbsCurve(curve),
        mesh_target([
            point(0., 0., 0.),
            point(10., 0., 0.),
            point(10., 10., 0.),
            point(0., 10., 0.),
        ]),
    );
    d.select_objects_direct([b], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, &format!("Project _SubCrv={a},0.2,0.7"))
        .unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let c = out[0].nurbs_curve_representation().unwrap().unwrap();
    assert_eq!(c.degree(), 2);
    assert!(c.is_rational());
    for i in 0..=32 {
        let t = 0.2 + 0.5 * i as f64 / 32.;
        let p = c.evaluate(t).unwrap();
        let a = (1. - t).powi(2);
        let b = 4. * t * (1. - t);
        let e = t * t;
        let expected = point(
            (2. * a + 5. * b + 8. * e) / (a + b + e),
            (2. * a + 8. * b + 2. * e) / (a + b + e),
            0.,
        );
        assert!(p.distance_to(expected).unwrap() < 1e-8);
    }
}

#[test]
fn project_invalid_later_subcurve_is_read_only_and_preserves_redo() {
    let (mut d, r, a, b) = setup(
        line(point(1., 5., 3.), point(9., 5., 3.)),
        Geometry::NurbsSurface(plane()),
    );
    let p = d
        .add_geometry(Geometry::Point(point(99., 99., 99.)))
        .unwrap();
    r.execute(&mut d, "Undo").unwrap();
    let before = format!("{d:?}");
    for spec in [
        format!("SubCrv={a},0.5,0.5"),
        format!("SubCrv={a},-1,0.5"),
        format!("SubCrv={a},NaN,0.5"),
        format!("SubCrvLength={a},0.5,99"),
        format!("SubCrv={p},0,1"),
        format!("SubCrv={b},0,1"),
    ] {
        assert!(
            r.execute(
                &mut d,
                &format!("Project SubCrv={a},0.2,0.7 {spec} DeleteInput=Yes")
            )
            .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
    }
    d.set_objects_locked([a], true).unwrap();
    let before = format!("{d:?}");
    assert!(
        r.execute(&mut d, &format!("Project SubCrv={a},0,1"))
            .is_err()
    );
    assert_eq!(format!("{d:?}"), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn project_native_subcurve_clips_the_source_before_curved_surface_projection() {
    let s = NurbsSurface::try_bilinear([
        point(0., 0., 0.),
        point(10., 0., 0.),
        point(10., 10., 4.),
        point(0., 10., 0.),
    ])
    .unwrap();
    let (mut d, r, a, b) = setup(
        line(point(1., 2., 7.), point(9., 8., 7.)),
        Geometry::NurbsSurface(s),
    );
    d.select_objects_direct([b], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, &format!("Project SubCrv={a},2.5,7.5"))
        .unwrap();
    let out = outputs(&d);
    assert_eq!(out.len(), 1);
    let c = out[0].nurbs_curve_representation().unwrap().unwrap();
    for i in 0..=64 {
        let p = c
            .parameter_sampler()
            .unwrap()
            .evaluate(i as f64 / 64.)
            .unwrap();
        assert!((p.z() - 0.04 * p.x() * p.y()).abs() < d.tolerance().absolute());
        assert!((p.y() - (2. + (p.x() - 1.) * 0.75)).abs() < d.tolerance().absolute());
        assert!(p.x() >= 3. - 1e-8 && p.x() <= 7. + 1e-8);
    }
}
