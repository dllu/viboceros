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
fn rational_pilot_offsets_match_native_history_archives_and_requested_fit() {
    replay(tolerance::rational_rows("pilot"), "RATIONAL_PILOT_OFFSET");
}

#[test]
fn rational_holdout_offsets_match_native_history_archives_and_requested_fit() {
    replay(
        tolerance::rational_rows("holdouts"),
        "RATIONAL_HOLDOUT_OFFSET",
    );
}

#[test]
fn rational_final_offsets_match_native_history_archives_and_requested_fit() {
    replay(tolerance::rational_rows("final"), "RATIONAL_FINAL_OFFSET");
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
            let check = if id == "offset_rf_two_axes_both" {
                tolerance::check_rational_partitions
            } else if prefix.starts_with("RATIONAL_") {
                tolerance::check_rational
            } else {
                tolerance::check
            };
            check(
                d,
                r,
                a,
                tolerance::epsilon(&v),
                tolerance::reference_tolerance(&v),
            )
        });
        if prefix.starts_with("RATIONAL_") {
            record["rational_topology"] = tolerance::rational_topology(&d, &v);
        }
        record["native_partition_difference"] = (id == "offset_rf_two_axes_both").into();
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

#[cfg(feature = "native-smlib")]
#[test]
fn rational_curvature_break_matches_retained_native_capture() {
    replay(
        tolerance::rows()
            .into_iter()
            .filter(|(id, _)| id == "offset_th_rational")
            .collect(),
        "RATIONAL_OFFSET",
    );
}

#[cfg(feature = "native-smlib")]
#[test]
fn rational_fine_fit_declines_atomically_and_preserves_redo() {
    let (_, v) = tolerance::rows()
        .into_iter()
        .find(|(id, _)| id == "offset_th_rational")
        .unwrap();
    let mut d = tolerance::setup(&v);
    inputs::select_sources(&mut d);
    let before = format!("{d:?}");
    let error = CommandRegistry::with_builtins()
        .execute(&mut d, "OffsetSrf 0.5 Tolerance=1e-11")
        .unwrap_err();
    assert!(
        error.to_string().contains("normal-offset tolerance"),
        "{error}"
    );
    assert_eq!(before, format!("{d:?}"));
    assert!(d.can_redo());
    assert!(!d.can_undo());
}

#[cfg(feature = "native-smlib")]
#[test]
fn rational_patch_sign_orientation_and_both_sides_preserve_history() {
    use viboceros_document::Geometry;
    use viboceros_geometry::Brep;
    let (_, v) = tolerance::rows()
        .into_iter()
        .find(|(id, _)| id == "offset_th_rational")
        .unwrap();
    for (distance, reversed, both) in [(-0.3, false, false), (0.5, true, false), (-0.3, true, true)]
    {
        let mut d = tolerance::setup(&v);
        inputs::select_sources(&mut d);
        let id = d.selected_objects().next().unwrap().id();
        let Geometry::NurbsSurface(source) = d.object(id).unwrap().geometry().clone() else {
            panic!("authored source")
        };
        let mut b = Brep::try_surface_face(source.clone(), d.tolerance()).unwrap();
        if reversed {
            b.reverse_orientation();
        }
        d.replace_object_geometries([(id, Geometry::Brep(b))])
            .unwrap();
        d.clear_history().unwrap();
        let before = d.objects().cloned().collect::<Vec<_>>();
        let command = format!(
            "OffsetSrf {distance} Tolerance=0.0001 DeleteInput=Yes BothSides={}",
            if both { "Yes" } else { "No" }
        );
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut d, &command).unwrap();
        assert_eq!(d.object(id), before.iter().find(|o| o.id() == id));
        assert_eq!(d.objects().count(), before.len() + if both { 2 } else { 1 });
        let outputs = d
            .objects()
            .filter(|o| o.attributes().name().is_none())
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), if both { 2 } else { 1 });
        for (skin, output) in outputs.into_iter().enumerate() {
            let Geometry::Brep(b) = output.geometry() else {
                panic!("offset shell")
            };
            assert_eq!(b.faces().len(), 2);
            assert_eq!(b.edge_use_counts().iter().filter(|&&n| n == 2).count(), 1);
            assert!(b.faces().iter().all(|f| f.is_reversed() == reversed));
            let sign = if skin == 0 { 1. } else { -1. };
            for f in b.faces() {
                let s = f.surface();
                for i in 0..=8 {
                    for j in 0..=8 {
                        let u = s.parameter_at_u(i as f64 / 8.).unwrap();
                        let v = s.parameter_at_v(j as f64 / 8.).unwrap();
                        let ideal = source
                            .evaluate(u, v)
                            .unwrap()
                            .translated(
                                source
                                    .normal_at(u, v)
                                    .unwrap()
                                    .as_vector()
                                    .scaled(distance * sign * if reversed { -1. } else { 1. })
                                    .unwrap(),
                            )
                            .unwrap();
                        assert!(ideal.distance_to(s.evaluate(u, v).unwrap()).unwrap() <= 1e-7);
                    }
                }
            }
        }
        let after = d.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut d, "Undo").unwrap();
        assert_eq!(before, d.objects().cloned().collect::<Vec<_>>());
        registry.execute(&mut d, "Redo").unwrap();
        assert_eq!(after, d.objects().cloned().collect::<Vec<_>>());
    }
}
