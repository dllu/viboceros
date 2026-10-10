use super::*;
use crate::CommandRegistry;

#[test]
fn fillet_edge_prompt_owns_radius_and_edge_filter() {
    let registry = CommandRegistry::with_builtins();
    let prompt = registry
        .component_selection_prompt("FilletEdge Radius=0.5")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.kind, ComponentSelectionKind::BrepEdge);
    assert_eq!(prompt.numbers[0].value, 0.5);
    assert!(
        registry
            .component_selection_prompt("FilletEdge Radius=-1")
            .is_err()
    );
}

#[cfg(not(feature = "native-smlib"))]
#[test]
fn unavailable_native_kernel_does_not_change_selected_geometry() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0,0 10,10,0 10").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "FilletEdge Radius=1 All=Yes"),
        Err(CommandError::FilletEdgeRequiresNativeKernel)
    ));
    assert_eq!(format!("{doc:?}"), before);
}

#[cfg(feature = "native-smlib")]
mod native {
    use super::*;
    fn selected_box() -> (CommandRegistry, Document, ObjectId) {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Box 0,0,0 10,10,0 10").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        let id = doc.objects().next().unwrap().id();
        (registry, doc, id)
    }
    #[test]
    fn all_edge_command_preserves_identity_attributes_and_replays_history() {
        let (registry, mut doc, id) = selected_box();
        registry.execute(&mut doc, "SetObjectName Rounded").unwrap();
        registry.execute(&mut doc, "Group FilletGroup").unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut doc, "FilletEdge Radius=1 All=Yes")
            .unwrap();
        let object = doc.object(id).unwrap();
        assert_eq!(object.attributes(), before[0].attributes());
        assert_eq!(object.group_ids(), before[0].group_ids());
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("editable rounded B-rep expected")
        };
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), 26);
        assert_eq!(doc.undo_label(), Some("FilletEdge"));
        let after = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }
    #[test]
    fn one_original_edge_is_selected_once_by_typed_index() {
        let (registry, mut doc, id) = selected_box();
        registry
            .execute(&mut doc, &format!("FilletEdge 1 {id} 0,0"))
            .unwrap();
        let Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(brep.is_solid());
        assert_eq!(brep.faces().len(), 7);
        assert!(
            (brep.signed_volume(doc.tolerance()).unwrap()
                - (1000. - 10. * (1. - std::f64::consts::PI / 4.)))
                .abs()
                < 1e-7
        );
    }
    #[test]
    fn failed_later_source_leaves_every_object_and_history_unchanged() {
        let (registry, mut doc, first) = selected_box();
        registry.execute(&mut doc, "Box 20,0,0 30,10,0 10").unwrap();
        let second = doc.objects().last().unwrap().id();
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(&mut doc, &format!("FilletEdge 1 {first} 0 {second} 999"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    #[test]
    fn stale_geometry_invalidates_a_prepared_fillet() {
        let (registry, mut doc, id) = selected_box();
        let prepared = FilletEdgeSelection::prepare(&doc, [(id, 0)], 1.).unwrap();
        registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
        let before = format!("{doc:?}");
        assert!(prepared.commit(&mut doc).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}
