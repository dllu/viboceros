#[path = "support/brep_comparison.rs"]
#[allow(dead_code)]
mod brep_comparison;
#[path = "support/offset_surface.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/offset_surface_loose.rs"]
#[allow(dead_code)]
mod loose;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/offset_surface_tolerance.rs"]
#[allow(dead_code)]
mod tolerance;
use viboceros_command::CommandRegistry;

#[test]
fn tolerance_offsets_match_native_history_archives_and_requested_fit() {
    let mut count = 0;
    let mut declined = 0;
    for (id, v) in tolerance::rows() {
        println!("TOLERANCE_OFFSET_CASE {id}");
        let mut d = tolerance::setup(&v);
        let state = inputs::State::capture(&d);
        tolerance::check(&d, &v["before"], false, 1e-6);
        inputs::select_sources(&mut d);
        if !tolerance::supported(&id, &v) {
            let before = format!("{d:?}");
            let registry = CommandRegistry::with_builtins();
            let error = registry.execute(&mut d, &inputs::command(&v)).unwrap_err();
            assert_eq!(
                registry.offset_surface_options().tolerance,
                tolerance::options(&v).tolerance
            );
            if cfg!(feature = "native-smlib") {
                assert!(error.to_string().contains("outside"), "{error}");
            }
            assert_eq!(before, format!("{d:?}"));
            assert!(d.can_redo());
            println!("TOLERANCE_OFFSET_COMMAND_DECLINED {id}");
            declined += 1;
            continue;
        }
        if inputs::success(&v) {
            let r = CommandRegistry::with_builtins();
            let text = inputs::command(&v);
            (if inputs::postselected(&v) {
                r.execute_postselected(&mut d, &text, Default::default())
            } else {
                r.execute(&mut d, &text)
            })
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        }
        let field = if inputs::success(&v) {
            tolerance::field_error(&d, &v)
        } else {
            if inputs::postselected(&v) {
                inputs::clear_command_selection(&mut d);
            }
            (0., 0.)
        };
        let mut record = state.verify_with(&mut d, &v, |d, r, a| {
            tolerance::check(d, r, a, tolerance::epsilon(&v))
        });
        record["id"] = id.into();
        record["maximum_field_error"] = field.0.into();
        record["native_maximum_field_error"] = field.1.into();
        println!("TOLERANCE_OFFSET_COMMAND {record}");
        count += 1;
    }
    println!("TOLERANCE_OFFSET_COMMAND_COUNT {count} DECLINED {declined}");
}

#[cfg(feature = "native-smlib")]
#[test]
fn unresolved_precision_declines_atomically_and_preserves_redo() {
    let (_, v) = tolerance::rows()
        .into_iter()
        .find(|(id, _)| id == "offset_th_bezier_fine")
        .unwrap();
    let mut d = tolerance::setup(&v);
    inputs::select_sources(&mut d);
    let before = format!("{d:?}");
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut d, "OffsetSrf 0.5 Tolerance=1e-15")
            .is_err()
    );
    assert_eq!(format!("{d:?}"), before);
    assert!(d.can_redo());
    assert!(!d.can_undo());
}
