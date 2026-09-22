use super::*;
use crate::{CommandContext, CommandRegistry};
use viboceros_document::{ObjectAttributes, SelectionMode};
use viboceros_geometry::{Point3, TriangleMesh};

fn mesh(x: Real, scale: Real, reversed: bool) -> Geometry {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let m = TriangleMesh::try_new(
        vec![
            p(x, 0., 0.),
            p(x + 3. * scale, 0., 0.),
            p(x, 4. * scale, 0.),
            p(x, 0., 5. * scale),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    Geometry::Mesh(if reversed { m.reversed() } else { m })
}

#[test]
fn signed_cumulative_marker_retains_sources_groups_and_real_undo_redo() {
    let mut d = Document::default();
    let r = CommandRegistry::with_builtins();
    let ids = [mesh(0., 1., false), mesh(10., 2., true)].map(|g| {
        d.add_geometry_with_attributes(
            g,
            ObjectAttributes::on_layer(d.current_layer_id()).with_name("Source"),
        )
        .unwrap()
    });
    d.add_group(Some("group".into()), ids).unwrap();
    d.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    r.execute(&mut d, "VolumeCentroid").unwrap();
    let after = d.objects().cloned().collect::<Vec<_>>();
    assert_eq!(&after[..2], before);
    let marker = after.last().unwrap();
    let Geometry::Point(p) = marker.geometry() else {
        panic!()
    };
    assert_eq!(p.to_array(), [365. / 28., 15. / 7., 75. / 28.]);
    assert!(marker.group_ids().is_empty());
    assert!(marker.attributes().name().is_none());
    assert_eq!(marker.attributes().layer_id(), d.current_layer_id());
    assert!(!d.is_selected(marker.id()));
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), ids);
    assert_eq!(d.undo_label(), Some("VolumeCentroid"));
    for _ in 0..2 {
        r.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        r.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn partial_group_postselection_and_signed_cancellation_do_not_expand_membership() {
    let r = CommandRegistry::with_builtins();
    for postselect in [false, true] {
        let mut d = Document::default();
        let ids = [mesh(0., 1., false), mesh(10., 1., true)].map(|g| d.add_geometry(g).unwrap());
        d.add_group(Some("group".into()), ids).unwrap();
        d.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let before = d.objects().cloned().collect::<Vec<_>>();
        let undo = d.undo_label().map(str::to_owned);
        let redo = d.redo_label().map(str::to_owned);
        if postselect {
            r.execute_postselected(&mut d, "VolumeCentroid", CommandContext::default())
                .unwrap();
        } else {
            r.execute(&mut d, "VolumeCentroid").unwrap();
        }
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(d.selected_object_count(), if postselect { 0 } else { 2 });
        assert_eq!(d.undo_label(), undo.as_deref());
        assert_eq!(d.redo_label(), redo.as_deref());
        d.select_objects_direct([ids[0]], SelectionMode::Replace)
            .unwrap();
        r.execute_postselected(&mut d, "VolumeCentroid", CommandContext::default())
            .unwrap();
        assert_eq!(d.selected_object_count(), 0);
        assert_eq!(d.objects().len(), 3);
        assert!(
            matches!(d.objects().last().unwrap().geometry(),Geometry::Point(p) if p.to_array()==[0.75,1.,1.25])
        );
    }
}

#[test]
fn ignored_geometry_is_preserved_and_empty_eligible_selection_is_cleaned_up() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let solid = d.add_geometry(mesh(0., 1., false)).unwrap();
    r.execute(&mut d, "Line 0,0 1,0").unwrap();
    let line = d.objects().last().unwrap().id();
    let prompt = r
        .object_selection_prompt("VolumeCentroid")
        .unwrap()
        .unwrap();
    assert!(prompt.filter.accepts_object(d.object(solid).unwrap()));
    assert!(!prompt.filter.accepts_object(d.object(line).unwrap()));
    d.select_objects_direct([solid, line], SelectionMode::Replace)
        .unwrap();
    r.execute(&mut d, "VolumeCentroid").unwrap();
    assert!(d.is_selected(line));
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    assert!(r.execute(&mut d, "VolumeCentroid").is_err());
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(d.selected_object_count(), 0);
    assert_eq!(d.redo_label(), Some("VolumeCentroid"));
}

#[test]
fn late_open_mesh_and_argument_errors_are_atomic() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let solid = d.add_geometry(mesh(0., 1., false)).unwrap();
    let Geometry::Mesh(m) = mesh(10., 1., false) else {
        panic!()
    };
    let open = d
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                m.vertices().to_vec(),
                m.triangles()[1..].to_vec(),
                d.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    r.execute(&mut d, "Point 0,0").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([solid, open], SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    for command in ["VolumeCentroid extra", "VolumeCentroid"] {
        assert!(r.execute(&mut d, command).is_err());
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(d.redo_label(), Some("Point"));
        assert_eq!(d.selected_object_count(), 2);
    }
}
