//! Multispan admission and atomic document history at tighter tolerances.
use super::*;

#[test]
fn multispan_refits_preserve_redo_when_a_tighter_fit_needs_subdivision() {
    let registry = CommandRegistry::with_builtins();
    for (points, accepted, rejected) in [
        (["0,0", "1,0", "2,0.00001"], "0.001", "0.000001"),
        (["0,0", "0.25,0", "0.75,0.000005"], "0.000001", "0.0000001"),
    ] {
        let mut document = Document::default();
        for z in [0., 3.] {
            let coordinates = points.map(|p| format!("{p},{z}")).join(" ");
            registry
                .execute(&mut document, &format!("Polyline {coordinates}"))
                .unwrap();
        }
        let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        document.clear_history().unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, &format!("Loft Refit={accepted}"))
            .unwrap();
        let after = document.objects().cloned().collect::<Vec<_>>();
        let output = after.iter().find(|o| !ids.contains(&o.id())).unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("Loft BRep")
        };
        assert_eq!(brep.faces()[0].surface().control_point_count_v(), 4);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
        for original in &before {
            assert_eq!(document.object(original.id()).unwrap(), original);
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.redo_label(), Some("Loft"));
        assert!(
            registry
                .execute(&mut document, &format!("Loft Refit={rejected}"))
                .is_err()
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
        assert_eq!(document.undo_label(), None);
        assert_eq!(document.redo_label(), Some("Loft"));
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
}
