use super::*;
fn scene() -> (CommandRegistry, Document, ObjectId) {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::new(Tolerance::DEFAULT);
    r.execute(&mut d, "Circle 0,0,0 2").unwrap();
    let id = d.objects().next().unwrap().id();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    (r, d, id)
}
#[test]
fn curvature_graph_is_readonly_and_scale_color_updates_reuse_cached_differentials() {
    let (r, mut d, id) = scene();
    let before = format!("{d:?}");
    r.execute(&mut d, "CurvatureGraph Scale=100 Density=1")
        .unwrap();
    let a = r.curvature_graph_view(&d).unwrap().unwrap();
    assert_eq!(a.sources, 1);
    for g in a.graphs.iter() {
        for h in &g.comb.hairs {
            assert!((a.tip(h).unwrap().distance_to(h.point).unwrap() - 0.5).abs() < 1e-12);
        }
    }
    r.execute(&mut d, "CurvatureGraph Scale=102 Color=1,2,3")
        .unwrap();
    let b = r.curvature_graph_view(&d).unwrap().unwrap();
    assert!(Arc::ptr_eq(&a.graphs, &b.graphs));
    assert_eq!(b.options.color, [1, 2, 3]);
    assert!(
        (b.tip(&b.graphs[0].comb.hairs[0])
            .unwrap()
            .distance_to(b.graphs[0].comb.hairs[0].point)
            .unwrap()
            - 1.)
            .abs()
            < 1e-12
    );
    assert!(d.is_selected(id));
    assert_eq!(format!("{d:?}"), before);
    r.execute(&mut d, "CurvatureGraphOff").unwrap();
    assert!(r.curvature_graph_view(&d).unwrap().is_none());
    assert_eq!(format!("{d:?}"), before);
}
#[test]
fn curvature_graph_tracks_edits_and_source_sets_and_rejects_bad_options_atomically() {
    let (r, mut d, _) = scene();
    r.execute(&mut d, "CurvatureGraph").unwrap();
    let old = r.curvature_graph_view(&d).unwrap().unwrap();
    for args in [
        "Scale=301",
        "Density=11",
        "Count=1",
        "Scale=NaN",
        "U=Maybe",
        "Color=0,256,0",
        "Add Remove",
        "Scale=1 Scale=2",
    ] {
        assert!(
            r.execute(&mut d, &format!("CurvatureGraph {args}"))
                .is_err()
        );
        assert!(Arc::ptr_eq(
            &old.graphs,
            &r.curvature_graph_view(&d).unwrap().unwrap().graphs
        ));
    }
    r.execute(&mut d, "Move 0,0,0 3,0,0").unwrap();
    let moved = r.curvature_graph_view(&d).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&old.graphs, &moved.graphs));
    d.undo().unwrap();
    let undone = r.curvature_graph_view(&d).unwrap().unwrap();
    assert!(
        undone.graphs[0].comb.hairs[0]
            .point
            .distance_to(old.graphs[0].comb.hairs[0].point)
            .unwrap()
            < 1e-12
    );
    r.execute(&mut d, "CurvatureGraph Remove").unwrap();
    assert_eq!(r.curvature_graph_view(&d).unwrap().unwrap().sources, 0);
    r.execute(&mut d, "CurvatureGraph Add").unwrap();
    assert_eq!(r.curvature_graph_view(&d).unwrap().unwrap().sources, 1);
    r.execute(&mut d, "Delete").unwrap();
    assert_eq!(r.curvature_graph_view(&d).unwrap().unwrap().sources, 0);
}
#[test]
fn curvature_graph_surface_directions_and_primitive_brep_are_supported() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::new(Tolerance::DEFAULT);
    r.execute(&mut d, "Sphere 0,0,0 2").unwrap();
    r.execute(&mut d, "SelAll").unwrap();
    r.execute(&mut d, "CurvatureGraph Count=3 U=Yes V=No")
        .unwrap();
    let v = r.curvature_graph_view(&d).unwrap().unwrap();
    assert!(!v.graphs.is_empty());
    assert!(v.graphs.iter().all(|g| g.kind == Kind::U));
    for graph in v.graphs.iter() {
        for hair in &graph.comb.hairs {
            assert!((hair.curvature.length().unwrap() - 0.5).abs() < 1e-8);
        }
    }
    r.execute(&mut d, "CurvatureGraph U=No V=Yes").unwrap();
    let v = r.curvature_graph_view(&d).unwrap().unwrap();
    assert!(!v.graphs.is_empty());
    assert!(v.graphs.iter().all(|g| g.kind == Kind::V));
    for graph in v.graphs.iter() {
        for hair in &graph.comb.hairs {
            assert!((hair.curvature.length().unwrap() - 0.5).abs() < 1e-8);
        }
    }
}
#[test]
fn curvature_graph_display_updates_preserve_redo_history() {
    let (r, mut d, _) = scene();
    r.execute(&mut d, "CurvatureGraph").unwrap();
    r.execute(&mut d, "Point 10,10,10").unwrap();
    d.undo().unwrap();
    let redo = d.redo_label().map(str::to_owned);
    r.execute(&mut d, "CurvatureGraph Density=2").unwrap();
    assert_eq!(d.redo_label(), redo.as_deref());
    d.redo().unwrap();
    assert_eq!(d.objects().len(), 2);
}
#[test]
fn curvature_graph_trimmed_face_combs_stay_outside_holes() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::new(Tolerance::DEFAULT);
    let loop_curve = |xy: &[(Real, Real)]| {
        let mut p = xy
            .iter()
            .map(|&(x, y)| Point3::try_new(x, y, 0.).unwrap())
            .collect::<Vec<_>>();
        p.push(p[0]);
        Polyline3::try_new(p, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let outer = loop_curve(&[(0., 0.), (10., 0.), (10., 10.), (0., 10.)]);
    let hole = loop_curve(&[(4., 4.), (4., 6.), (6., 6.), (6., 4.)]);
    let id = d
        .add_geometry(Geometry::Brep(
            Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "CurvatureGraph Count=3").unwrap();
    let view = r.curvature_graph_view(&d).unwrap().unwrap();
    assert_eq!(view.graphs.len(), 8);
    for g in view.graphs.iter() {
        for h in &g.comb.hairs {
            assert!(
                !(h.point.x() > 4. + 1e-9
                    && h.point.x() < 6. - 1e-9
                    && h.point.y() > 4. + 1e-9
                    && h.point.y() < 6. - 1e-9)
            );
        }
    }
}
#[test]
fn curvature_graph_repeated_view_queries_cannot_resurrect_stale_graphs_after_budget_failure() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "Line 0,0,0 2,0,0").unwrap();
    r.execute(&mut d, "Line 0,1,0 2,1,0").unwrap();
    let ids = d.objects().map(|o| o.id()).collect::<BTreeSet<_>>();
    let mut state = State {
        enabled: true,
        sources: ids.clone(),
        ..State::default()
    };
    Session::resolve_with_budget(&mut state, &d, 200).unwrap();
    let (_, circle_doc, _) = scene();
    let circle = circle_doc.objects().next().unwrap().geometry().clone();
    d.replace_object_geometries(ids.into_iter().map(|id| (id, circle.clone())))
        .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            Session::resolve_with_budget(&mut state, &d, 200),
            Err(CommandError::Geometry(GeometryError::CurvatureCombLimit))
        ));
    }
}
