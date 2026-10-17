//! Ellipse point/numeric getters and complete document/history snapshots.
use super::*;
use viboceros_command::{
    curve_reference::{CapturedCurve, perpendicular_frame},
    ellipse_input::Input,
};
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Action {
    Point([f64; 3]),
    Number(f64),
    Option(String),
    Relative {
        relative: [f64; 3],
    },
    Pick {
        curve: usize,
        #[serde(default)]
        edge: Option<usize>,
        #[serde(default)]
        pick: Option<[f64; 3]>,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub actions: Vec<Action>,
    #[serde(default)]
    pub construction_plane: Option<[[f64; 3]; 3]>,
    #[serde(default)]
    pub setup: Option<Vec<Action>>,
    #[serde(default)]
    pub prior_point: Option<[f64; 3]>,
    #[serde(default)]
    pub curves: Vec<crate::object_source::ObjectSource>,
    #[serde(default)]
    pub preselect: bool,
    #[serde(default)]
    pub guide_selection: bool,
}
impl Fixture {
    pub fn source_geometries(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
        self.curves
            .iter()
            .map(|source| source.geometry(tolerance))
            .collect()
    }
}
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
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let plane = if let Some([o, x, y]) = f.construction_plane {
            Frame3::try_from_directions(
                Point3::try_from(o)?,
                Vector3::try_from(x)?,
                Vector3::try_from(y)?,
                tolerance,
            )?
        } else {
            viboceros_command::CommandContext::default().construction_plane
        };
        let guides = f
            .source_geometries(tolerance)?
            .into_iter()
            .map(|g| {
                let id = doc.add_geometry(g.clone())?;
                Ok((id, g))
            })
            .collect::<Result<Vec<_>, ProbeError>>()?;
        if f.preselect
            && let Some((id, _)) = guides.first()
        {
            doc.select_object(*id, viboceros_document::SelectionMode::Replace)?;
        }
        let mut last = f.prior_point.map(Point3::try_from).transpose()?;
        if let Some(actions) = &f.setup {
            enter(&mut doc, &commands, plane, actions, &mut last, &guides)?;
            let ids = doc
                .objects()
                .filter(|o| !guides.iter().any(|(id, _)| *id == o.id()))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            doc.delete_objects(ids)?;
        }
        doc.clear_history()?;
        let snapshot = |d: &Document| -> Result<Value, ProbeError> {
            let mut value = crate::curve_primitive::snapshot(d, &guides)?;
            if f.guide_selection {
                value["guide_selected"] = json!(
                    guides
                        .iter()
                        .map(|(id, _)| d.is_selected(*id))
                        .collect::<Vec<_>>()
                );
            }
            Ok(value)
        };
        let before = snapshot(&doc)?;
        enter(&mut doc, &commands, plane, &f.actions, &mut last, &guides)?;
        let after = snapshot(&doc)?;
        let undo_label = doc.undo()?;
        let undo = snapshot(&doc)?;
        let redo_label = doc.redo()?;
        let redo = snapshot(&doc)?;
        Ok(
            json!({"command_returned":true,"before":before,"after":after,"undo":undo,"redo":redo,"undo_label":undo_label,"redo_label":redo_label}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: &Value, b: &Value, path: &str) {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6,
                "{path}: {a} != {b}"
            ),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    close(a, b, &format!("{path}/{i}"));
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (k, a) in a {
                    close(a, b.get(k).unwrap(), &format!("{path}/{k}"));
                }
            }
            _ => assert_eq!(a, b, "{path}"),
        }
    }
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
