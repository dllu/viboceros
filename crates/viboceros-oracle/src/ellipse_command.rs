//! Ellipse point/numeric getters and complete document/history snapshots.
use super::*;
use crate::curve_getters::{Action, Fixture};
use viboceros_command::{
    curve_reference::{CapturedCurve, perpendicular_frame},
    ellipse_input::Input,
};
fn enter(
    doc: &mut Document,
    commands: &CommandRegistry,
    plane: Frame3,
    actions: &[Action],
    last_point: &mut Option<Point3>,
    guides: &[(ObjectId, Geometry)],
) -> Result<(), ProbeError> {
    let mut input = Input::new(commands.ellipse_options(), plane, doc.tolerance());
    let mut guide: Option<CapturedCurve> = None;
    let mut postselected = false;
    for action in actions {
        let previous = *last_point;
        let mut accept = |point: Point3| -> Result<(), ProbeError> {
            if let Some(curve) = &guide
                && input.requests_curve()
            {
                let frame = perpendicular_frame(curve.curve_ref(), point)?;
                input.around_curve(frame);
                *last_point = Some(frame.origin());
            } else {
                let point = input.resolve_point(point)?;
                input.point(point)?;
                *last_point = Some(point);
            }
            Ok(())
        };
        match action {
            Action::Click { .. } | Action::Hover { .. } => {
                return Err(ProbeError::FixtureInvariant(
                    "Ellipse requires explicit points or source picks",
                ));
            }
            Action::Point(p) => {
                let point = Point3::try_from(*p)?;
                accept(point)?;
            }
            Action::Pick { curve, edge, pick } => {
                if !input.requests_curve() {
                    return Err(ProbeError::FixtureInvariant(
                        "Ellipse is not selecting a curve",
                    ));
                }
                let (id, geometry) = guides
                    .get(*curve)
                    .ok_or(ProbeError::FixtureInvariant("missing Ellipse guide"))?;
                if let (Some(edge), Some(point)) = (edge, pick) {
                    let resolved = viboceros_command::curve_reference::resolve(
                        geometry,
                        Some(*edge),
                        doc.tolerance(),
                    )
                    .ok_or(ProbeError::FixtureInvariant("missing Ellipse edge"))?;
                    let p = Point3::try_from(*point)?;
                    let c = resolved.curve();
                    let t = c.closest_parameter(p, doc.tolerance())?;
                    if c.evaluate(t)?.distance_to(p)? > doc.tolerance().absolute() {
                        return Err(ProbeError::FixtureInvariant(
                            "Ellipse pick must lie on the named edge",
                        ));
                    }
                }
                guide = Some(CapturedCurve::capture(doc, *id, *edge)?);
                postselected = edge.is_none() && !doc.is_selected(*id);
                if edge.is_none() {
                    doc.select_object(*id, viboceros_document::SelectionMode::Add)?;
                }
            }
            Action::Number(value) => {
                if let Err(error) = input.number(*value) {
                    // Native integer getters reject invalid counts and keep
                    // waiting, allowing a later corrected count in the recipe.
                    if !input.point_count_pending() {
                        return Err(error.into());
                    }
                }
            }
            Action::Option(word) => {
                let prior_mode = input.mode();
                if !input.option(word)? {
                    return Err(ProbeError::FixtureInvariant("unsupported Ellipse option"));
                }
                if input.mode() != prior_mode {
                    if postselected && let Some(curve) = &guide {
                        doc.select_object(
                            curve.object_id(),
                            viboceros_document::SelectionMode::Remove,
                        )?;
                    }
                    guide = None;
                    postselected = false;
                }
                if input.requests_curve() && guide.is_none() {
                    let selected = doc.selected_object_ids().collect::<Vec<_>>();
                    if let [id] = selected.as_slice()
                        && doc
                            .object(*id)
                            .is_some_and(|o| o.geometry().curve_ref().is_some())
                    {
                        guide = Some(CapturedCurve::capture(doc, *id, None)?);
                        postselected = false;
                    }
                }
            }
            Action::Relative {
                relative: [x, y, z],
            } => {
                let point = viboceros_drafting::PointInput::parse(&format!("r{x},{y},{z}"))
                    .unwrap()
                    .map_err(|_| ProbeError::FixtureInvariant("invalid relative ellipse point"))?
                    .resolve(plane, previous)
                    .map_err(|_| {
                        ProbeError::FixtureInvariant("relative ellipse point requires a base")
                    })?;
                accept(point)?;
            }
        }
    }
    let command = if let Some(guide) = guide {
        input.command_with_guide(guide.object_id(), guide.source().1, postselected)?
    } else {
        input.command()?
    };
    commands.execute(doc, &command)?;
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
    use crate::curve_getters::assert_close as close;
    #[test]
    fn ellipse_around_curve_native_geometry_source_selection_and_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_around_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_around_command_rhino_20261017.json"
                ),
                8,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_around_extended.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_around_extended_rhino_20261017.json"
                ),
                19,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let actual = run_request(&q).unwrap();
            assert_eq!(actual.results.len(), count);
            for (a, b) in actual.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    close(
                        &a.value[stage],
                        &b["value"][if stage == "after" {
                            "after_script"
                        } else {
                            stage
                        }],
                        &format!("{}/{stage}", a.id),
                    );
                }
                assert_eq!(a.value["undo_label"], "Ellipse");
                assert_eq!(a.value["redo_label"], "Ellipse");
            }
        }
    }
    #[test]
    fn ellipse_deform_native_cubic_point_counts_geometry_and_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_deform_shapes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_deform_shapes_rhino_20261017.json"
                ),
                9,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_deform_modes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_deform_modes_rhino_20261017.json"
                ),
                20,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_deform_counts.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_deform_counts_rhino_20261017.json"
                ),
                4,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let local = run_request(&q).unwrap();
            assert_eq!(local.results.len(), count);
            for (a, b) in local.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    close(
                        &a.value[stage],
                        &b["value"][if stage == "after" {
                            "after_script"
                        } else {
                            stage
                        }],
                        &format!("{}/{stage}", a.id),
                    );
                }
                assert_eq!(a.value["undo_label"], "Ellipse");
                assert_eq!(a.value["redo_label"], "Ellipse");
            }
        }
    }
    #[test]
    fn ellipse_native_construction_axes_focus_markers_and_actual_history() {
        for (q, n, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_command_rhino_20261017.json"
                ),
                16,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_directions.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_directions_rhino_20261017.json"
                ),
                18,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_final.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_final_rhino_20261017.json"
                ),
                11,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_locks.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_locks_rhino_20261017.json"
                ),
                5,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/ellipse_planes.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/ellipse_planes_rhino_20261017.json"
                ),
                5,
            ),
        ] {
            let q: ProbeRequest = serde_json::from_str(q).unwrap();
            let n: Value = serde_json::from_str(n).unwrap();
            let local = run_request(&q).unwrap();
            assert_eq!(local.results.len(), count);
            for (a, b) in local.results.iter().zip(n["results"].as_array().unwrap()) {
                assert_eq!(a.id, b["id"]);
                for stage in ["before", "after", "undo", "redo"] {
                    close(
                        &a.value[stage],
                        &b["value"][stage],
                        &format!("{}/{stage}", a.id),
                    );
                }
                assert_eq!(a.value["undo_label"], "Ellipse");
                assert_eq!(a.value["redo_label"], "Ellipse");
            }
        }
    }
}
