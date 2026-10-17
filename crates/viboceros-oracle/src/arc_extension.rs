//! Replay Extension getters through the shared production input and Arc command.
use super::*;
use crate::curve_getters::{Action, Fixture};
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last: &mut Option<Point3>,
    guides: &[(ObjectId, Geometry)],
) -> Result<(), ProbeError> {
    let mut input = viboceros_command::arc_extension_input::Input::new(doc.tolerance());
    let mut settings =
        viboceros_command::arc_deform_input::Settings::new(commands.arc_deformation());
    let mut extension = false;
    let mut complete = false;
    for action in actions {
        if complete {
            return Err(ProbeError::FixtureInvariant("trailing Arc Extension input"));
        }
        if let Action::Option(w) = action
            && w.eq_ignore_ascii_case("Cancel")
        {
            if !input.wants_curve() {
                doc.clear_selection();
            }
            return Ok(());
        }
        if !extension {
            if let Action::Option(w) = action
                && w.eq_ignore_ascii_case("Extension")
            {
                extension = true;
                continue;
            }
            if settings.pending().is_some() {
                match action {
                    Action::Number(n) => {
                        let _ = settings.number(*n);
                    }
                    Action::Option(w) if w.eq_ignore_ascii_case("Enter") => {
                        let _ = settings.default_value();
                    }
                    _ => {
                        return Err(ProbeError::FixtureInvariant(
                            "Arc Extension setting requires an integer",
                        ));
                    }
                }
            } else if let Action::Option(w) = action
                && settings.option(w)?
            {
            } else {
                return Err(ProbeError::FixtureInvariant("Arc Extension prefix"));
            }
            continue;
        }
        let result = match action {
            Action::Pick { curve, edge, pick } => {
                input.curve(
                    doc,
                    guides
                        .get(*curve)
                        .ok_or(ProbeError::FixtureInvariant("missing extension source"))?
                        .0,
                    *edge,
                    Point3::try_from(
                        pick.ok_or(ProbeError::FixtureInvariant("missing extension pick"))?,
                    )?,
                )?;
                doc.select_object(guides[*curve].0, SelectionMode::Replace)?;
                *last = input.anchor();
                None
            }
            Action::Hover { hover } => {
                input.observe_cursor(Point3::try_from(*hover)?)?;
                None
            }
            Action::Point(p) | Action::Click { click: p } => {
                let p = input.resolve_point(Point3::try_from(*p)?)?;
                if matches!(action, Action::Click { .. }) {
                    input.observe_cursor(p)?;
                }
                *last = Some(p);
                input.point(p)?
            }
            Action::Relative {
                relative: [x, y, z],
            } => {
                let p = viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .unwrap()
                    .map_err(|_| ProbeError::FixtureInvariant("invalid relative extension point"))?
                    .resolve(plane, *last)
                    .map_err(|_| ProbeError::FixtureInvariant("missing relative extension base"))?;
                let p = input.resolve_point(p)?;
                *last = Some(p);
                input.point(p)?
            }
            Action::Number(n) => input.number(*n)?,
            Action::Option(w) if input.option(w) => None,
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "unsupported Arc Extension action",
                ));
            }
        };
        complete = result.is_some();
    }
    if !complete {
        return Err(ProbeError::FixtureInvariant("incomplete Arc Extension"));
    }
    commands.execute_in_context(
        doc,
        &settings.output().wrap_command(&input.command()),
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
    fn arc_extension_native_controls_knots_domains_guides_and_history() {
        for (q, n) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_relative.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_relative_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_selection.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_selection_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_end_choice.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_end_choice_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_sources.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_sources_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/arc_extension_basic.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_basic_rhino_20261017.json"
                ),
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_extension_center_numeric.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_center_numeric_rhino_20261017.json"
                ),
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/arc_extension_center_cursor.json"
                ),
                include_str!(
                    "../../../tools/rhino_oracle/observations/arc_extension_center_cursor_rhino_20261017.json"
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
                for c in row["value"]["input_trace"]["clicks"].as_array().unwrap() {
                    let i = c["action"].as_u64().unwrap() as usize;
                    let key = if op["actions"][i].get("curve").is_some() {
                        "pick"
                    } else {
                        "click"
                    };
                    op["actions"][i][key] = c["projected_point"].clone();
                }
            }
            for (op, row) in q["operations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(n["results"].as_array().unwrap())
            {
                if let Some(c) = row["value"]["input_trace"]["clicks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c.get("curve").is_none())
                {
                    op["actions"].as_array_mut().unwrap().insert(
                        c["action"].as_u64().unwrap() as usize,
                        json!({"hover":c["initial_projected_point"]}),
                    );
                }
            }
            let actual = run_request(&serde_json::from_value(q).unwrap()).unwrap();
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
