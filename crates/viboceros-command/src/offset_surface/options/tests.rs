use super::*;

#[test]
fn tolerance_preferences_follow_document_defaults_and_keep_custom_values() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0").unwrap();
    r.execute(&mut d, "SelAll").unwrap();
    d.clear_history().unwrap();
    let t = Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap();
    d.set_tolerance(t);
    r.execute(&mut d, "OffsetSrf 0.5 Tolerance=0.0002").unwrap();
    d.undo().unwrap();
    assert_eq!(r.offset_surface_options().effective_tolerance(t), 0.0002);
    let newer = Tolerance::try_new(0.003, 1e-9, 1e-6).unwrap();
    d.set_tolerance(newer);
    r.execute(&mut d, "OffsetSrf 0.5").unwrap();
    d.undo().unwrap();
    assert_eq!(
        r.offset_surface_options().effective_tolerance(newer),
        0.0002
    );
    for reset in ["Default", "0", "0.003"] {
        r.execute(&mut d, &format!("OffsetSrf 0.5 Tolerance={reset}"))
            .unwrap();
        d.undo().unwrap();
        assert_eq!(r.offset_surface_options().tolerance, 0.);
        assert_eq!(r.offset_surface_options().effective_tolerance(t), 1e-6);
    }
}

#[test]
fn tolerance_lengths_are_unit_aware_and_invalid_inputs_are_atomic() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    r.execute(&mut d, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0").unwrap();
    r.execute(&mut d, "SelAll").unwrap();
    d.clear_history().unwrap();
    r.execute(&mut d, "OffsetSrf 0.5 Tolerance=.0001cm")
        .unwrap();
    d.undo().unwrap();
    assert_eq!(r.offset_surface_options().tolerance, 0.001);
    let before = format!("{d:?}");
    let preferences = r.offset_surface_options();
    for args in [
        "Tolerance=-1",
        "Tolerance=NaN",
        "Tolerance=inf",
        "Tolerance=1e400",
        "Tolerance=1 Tolerance=2",
        "Tolerance=Default Tolerance=0",
        "Tolerance=No",
        "Tolerance=1,2,3",
    ] {
        assert!(
            r.execute(&mut d, &format!("OffsetSrf 0.5 {args}")).is_err(),
            "{args}"
        );
        assert_eq!(format!("{d:?}"), before);
        assert_eq!(r.offset_surface_options(), preferences);
    }
}
