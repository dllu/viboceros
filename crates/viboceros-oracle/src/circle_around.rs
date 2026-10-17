//! Replay public Circle getters using shared capture, sizing and history code.
use super::*;
use crate::curve_getters::{Action, Fixture};
use viboceros_command::{
    circle_around_input::Input,
    curve_reference::{CapturedCurve, perpendicular_frame},
};
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last: &mut Option<Point3>,
    guides: &[(ObjectId, Geometry)],
) -> Result<(), ProbeError> {
    let mut guide: Option<CapturedCurve> = None;
    let mut around = false;
    let mut input: Option<Input> = None;
    let mut postselected = false;
    let mut ordinary = viboceros_command::circle_deform_input::Input::new(
        plane,
        commands.circle_preferences(),
        doc.tolerance(),
    );
    let mut completed = None;
    let mut tokens = vec!["Circle".to_owned()];
    for action in actions {
        if completed.is_some() {
            return Err(ProbeError::FixtureInvariant("trailing Circle input"));
        }
        let enter_cancels = if around {
            input.is_none_or(|i| {
                !matches!(
                    i.size_mode(),
                    viboceros_command::circle_input::CircleSizeMode::Radius
                        | viboceros_command::circle_input::CircleSizeMode::Diameter
                )
            })
        } else {
            !ordinary.accepts_default()
        };
        match action {
            Action::Option(word)
                if word.eq_ignore_ascii_case("Cancel")
                    || (word.eq_ignore_ascii_case("Enter") && enter_cancels) =>
            {
                if postselected && let Some(curve) = &guide {
                    doc.select_object(
                        curve.object_id(),
                        viboceros_document::SelectionMode::Remove,
                    )?;
                }
                return Ok(());
            }
            Action::Option(word) if word.eq_ignore_ascii_case("AroundCurve") => {
                around = true;
                let selected = doc.selected_object_ids().collect::<Vec<_>>();
                if let [id] = selected.as_slice()
                    && doc
                        .object(*id)
                        .is_some_and(|o| o.geometry().curve_ref().is_some())
                {
                    guide = Some(CapturedCurve::capture(doc, *id, None)?);
                }
            }
            Action::Pick { curve, edge, pick } => {
                if !around || guide.is_some() {
                    return Err(ProbeError::FixtureInvariant(
                        "Circle is not selecting a curve",
                    ));
                }
                let (id, geometry) = guides
                    .get(*curve)
                    .ok_or(ProbeError::FixtureInvariant("missing Circle guide"))?;
                if let (Some(edge), Some(p)) = (edge, pick) {
                    let resolved = viboceros_command::curve_reference::resolve(
                        geometry,
                        Some(*edge),
                        doc.tolerance(),
                    )
                    .ok_or(ProbeError::FixtureInvariant("missing Circle edge"))?;
                    let c = resolved.curve();
                    let p = Point3::try_from(*p)?;
                    let t = c.closest_parameter(p, doc.tolerance())?;
                    if c.evaluate(t)?.distance_to(p)? > doc.tolerance().absolute() {
                        return Err(ProbeError::FixtureInvariant(
                            "Circle pick must lie on the named edge",
                        ));
                    }
                }
                guide = Some(CapturedCurve::capture(doc, *id, *edge)?);
                postselected = edge.is_none() && !doc.is_selected(*id);
                if edge.is_none() {
                    doc.select_object(*id, viboceros_document::SelectionMode::Add)?;
                }
            }
            Action::Option(word) => {
                if !around && word.eq_ignore_ascii_case("Enter") {
                    tokens.push("Enter".into());
                    completed = ordinary.default_value()?;
                    continue;
                }
                if !around && word.eq_ignore_ascii_case("FitPoints") {
                    let command = if ordinary.output().deformable {
                        format!("{} FitPoints", ordinary.output().command_prefix())
                    } else {
                        "Circle FitPoints".into()
                    };
                    commands.execute_in_context(
                        doc,
                        &command,
                        viboceros_command::CommandContext {
                            construction_plane: plane,
                        },
                    )?;
                    return Ok(());
                }
                if !around {
                    tokens.push(word.clone());
                }
                if let Some(input) = &mut input {
                    if word.eq_ignore_ascii_case("Enter") {
                        completed = Some(input.default_size(commands.circle_preferences())?);
                    } else if let Some((k, v)) = word.split_once('=') {
                        if !input.option(k) {
                            return Err(ProbeError::FixtureInvariant("unsupported Circle option"));
                        }
                        let n = v
                            .parse()
                            .map_err(|_| ProbeError::FixtureInvariant("invalid Circle size"))?;
                        completed = Some(input.number(n)?);
                    } else if !input.option(word) {
                        return Err(ProbeError::FixtureInvariant("unsupported Circle option"));
                    }
                } else if !around && !ordinary.option(word)? {
                    if let Some((key, value)) = word.split_once('=')
                        && viboceros_command::circle_input::CircleSizeMode::parse(key).is_some()
                        && ordinary.option(key)?
                    {
                        completed =
                            ordinary.number(value.parse().map_err(|_| {
                                ProbeError::FixtureInvariant("invalid Circle size")
                            })?)?;
                    } else {
                        return Err(ProbeError::FixtureInvariant(
                            "unsupported setup Circle option",
                        ));
                    }
                }
            }
            Action::Number(value) => {
                if !around {
                    tokens.push(value.to_string());
                }
                if let Some(input) = &mut input {
                    match input.number(*value) {
                        Ok(circle) => completed = Some(circle),
                        // The native radius getter rejects nonpositive sizes
                        // and waits for the next authored value.
                        Err(_) if *value <= 0.0 => {}
                        Err(error) => return Err(error.into()),
                    }
                } else if !around {
                    completed = ordinary.number(*value)?;
                } else {
                    return Err(ProbeError::FixtureInvariant("Circle requires a center"));
                }
            }
            Action::Point(_) | Action::Relative { .. } => {
                let point = match action {
                    Action::Point(p) => Point3::try_from(*p)?,
                    Action::Relative {
                        relative: [x, y, z],
                    } => viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                        .unwrap()
                        .map_err(|_| ProbeError::FixtureInvariant("invalid Circle relative point"))?
                        .resolve(plane, *last)
                        .map_err(|_| {
                            ProbeError::FixtureInvariant("Circle relative point requires a base")
                        })?,
                    _ => unreachable!(),
                };
                if let Some(input) = &mut input {
                    let point = input.resolve_point(point)?;
                    completed = Some(input.point(point)?);
                    *last = Some(point);
                } else if let Some(curve) = &guide {
                    let frame = perpendicular_frame(curve.curve_ref(), point)?;
                    input = Some(Input::new(
                        frame,
                        commands.circle_preferences(),
                        doc.tolerance(),
                    ));
                    *last = Some(frame.origin());
                } else if !around {
                    let [x, y, z] = point.to_array();
                    tokens.push(format!("{x},{y},{z}"));
                    completed = ordinary.point(point)?;
                    *last = Some(point);
                } else {
                    return Err(ProbeError::FixtureInvariant("Circle requires a guide"));
                }
            }
        }
    }
    let circle = completed.ok_or(ProbeError::FixtureInvariant("incomplete Circle"))?;
    let command = if let Some(input) = input {
        let guide = guide.unwrap();
        let command = input.command(circle, guide.object_id(), guide.source().1, postselected)?;
        if ordinary.output().deformable {
            ordinary.output().wrap_command(&command)
        } else {
            command
        }
    } else {
        tokens.join(" ")
    };
    commands.execute_in_context(
        doc,
        &command,
        viboceros_command::CommandContext {
            construction_plane: plane,
        },
    )?;
    Ok(())
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    crate::curve_getters::run(f, iterations, tolerance, enter)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_around_native_geometry_size_modes_selection_and_actual_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_command_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_extended.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_extended_rhino_20261017.json"
                ),
                22,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_numeric.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_numeric_rhino_20261017.json"
                ),
                9,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_memory.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_memory_rhino_20261017.json"
                ),
                2,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let actual = run_request(&q).unwrap();
            assert_eq!(actual.results.len(), count);
            for (a, b) in actual.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    crate::curve_getters::assert_close(
                        &a.value[stage],
                        &b["value"][if stage == "after" {
                            "after_script"
                        } else {
                            stage
                        }],
                        &format!("{}/{stage}", a.id),
                    );
                }
                assert_eq!(a.value["undo_label"], "Circle");
                assert_eq!(a.value["redo_label"], "Circle");
            }
        }
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn circle_around_native_cancelled_getters_have_no_geometry_or_history() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/circle_around_controls.json"
        ))
        .unwrap();
        let n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/circle_around_controls_rhino_20261017.json"
        ))
        .unwrap();
        let actual = run_request(&q).unwrap();
        assert_eq!(actual.results.len(), 5);
        for (a, b) in actual.results.iter().zip(n["results"].as_array().unwrap()) {
            for stage in ["before", "after", "undo", "redo"] {
                crate::curve_getters::assert_close(
                    &a.value[stage],
                    &b["value"][if stage == "after" {
                        "after_script"
                    } else {
                        stage
                    }],
                    &format!("{}/{stage}", a.id),
                );
            }
            assert!(a.value["undo_label"].is_null());
            assert!(a.value["redo_label"].is_null());
        }
    }
}

