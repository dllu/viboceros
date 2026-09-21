use super::*;
use viboceros_document::{ColorRgb, SelectionMode};
use viboceros_geometry::LengthUnitSystem;

fn assert_fresh_views(views: &[Viewport; 4], document: &Document) {
    for view in views {
        let mut fresh = Viewport::new(view.kind);
        fresh.display_mode = view.display_mode;
        assert_same_scene(
            &view.object_scene(rect(), document),
            &fresh.object_scene(rect(), document),
        );
    }
}

#[test]
fn layer_style_and_selection_edits_preserve_shared_display_geometry() {
    let mut document = fixture();
    let ids: Vec<_> = document.objects().map(|o| o.id()).collect();
    let layer = document.add_layer("Display", ColorRgb::BLACK).unwrap();
    document
        .set_objects_layer(ids.iter().copied(), layer)
        .unwrap();
    let mut views = Viewport::standard_views();
    for (i, view) in views.iter_mut().enumerate() {
        view.display_mode = DisplayMode::ALL[i % DisplayMode::ALL.len()];
    }
    assert_fresh_views(&views, &document);
    let original = views[0].display_cache.borrow().entries.clone();
    for step in 0..6 {
        match step {
            0 => {
                document
                    .set_layer_color(layer, ColorRgb::new(25, 50, 75))
                    .unwrap();
            }
            1 => {
                document.set_layer_locked(layer, true).unwrap();
            }
            2 => {
                document.set_layer_locked(layer, false).unwrap();
            }
            3 => {
                document
                    .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                    .unwrap();
            }
            4 => {
                document.clear_selection();
            }
            _ => {
                document
                    .set_objects_color(ids.iter().copied(), Some(ColorRgb::new(80, 60, 40)))
                    .unwrap();
            }
        }
        assert_fresh_views(&views, &document);
        let cache = views[0].display_cache.borrow();
        for (id, old) in &original {
            assert!(Rc::ptr_eq(old, &cache.entries[id]));
            assert!(
                old.geometry
                    .shares_storage_with(document.object(*id).unwrap().geometry_snapshot())
            );
        }
    }
    // A cloned document retains identical immutable values and prepared scenes.
    let clone = document.clone();
    for view in &views {
        assert!(Arc::ptr_eq(
            &view.object_scene(rect(), &document),
            &view.object_scene(rect(), &clone)
        ));
    }
}

#[test]
fn hidden_layer_unit_edits_and_history_never_show_an_old_snapshot() {
    let mut document = fixture();
    let layer = document
        .add_layer("Hidden source", ColorRgb::BLACK)
        .unwrap();
    let ids: Vec<_> = document.objects().map(|o| o.id()).collect();
    document.set_objects_layer(ids, layer).unwrap();
    let views = Viewport::standard_views();
    let before = views.each_ref().map(|v| v.object_scene(rect(), &document));
    document.set_layer_visibility(layer, false).unwrap();
    assert_fresh_views(&views, &document);
    assert!(views[0].display_cache.borrow().entries.is_empty());
    for view in &views {
        let scene = view.object_scene(rect(), &document);
        assert!(scene.points.is_empty() && scene.lines.is_empty() && scene.triangles.is_empty());
    }
    // Unit edits affect hidden geometry too, while stale scenes may still own
    // the old snapshot. Restoring visibility must obtain the current one.
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    document.set_layer_visibility(layer, true).unwrap();
    assert_fresh_views(&views, &document);
    for (i, view) in views.iter().enumerate() {
        assert!(!Arc::ptr_eq(
            &before[i],
            &view.object_scene(rect(), &document)
        ));
    }
    document.undo().unwrap(); // show
    document.undo().unwrap(); // units
    document.undo().unwrap(); // hide
    for (i, view) in views.iter().enumerate() {
        assert_same_scene(&before[i], &view.object_scene(rect(), &document));
    }
    document.redo().unwrap();
    document.redo().unwrap();
    document.redo().unwrap();
    assert_fresh_views(&views, &document);
}

#[test]
fn model_tolerance_still_invalidates_display_data_for_the_same_snapshot() {
    let mut document = fixture();
    let view = Viewport::new(ViewKind::Top);
    view.object_scene(rect(), &document);
    let original = view.display_cache.borrow().entries.clone();
    document.set_tolerance(Tolerance::try_new(0.002, 0.002, 0.002).unwrap());
    view.object_scene(rect(), &document);
    let cache = view.display_cache.borrow();
    for (id, old) in original {
        assert!(!Rc::ptr_eq(&old, &cache.entries[&id]));
        assert!(
            old.geometry
                .shares_storage_with(&cache.entries[&id].geometry)
        );
    }
}
