use super::*;

#[test]
fn locked_subcurve_scripts_use_explicit_direction_and_preserve_source_orientation() {
    for (direction, fraction) in [("Forward", 0.5), ("Backward", 0.5 - 2. / 52_f64.sqrt())] {
        let registry = CommandRegistry::with_builtins();
        let mut d = Document::default();
        registry.execute(&mut d, "Line 0,0 4,6").unwrap();
        let id = d.objects().next().unwrap().id();
        let domain = d
            .object(id)
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .domain();
        let anchor = (*domain.start() + *domain.end()) * 0.5;
        d.select_command_results([id]).unwrap();
        registry
            .execute(
                &mut d,
                &format!("SubCrv Numeric={anchor},-2,{anchor} Copy=Yes Locked={direction}"),
            )
            .unwrap();
        let c = d
            .selected_objects()
            .next()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!((c.length(d.tolerance()).unwrap() - 2.).abs() < 1e-8);
        assert!(
            c.start_point()
                .unwrap()
                .distance_to(Point3::try_new(4. * fraction, 6. * fraction, 0.).unwrap())
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn locked_subcurve_no_result_and_invalid_options_do_not_edit_history() {
    let registry = CommandRegistry::with_builtins();
    let mut d = Document::default();
    registry.execute(&mut d, "Line 0,0 4,6").unwrap();
    let id = d.objects().next().unwrap().id();
    let original = d.object(id).unwrap().geometry().clone();
    d.clear_history().unwrap();
    for option in ["Locked=Maybe", "Locked=Forward Locked=Backward"] {
        d.select_command_results([id]).unwrap();
        assert!(
            registry
                .execute(&mut d, &format!("SubCrv 2,3 .8,1.2 {option}"))
                .is_err()
        );
        assert_eq!(d.object(id).unwrap().geometry(), &original);
        assert!(!d.can_undo());
    }
    d.select_command_results([id]).unwrap();
    registry
        .execute(&mut d, "SubCrv 2,3 .8,1.2 Locked=Forward")
        .unwrap();
    assert_eq!(d.object(id).unwrap().geometry(), &original);
    assert!(!d.can_undo());
    assert_eq!(d.selected_object_count(), 0);
}
