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

#[test]
fn explicit_locked_closed_lengths_keep_valid_intervals_across_chart_boundaries() {
    for (direction, expected_start, expected_end) in [
        ("Forward", [3.2, 0., 0.], [2.8, 6., 0.]),
        ("Backward", [0., 4.8, 0.], [3.2, 0., 0.]),
    ] {
        let registry = CommandRegistry::with_builtins();
        let mut d = Document::default();
        registry.execute(&mut d, "Rectangle 0,0 4,6").unwrap();
        let id = d.objects().next().unwrap().id();
        let curve = d.object(id).unwrap().geometry().curve_ref().unwrap();
        let anchor = curve
            .closest_parameter(Point3::try_new(3.2, 0., 0.).unwrap(), d.tolerance())
            .unwrap();
        d.select_command_results([id]).unwrap();
        registry
            .execute(
                &mut d,
                &format!("SubCrv Numeric={anchor},8,{anchor} Locked={direction} Copy=Yes"),
            )
            .unwrap();
        let curve = d
            .selected_objects()
            .next()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!((curve.length(d.tolerance()).unwrap() - 8.).abs() < 1e-8);
        assert!(
            curve
                .start_point()
                .unwrap()
                .distance_to(Point3::try_from(expected_start).unwrap())
                .unwrap()
                < 1e-8
        );
        assert!(
            curve
                .end_point()
                .unwrap()
                .distance_to(Point3::try_from(expected_end).unwrap())
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn numeric_confirmation_on_close_closed_branches_chooses_the_candidate_endpoint() {
    let registry = CommandRegistry::with_builtins();
    let mut d = Document::default();
    registry
        .execute(&mut d, "Polyline 0,0 1,0 1,1 21,1 0,0")
        .unwrap();
    let id = d.objects().next().unwrap().id();
    let curve = d.object(id).unwrap().geometry().curve_ref().unwrap();
    let anchor = curve
        .closest_parameter(Point3::try_new(0.8, 0., 0.).unwrap(), d.tolerance())
        .unwrap();
    let confirm = curve
        .closest_parameter(Point3::try_new(1., 0.2, 0.).unwrap(), d.tolerance())
        .unwrap();
    d.select_command_results([id]).unwrap();
    registry
        .execute(
            &mut d,
            &format!("SubCrv Numeric={anchor},8,{confirm} Copy=Yes"),
        )
        .unwrap();
    let curve = d
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!((curve.length(d.tolerance()).unwrap() - 8.).abs() < 1e-8);
    assert!(
        curve
            .start_point()
            .unwrap()
            .distance_to(Point3::try_new(7.191850591615953, 0.34246907579123587, 0.).unwrap())
            .unwrap()
            < 1e-8
    );
    assert!(
        curve
            .end_point()
            .unwrap()
            .distance_to(Point3::try_new(0.8, 0., 0.).unwrap())
            .unwrap()
            < 1e-8
    );
}
