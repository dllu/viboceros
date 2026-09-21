use super::*;
use std::sync::Arc;
use viboceros_command::CommandRegistry;

fn fixture() -> Document {
    let mut document = Document::default();
    let commands = CommandRegistry::with_builtins();
    for command in [
        "Circle 0,0,0 2",
        "Curve 0,0 1,3 3,2 4,0 Degree=3",
        "SrfPt 0,0,0 4,0,0 4,4,1 0,4,0",
        "Box 5,0,0 7,2,0 3",
        "MeshBox -3,-3,0 -1,-1,0 2",
    ] {
        commands.execute(&mut document, command).unwrap();
    }
    document.clear_selection();
    document
}

fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.))
}

fn assert_same_scene(a: &GpuViewportScene, b: &GpuViewportScene) {
    assert_eq!(
        bytemuck::bytes_of(&a.uniform),
        bytemuck::bytes_of(&b.uniform)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&a.triangles),
        bytemuck::cast_slice::<_, u8>(&b.triangles)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&a.lines),
        bytemuck::cast_slice::<_, u8>(&b.lines)
    );
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&a.points),
        bytemuck::cast_slice::<_, u8>(&b.points)
    );
    assert_eq!(a.transparent, b.transparent);
}

#[test]
fn four_views_share_geometry_and_reuse_unchanged_scenes() {
    let document = fixture();
    let mut views = Viewport::standard_views();
    for view in &mut views {
        view.display_mode = DisplayMode::Shaded;
    }
    let first = views
        .each_ref()
        .map(|view| view.object_scene(rect(), &document));
    let entries: Vec<_> = views[0]
        .display_cache
        .borrow()
        .entries
        .values()
        .cloned()
        .collect();
    assert_eq!(entries.len(), document.objects().len());
    for entry in &entries {
        assert!(entry.wires.get().is_some());
        if entry.mesh().is_some() {
            assert!(entry.normals.get().is_some());
        }
    }
    for (i, view) in views.iter().enumerate() {
        assert!(Rc::ptr_eq(&views[0].display_cache, &view.display_cache));
        assert!(Arc::ptr_eq(
            &first[i],
            &view.object_scene(rect(), &document)
        ));
    }
    // Navigation rebuilds only that view; geometry and the other scenes survive.
    views[1].orbit_yaw += 0.1;
    assert!(!Arc::ptr_eq(
        &first[1],
        &views[1].object_scene(rect(), &document)
    ));
    assert!(Arc::ptr_eq(
        &first[0],
        &views[0].object_scene(rect(), &document)
    ));
    let cache = views[0].display_cache.borrow();
    for entry in &entries {
        assert!(
            cache
                .entries
                .values()
                .any(|current| Rc::ptr_eq(entry, current))
        );
    }
}

#[test]
fn edits_undo_redo_rollback_and_cloned_documents_never_reuse_stale_geometry() {
    let mut document = fixture();
    let view = Viewport::new(ViewKind::Top);
    let commands = CommandRegistry::with_builtins();
    let before = view.object_scene(rect(), &document);
    commands.execute(&mut document, "SelAll").unwrap();
    commands.execute(&mut document, "Move 0,0,0 2,1,0").unwrap();
    document.clear_selection();
    let moved = view.object_scene(rect(), &document);
    assert!(!Arc::ptr_eq(&before, &moved));
    commands.execute(&mut document, "Undo").unwrap();
    document.clear_selection();
    assert_same_scene(&before, &view.object_scene(rect(), &document));
    commands.execute(&mut document, "Redo").unwrap();
    document.clear_selection();
    assert_same_scene(&moved, &view.object_scene(rect(), &document));
    let mut clone = document.clone();
    commands.execute(&mut clone, "SelAll").unwrap();
    commands.execute(&mut clone, "Move 0,0,0 -4,0,0").unwrap();
    clone.clear_selection();
    assert!(!Arc::ptr_eq(&moved, &view.object_scene(rect(), &clone)));
    assert_same_scene(&moved, &view.object_scene(rect(), &document));
    document.begin_transaction("cancelled edit").unwrap();
    document
        .add_geometry(Geometry::Point(Point3::try_new(30., 0., 0.).unwrap()))
        .unwrap();
    let during = view.object_scene(rect(), &document);
    document.rollback_transaction().unwrap();
    let after = view.object_scene(rect(), &document);
    assert!(!Arc::ptr_eq(&during, &after));
    assert_same_scene(&moved, &after);
}

