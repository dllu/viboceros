//! Actual adaptive construction retains selection, originals and atomic history.
use super::*;

#[test]
fn adaptive_rational_refit_has_one_undo_step_and_retains_redo_after_failure() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for z in [0., 2., 4.] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [
                ([2., 0., z], 1.),
                ([2., 2., z], std::f64::consts::FRAC_1_SQRT_2),
                ([0., 2., z], 1.),
            ]
            .map(|(p, w)| {
                viboceros_geometry::WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w)
                    .unwrap()
            })
            .to_vec(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "Loft Refit=0.0007")
        .unwrap();
    let after = document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(after.len(), before.len() + 1);
    for source in &before {
        assert_eq!(document.object(source.id()).unwrap(), source);
    }
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
    let Geometry::Brep(brep) = after
        .iter()
        .find(|o| !ids.contains(&o.id()))
        .unwrap()
        .geometry()
    else {
        panic!("Loft BRep")
    };
    assert!(brep.faces()[0].surface().control_point_count_v() > 4);
    assert!(
        brep.faces()[0]
            .surface()
            .control_points()
            .iter()
            .all(|p| p.weight() == 1.)
    );
    assert_eq!(document.undo_label(), Some("Loft"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), None);
    assert!(registry.execute(&mut document, "Loft Refit=100").is_err());
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
    assert_eq!(document.redo_label(), Some("Loft"));
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}
