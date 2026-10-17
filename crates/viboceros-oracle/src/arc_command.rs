//! Replay public point/numeric Arc getters through the production command.
use super::*;
use crate::curve_getters::{Action, Fixture};
pub(super) fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last: &mut Option<Point3>,
    _guides: &[(ObjectId, Geometry)],
) -> Result<(), ProbeError> {
    let mut tokens = vec!["Arc".to_owned()];
    let mut input: Option<viboceros_command::arc_tilted_input::Input> = None;
    let mut center = None;
    let mut complete = false;
    let mut output = viboceros_command::arc_deform_input::Settings::new(commands.arc_deformation());
    for action in actions {
        if complete {
            return Err(ProbeError::FixtureInvariant("trailing Arc input"));
        }
        if output.pending().is_some() {
            match action {
                Action::Number(n) => {
                    let _ = output.number(*n);
                }
                Action::Option(w) if w.eq_ignore_ascii_case("Enter") => {
                    let _ = output.default_value();
                }
                Action::Option(w) if w.eq_ignore_ascii_case("Cancel") => return Ok(()),
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "Arc setting requires an integer",
                    ));
                }
            }
            continue;
        }
        if center.is_none()
            && let Action::Option(w) = action
            && output.option(w)?
        {
            continue;
        }
        if let Action::Hover { hover } = action {
            input
                .as_mut()
                .ok_or(ProbeError::FixtureInvariant(
                    "Arc hover requires tilted arc input",
                ))?
                .observe_cursor(Point3::try_from(*hover)?)?;
            continue;
        }
        let point = match action {
            Action::Point(p) | Action::Click { click: p } => Some(Point3::try_from(*p)?),
            Action::Relative {
                relative: [x, y, z],
            } => Some(
                viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .unwrap()
                    .map_err(|_| ProbeError::FixtureInvariant("invalid relative Arc point"))?
                    .resolve(plane, *last)
                    .map_err(|_| ProbeError::FixtureInvariant("missing relative Arc base"))?,
            ),
            _ => None,
        };
        if let Some(p) = point {
            if let Some(i) = input.as_mut() {
                if matches!(action, Action::Click { .. }) {
                    i.observe_cursor(p)?;
                    tokens.push(format!(
                        "Direction={}",
                        if i.clockwise() {
                            "Clockwise"
                        } else {
                            "Counterclockwise"
                        }
                    ));
                }
                match viboceros_command::arc_tilted_input::Input::point(i, p) {
                    Ok(result) => complete = result.is_some(),
                    Err(_) => continue, // Native point getters reject and reprompt.
                }
            } else {
                center = Some(p);
            }
            *last = Some(p);
            let [x, y, z] = p.to_array();
            tokens.push(format!("{x},{y},{z}"));
            continue;
        }
        match action {
            Action::Option(w) if w.eq_ignore_ascii_case("Cancel") => return Ok(()),
            Action::Option(w) if w.eq_ignore_ascii_case("Tilted") => {
                input = Some(viboceros_command::arc_tilted_input::Input::new(
                    center.ok_or(ProbeError::FixtureInvariant("Tilted requires a center"))?,
                    doc.tolerance(),
                ));
                let [x, y, z] = center.unwrap().to_array();
                tokens = vec![
                    "Arc".into(),
                    "Center".into(),
                    format!("{x},{y},{z}"),
                    "Tilted".into(),
                ];
            }
            Action::Option(w) => {
                if let Some(i) = input.as_mut()
                    && !i.option(w)
                {
                    return Err(ProbeError::FixtureInvariant(
                        "unsupported tilted Arc option",
                    ));
                }
                tokens.push(w.clone());
            }
            Action::Number(n) => {
                if let Some(i) = input.as_mut() {
                    match i.number(*n) {
                        Ok(result) => {
                            complete = result.is_some();
                            if result.is_none() {
                                tokens.push(format!("Height={n}"));
                                continue;
                            }
                        }
                        Err(_) => continue,
                    }
                }
                tokens.push(n.to_string());
            }
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "Arc construction requires typed points or numbers",
                ));
            }
        }
    }
    if input.is_some() && !complete {
        return Err(ProbeError::FixtureInvariant("incomplete tilted Arc input"));
    }
    commands.execute_in_context(
        doc,
        &if output.output().deformable {
            output.output().wrap_command(&tokens.join(" "))
        } else {
            tokens.join(" ")
        },
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
    fn arc_deformable_and_tilted_native_controls_knots_domains_and_actual_history() {
        for (q, n) in [
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_deform_midpoint_qualified.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_midpoint_qualified_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_cursor.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_cursor_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_basic.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_basic_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_ordered.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_ordered_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_constructions.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_constructions_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_deform_settings.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_deform_settings_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_initial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_initial_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_modes_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_endings.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_endings_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_spatial_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_tilted_cursor.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_tilted_cursor_rhino_20261017.json"
                ),
            ),
        ] {
            let mut q: Value = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            for (op, row) in q["operations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(n["results"].as_array().unwrap())
            {
                if let Some(clicks) = row["value"]["input_trace"]["clicks"].as_array() {
                    for c in clicks {
                        op["actions"][c["action"].as_u64().unwrap() as usize]["click"] =
                            c["projected_point"].clone();
                    }
                }
                if let Some(motion) = row["value"]["input_trace"]["motion"].as_array() {
                    for c in motion {
                        op["actions"][c["action"].as_u64().unwrap() as usize]["hover"] =
                            c["projected_point"].clone();
                    }
                }
                let first = row["value"]["input_trace"]["clicks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(
                        row["value"]["input_trace"]["motion"]
                            .as_array()
                            .into_iter()
                            .flatten(),
                    )
                    .filter(|c| c["initial_projected_point"].is_array())
                    .min_by_key(|c| c["action"].as_u64().unwrap());
                if let Some(c) = first {
                    op["actions"].as_array_mut().unwrap().insert(
                        c["action"].as_u64().unwrap() as usize,
                        json!({"hover":c["initial_projected_point"]}),
                    );
                }
            }
            let a = run_request(&serde_json::from_value(q).unwrap()).unwrap();
            for (a, b) in a.results.iter().zip(n["results"].as_array().unwrap()) {
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
                let label = if b["value"]["after_script"]["outputs"]
                    .as_array()
                    .unwrap()
                    .is_empty()
                {
                    Value::Null
                } else {
                    json!("Arc")
                };
                assert_eq!(a.value["undo_label"], label);
                assert_eq!(a.value["redo_label"], label);
            }
        }
    }
}
