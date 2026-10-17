//! Replay public tangent-circle contacts without selecting or changing guides.
use super::*;
use crate::curve_getters::{Action, Fixture};
use viboceros_command::circle_tangent_input::Input;

pub(super) fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last: &mut Option<Point3>,
    guides: &[(ObjectId, Geometry)],
    arc: bool,
) -> Result<(), ProbeError> {
    if arc && !matches!(actions.first(),Some(Action::Option(w))if w.eq_ignore_ascii_case("Tangent"))
    {
        let mut tokens = vec!["Arc".to_owned()];
        for action in actions {
            let point = match action {
                Action::Point(p) => Some(Point3::try_from(*p)?),
                Action::Relative {
                    relative: [x, y, z],
                } => Some(
                    viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                        .unwrap()
                        .map_err(|_| ProbeError::FixtureInvariant("invalid arc setup point"))?
                        .resolve(plane, *last)
                        .map_err(|_| ProbeError::FixtureInvariant("missing arc setup base"))?,
                ),
                Action::Option(w) => {
                    tokens.push(w.clone());
                    None
                }
                Action::Number(n) => {
                    tokens.push(n.to_string());
                    None
                }
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "unsupported arc setup reference",
                    ));
                }
            };
            if let Some(p) = point {
                let [x, y, z] = p.to_array();
                tokens.push(format!("{x},{y},{z}"));
                *last = Some(p);
            }
        }
        commands.execute_in_context(
            doc,
            &tokens.join(" "),
            viboceros_command::CommandContext {
                construction_plane: plane,
            },
        )?;
        return Ok(());
    }
    let preferences = if arc {
        viboceros_command::circle_around_input::Preferences {
            radius: commands.arc_tangent_radius(),
            ..Default::default()
        }
    } else {
        commands.circle_preferences()
    };
    let mut input = Input::new(plane, doc.tolerance(), preferences);
    let mut output = viboceros_command::circle_deform_input::Input::new(
        plane,
        commands.circle_preferences(),
        doc.tolerance(),
    );
    let mut tangent = false;
    let mut completed = None;
    let mut choice = None;
    for action in actions {
        if completed.is_some() {
            if !arc || choice.is_some() {
                return Err(ProbeError::FixtureInvariant("trailing tangent input"));
            }
            let point = match action {
                Action::Point(p) => Point3::try_from(*p)?,
                Action::Relative {
                    relative: [x, y, z],
                } => viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .unwrap()
                    .map_err(|_| ProbeError::FixtureInvariant("invalid arc choice"))?
                    .resolve(plane, *last)
                    .map_err(|_| ProbeError::FixtureInvariant("missing choice base"))?,
                Action::Option(w) if w.eq_ignore_ascii_case("Cancel") => return Ok(()),
                _ => return Err(ProbeError::FixtureInvariant("expected final arc choice")),
            };
            choice = Some(point);
            *last = Some(point);
            continue;
        }
        if !tangent {
            match action {
                Action::Option(w) if w.eq_ignore_ascii_case("Tangent") => tangent = true,
                Action::Option(w) if output.option(w)? => {}
                Action::Number(n) if output.integer_pending() => {
                    output.number(*n)?;
                }
                _ => return Err(ProbeError::FixtureInvariant("invalid Tangent prefix")),
            }
            continue;
        }
        completed = match action {
            Action::Option(w) if w.eq_ignore_ascii_case("Cancel") => return Ok(()),
            Action::Option(w) if w.eq_ignore_ascii_case("Enter") => {
                if !input.accepts_default() {
                    return Ok(());
                }
                input.enter()?
            }
            Action::Option(w) if input.option(w) => None,
            Action::Number(n) => input.number(*n)?,
            Action::Pick { curve, edge, pick } => {
                let id = guides
                    .get(*curve)
                    .ok_or(ProbeError::FixtureInvariant("missing tangent guide"))?
                    .0;
                let p = pick.ok_or(ProbeError::FixtureInvariant(
                    "tangent pick requires a location",
                ))?;
                input.curve(doc, id, *edge, Point3::try_from(p)?)?
            }
            Action::Point(p) => {
                let p = Point3::try_from(*p)?;
                let circle = input.point(p)?;
                *last = Some(p);
                circle
            }
            Action::Relative {
                relative: [x, y, z],
            } => {
                let p = viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .unwrap()
                    .map_err(|_| ProbeError::FixtureInvariant("invalid tangent point"))?
                    .resolve(plane, *last)
                    .map_err(|_| ProbeError::FixtureInvariant("missing relative point base"))?;
                let circle = input.point(p)?;
                *last = Some(p);
                circle
            }
            _ => return Err(ProbeError::FixtureInvariant("unsupported Tangent input")),
        };
    }
    completed.ok_or(ProbeError::FixtureInvariant("incomplete tangent circle"))?;
    let command = if arc {
        viboceros_command::arc_tangent_input::command(
            &input,
            choice.ok_or(ProbeError::FixtureInvariant("incomplete arc choice"))?,
        )
    } else {
        input.command()
    };
    let command = if output.output().deformable {
        output.output().wrap_command(&command)
    } else {
        command
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
    crate::curve_getters::run(f, iterations, tolerance, |d, c, p, a, l, g| {
        enter(d, c, p, a, l, g, false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_tangent_native_contacts_full_curve_selection_and_history() {
        for (q, n) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_getters.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_getters_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_special.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_special_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_curves.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_curves_rhino_20261017.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_spatial_rhino_20261017.json"
                ),
            ),
        ] {
            let mut q: Value = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            // Replay the INPUT locations resolved from native viewport rays.
            // No expected circle geometry participates in calibration. Pixel
            // aperture/viewport parity is a separate UI claim.
            for (op, row) in q["operations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(n["results"].as_array().unwrap())
            {
                if let Some(clicks) = row["value"]["input_trace"]["clicks"].as_array() {
                    for c in clicks {
                        let i = c["action"].as_u64().unwrap() as usize;
                        assert!(c["projected_point"].is_array());
                        op["actions"][i]["pick"] = c["projected_point"].clone();
                    }
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
                    json!("Circle")
                };
                assert_eq!(a.value["undo_label"], label);
                assert_eq!(a.value["redo_label"], label);
            }
        }
    }
}
