use super::*;
use crate::{CommandContext, CommandRegistry};
use viboceros_document::{ObjectAttributes, SelectionMode};
use viboceros_geometry::{Circle3, Point3, UnitVector3};

fn setup() -> (Document, Vec<viboceros_document::ObjectId>) {
    let mut d = Document::default();
    let normal = UnitVector3::try_new(0., 0., 1., d.tolerance()).unwrap();
    let ids = [(0., 1.), (10., 2.), (0., 3.)]
        .into_iter()
        .map(|(x, r)| {
            d.add_geometry_with_attributes(
                Geometry::Circle(
                    Circle3::try_new(
                        Point3::try_new(x, 0., 0.).unwrap(),
                        r,
                        normal,
                        d.tolerance(),
                    )
                    .unwrap(),
                ),
                ObjectAttributes::on_layer(d.current_layer_id()).with_name("Source"),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    (d, ids)
}

#[test]
fn cumulative_marker_preserves_sources_groups_attributes_and_preselection_with_undo_redo() {
    let r = CommandRegistry::with_builtins();
    let (mut d, ids) = setup();
    d.add_group(Some("sources".into()), ids.clone()).unwrap();
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let selected = d.selected_object_ids().collect::<Vec<_>>();
    r.execute(&mut d, "AreaCentroid").unwrap();
    let after = d.objects().cloned().collect::<Vec<_>>();
    assert_eq!(&after[..before.len()], before);
    let marker = after.last().unwrap();
    let Geometry::Point(point) = marker.geometry() else {
        panic!()
    };
    assert!((point.x() - 20. / 7.).abs() < 1e-14);
    assert_eq!(point.y(), 0.);
    assert!(marker.attributes().name().is_none());
    assert!(marker.group_ids().is_empty());
    assert_eq!(marker.attributes().layer_id(), d.current_layer_id());
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), selected);
    assert_eq!(d.undo_label(), Some("AreaCentroid"));
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn direct_partial_group_selection_uses_only_picked_sources_and_postselection_clears() {
    let r = CommandRegistry::with_builtins();
    let (mut d, ids) = setup();
    d.add_group(Some("sources".into()), ids.clone()).unwrap();
    d.select_objects_direct(ids[..2].iter().copied(), SelectionMode::Replace)
        .unwrap();
    r.execute_postselected(&mut d, "AreaCentroid", CommandContext::default())
        .unwrap();
    let Geometry::Point(p) = d.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(p.x(), 8.);
    assert_eq!(d.selected_object_count(), 0);
}

#[test]
fn prompt_filters_open_curves_and_points_while_mixed_preselection_retains_them() {
    let r = CommandRegistry::with_builtins();
    let (mut d, ids) = setup();
    r.execute(&mut d, "Line 0,0 1,0").unwrap();
    let line = d.objects().last().unwrap().id();
    r.execute(&mut d, "Point 20,30").unwrap();
    let point = d.objects().last().unwrap().id();
    let prompt = r.object_selection_prompt("AreaCentroid").unwrap().unwrap();
    assert!(prompt.filter.accepts_object(d.object(ids[0]).unwrap()));
    assert!(!prompt.filter.accepts_object(d.object(line).unwrap()));
    assert!(!prompt.filter.accepts_object(d.object(point).unwrap()));
    d.select_objects_direct([ids[0], line, point], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "AreaCentroid").unwrap();
    assert!(d.is_selected(line));
    assert!(d.is_selected(point));
    let Geometry::Point(p) = d.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(*p, Point3::try_new(0., 0., 0.).unwrap());
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let redo = d.redo_label().map(str::to_owned);
    assert!(r.execute(&mut d, "AreaCentroid").is_err());
    assert_eq!(d.selected_object_count(), 0);
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(d.redo_label(), redo.as_deref());
}

#[test]
fn argument_and_late_geometry_failures_do_not_leave_markers_or_clear_redo() {
    let r = CommandRegistry::with_builtins();
    let (mut d, ids) = setup();
    r.execute(&mut d, "Polyline 0,0 2,2 0,2 2,0 0,0").unwrap();
    let invalid = d.objects().last().unwrap().id();
    r.execute(&mut d, "Point 5,5").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([ids[0], invalid], SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let undo = d.undo_label().map(str::to_owned);
    let redo = d.redo_label().map(str::to_owned);
    for command in ["AreaCentroid extra", "AreaCentroid"] {
        assert!(r.execute(&mut d, command).is_err());
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(d.undo_label(), undo.as_deref());
        assert_eq!(d.redo_label(), redo.as_deref());
        assert_eq!(d.selected_object_count(), 2);
    }
}
