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
    replay(tolerance::rows(), "TOLERANCE_OFFSET");
}

#[test]
fn trimmed_offsets_match_native_history_archives_and_requested_fit() {
    replay(tolerance::trimmed_rows(), "TRIM_OFFSET");
}

#[test]
fn native_trim_reference_edges_are_admitted_at_their_captured_fit_budget() {
    for (id, v) in tolerance::trimmed_rows() {
        let t = tolerance::setup(&v).tolerance();
        for row in v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["guide"] != true && r["source"] != true)
        {
            assert_eq!(row["native_valid"], true, "{id}");
            let b = tolerance::reference(row, t, tolerance::reference_tolerance(&v));
            assert_eq!(
                b.faces().len(),
                row["face_count"].as_u64().unwrap() as usize,
                "{id}"
            );
        }
    }
}

fn replay(rows: Vec<(String, serde_json::Value)>, prefix: &str) {
    let mut count = 0;
    let mut declined = 0;
    for (id, v) in rows {
        println!("{prefix}_CASE {id}");
        let mut d = tolerance::setup(&v);
        let state = inputs::State::capture(&d);
        tolerance::check(&d, &v["before"], false, 1e-6, d.tolerance().absolute());
        inputs::select_sources(&mut d);
        let started = std::time::Instant::now();
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
            println!("{prefix}_COMMAND_DECLINED {id}");
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
        println!("{prefix}_PREPARED {id} {:?}", started.elapsed());
        let field = if inputs::success(&v) {
            tolerance::field_error(&d, &v)
        } else {
            if inputs::postselected(&v) {
                inputs::clear_command_selection(&mut d);
            }
            (0., 0.)
        };
        println!("{prefix}_FIELD_CHECKED {id} {:?}", started.elapsed());
        let mut record = state.verify_with(&mut d, &v, |d, r, a| {
            tolerance::check(
                d,
                r,
                a,
                tolerance::epsilon(&v),
                tolerance::reference_tolerance(&v),
            )
        });
        record["id"] = id.into();
        record["maximum_field_error"] = field.0.into();
        record["native_maximum_field_error"] = field.1.into();
        record["native_reference_tolerance"] = tolerance::reference_tolerance(&v).into();
        println!("{prefix}_COMMAND {record}");
        count += 1;
    }
    println!("{prefix}_COMMAND_COUNT {count} DECLINED {declined}");
}

#[cfg(feature = "native-smlib")]
#[test]
fn trimmed_curvature_breaks_decline_atomically_and_preserve_redo() {
    use viboceros_document::Geometry;
    let (_, v) = tolerance::rows()
        .into_iter()
        .find(|(id, _)| id == "offset_th_rational")
        .unwrap();
    let mut d = tolerance::setup(&v);
    let id = inputs::guide(&d, 0);
    let Geometry::NurbsSurface(s) = d.object(id).unwrap().geometry() else {
        panic!("authored surface")
    };
    let b = viboceros_geometry::Brep::try_rectangular_surface_face(
        s.clone(),
        2.2..=6.8,
        -2.0..=4.0,
        d.tolerance(),
    )
    .unwrap();
    d.replace_object_geometries([(id, Geometry::Brep(b))])
        .unwrap();
    d.clear_history().unwrap();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut d, "Point 90,91,92").unwrap();
    commands.execute(&mut d, "Undo").unwrap();
    inputs::select_sources(&mut d);
    let before = format!("{d:?}");
    let error = commands
        .execute(&mut d, "OffsetSrf 0.5 Tolerance=0.0001")
        .unwrap_err();
    assert!(error.to_string().contains("curvature breaks"), "{error}");
    assert_eq!(before, format!("{d:?}"));
    assert!(d.can_redo());
    assert!(!d.can_undo());
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