#[cfg(test)]
mod deform_tests {
    use super::*;
    #[test]
    fn circle_deform_native_historical_clamped_layouts_match_full_geometry_and_history() {
        let mut q: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/circle_deform_diagnostics.json"
        ))
        .unwrap();
        let n: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/circle_deform_diagnostics_rhino_20261017.json"
        ))
        .unwrap();
        let indexes = n["results"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let v = &r["value"];
                (v["input_trace"]["errors"].as_array().unwrap().is_empty()
                    && !v["history"].as_str().unwrap().contains("Unknown command"))
                .then_some(i)
            })
            .collect::<Vec<_>>();
        let ops = q["operations"].as_array().unwrap();
        q["operations"] = json!(indexes.iter().map(|i| ops[*i].clone()).collect::<Vec<_>>());
        let actual = run_request(&serde_json::from_value(q).unwrap()).unwrap();
        assert_eq!(actual.results.len(), 8);
        for (a, i) in actual.results.iter().zip(indexes) {
            let b = &n["results"][i];
            assert_eq!(a.id, b["id"]);
            for stage in ["before", "after", "undo", "redo"] {
                crate::curve_getters::assert_close(
                    &a.value[stage],
                    &b["value"][if stage == "after" {
                        "after_script"
                    } else {
                        stage
                    }],
                    &format!("{}/{stage}", a.id),
                );
            }
            assert_eq!(a.value["undo_label"], "Circle");
            assert_eq!(a.value["redo_label"], "Circle");
        }
    }

    #[test]
    fn circle_deform_native_cancellations_have_no_output_or_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_controls.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_controls_rhino_20261017.json"
                ),
                4,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_clamped_controls.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_clamped_controls_rhino_20261017.json"
                ),
                1,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let actual = run_request(&q).unwrap();
            assert_eq!(actual.results.len(), count);
            for (a, b) in actual.results.iter().zip(n["results"].as_array().unwrap()) {
                for stage in ["before", "after", "undo", "redo"] {
                    crate::curve_getters::assert_close(
                        &a.value[stage],
                        &b["value"][if stage == "after" {
                            "after_script"
                        } else {
                            stage
                        }],
                        &a.id,
                    );
                }
                assert!(a.value["undo_label"].is_null());
                assert!(a.value["redo_label"].is_null());
            }
        }
    }

    #[test]
    fn circle_deform_native_controls_knots_domain_selection_and_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_degrees.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_degrees_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_default.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_default_rhino_20261017.json"
                ),
                1,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_modes_rhino_20261017.json"
                ),
                22,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_extended.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_extended_rhino_20261017.json"
                ),
                7,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_deform_boundaries.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_deform_boundaries_rhino_20261017.json"
                ),
                17,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_clamped_grid.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_clamped_grid_rhino_20261017.json"
                ),
                43,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_clamped_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_clamped_modes_rhino_20261017.json"
                ),
                14,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_clamped_precise.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_clamped_precise_rhino_20261017.json"
                ),
                2,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let actual = run_request(&q).unwrap();
            assert_eq!(actual.results.len(), count);
            for (a, b) in actual.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    crate::curve_getters::assert_close(
                        &a.value[stage],
                        &b["value"][if stage == "after" {
                            "after_script"
                        } else {
                            stage
                        }],
                        &format!("{}/{stage}", a.id),
                    );
                }
                assert_eq!(a.value["undo_label"], "Circle");
                assert_eq!(a.value["redo_label"], "Circle");
            }
        }
    }
}