#[test]
fn display_changes_invalidate_scene_and_hidden_deleted_entries_are_evicted() {
    let mut document = fixture();
    let mut view = Viewport::new(ViewKind::Top);
    let commands = CommandRegistry::with_builtins();
    let mut previous = view.object_scene(rect(), &document);
    for command in [
        "SelAll",
        "SetObjectColor 20,40,60",
        "SelNone",
        "Lock",
        "Unlock",
        "Hide",
        "Show",
    ] {
        // Attribute edits operate on selection; select before edits which need it.
        if matches!(command, "Lock" | "Hide") {
            commands.execute(&mut document, "SelAll").unwrap();
        }
        commands.execute(&mut document, command).unwrap();
        let current = view.object_scene(rect(), &document);
        // Selection can mask the edited display color, so validate against a fresh view.
        let fresh = Viewport::new(ViewKind::Top).object_scene(rect(), &document);
        assert_same_scene(&current, &fresh);
        previous = current;
    }
    view.display_mode = DisplayMode::Shaded;
    assert!(!Arc::ptr_eq(
        &previous,
        &view.object_scene(rect(), &document)
    ));
    document.set_tolerance(Tolerance::try_new(0.002, 0.002, 0.002).unwrap());
    let current = view.object_scene(rect(), &document);
    let mut fresh = Viewport::new(ViewKind::Top);
    fresh.display_mode = DisplayMode::Shaded;
    assert_same_scene(&current, &fresh.object_scene(rect(), &document));
    commands.execute(&mut document, "SelAll").unwrap();
    commands.execute(&mut document, "Delete").unwrap();
    let empty = view.object_scene(rect(), &document);
    assert!(empty.lines.is_empty() && empty.triangles.is_empty());
    assert!(view.display_cache.borrow().entries.is_empty());
}

#[test]
fn wireframe_does_not_tessellate_surfaces_and_picking_reuses_shaded_mesh() {
    let document = fixture();
    let mut view = Viewport::new(ViewKind::Top);
    view.object_scene(rect(), &document);
    {
        let cache = view.display_cache.borrow();
        for entry in cache.entries.values() {
            assert!(entry.mesh.get().is_none());
            assert!(entry.normals.get().is_none());
        }
    }
    view.display_mode = DisplayMode::Shaded;
    view.object_scene(rect(), &document);
    let entries: Vec<_> = view
        .display_cache
        .borrow()
        .entries
        .values()
        .cloned()
        .collect();
    view.pick_object(rect().center(), rect(), &document);
    view.objects_in_selection(rect(), rect(), true, &document);
    for entry in entries {
        assert!(
            view.display_cache
                .borrow()
                .entries
                .values()
                .any(|current| Rc::ptr_eq(&entry, current))
        );
    }
}

#[test]
fn cached_window_geometry_matches_original_projection_in_every_view_and_mode() {
    let document = fixture();
    for mut view in Viewport::standard_views() {
        for mode in DisplayMode::ALL {
            view.display_mode = mode;
            for object in document.objects() {
                let display = view
                    .display_cache
                    .borrow_mut()
                    .get(object, document.tolerance());
                assert_eq!(
                    view.projected_display(&display, rect(), document.tolerance()),
                    view.projected_primitives(
                        object.geometry(),
                        object.attributes(),
                        rect(),
                        document.tolerance()
                    ),
                    "{:?} {mode:?} {:?}",
                    view.kind,
                    object.id(),
                );
            }
        }
    }
}

#[test]
#[ignore = "manual release-mode CPU scene benchmark; run with --ignored --nocapture"]
fn benchmark_cached_viewport_frames() {
    use std::hint::black_box;
    use std::time::Instant;
    let commands = CommandRegistry::with_builtins();
    let mut document = fixture();
    for i in 0..8 {
        commands
            .execute(&mut document, &format!("Sphere {},4,0 1", i * 3))
            .unwrap();
    }
    let mut views = Viewport::standard_views();
    for view in &mut views {
        view.display_mode = DisplayMode::Shaded;
    }
    let start = Instant::now();
    for view in &views {
        black_box(view.object_scene(rect(), &document));
    }
    let cold = start.elapsed();
    let frames = 100;
    let start = Instant::now();
    for _ in 0..frames {
        for view in &views {
            black_box(view.object_scene(rect(), &document));
        }
    }
    let stationary = start.elapsed() / frames;
    let start = Instant::now();
    for _ in 0..frames {
        views[1].orbit_yaw += 0.001;
        for view in &views {
            black_box(view.object_scene(rect(), &document));
        }
    }
    let orbit = start.elapsed() / frames;
    eprintln!(
        "{} objects, four shaded views: cold {:?}, cached stationary {:?}/frame, cached orbit {:?}/frame (CPU scene preparation only)",
        document.objects().len(),
        cold,
        stationary,
        orbit
    );
}
