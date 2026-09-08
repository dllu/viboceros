//! Matches the dedicated Rhino idle-viewport worker's three-line cases.
use super::*;
mod last_selection;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct GroupPickingFixture {
    groups: Vec<Vec<usize>>,
    seed: usize,
    #[serde(default)]
    reverse_bridge: bool,
    #[serde(default)]
    locked: Vec<usize>,
    #[serde(default)]
    hidden: Vec<usize>,
    layer_mode: Option<String>,
    #[serde(default, rename = "move")]
    move_objects: bool,
    #[serde(default)]
    recall_previous: bool,
    #[serde(default)]
    recall_last: bool,
    #[serde(default)]
    last_steps: Vec<last_selection::Step>,
}

pub(super) fn run(
    f: &GroupPickingFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let valid = |indices: &[usize]| {
        indices.iter().all(|i| *i < 3)
            && indices.iter().collect::<BTreeSet<_>>().len() == indices.len()
    };
    if f.seed >= 3
        || f.groups.len() > 16
        || !f.groups.iter().all(|group| valid(group))
        || !valid(&f.locked)
        || !valid(&f.hidden)
        || f.locked.iter().any(|i| f.hidden.contains(i))
        || !matches!(f.layer_mode.as_deref(), None | Some("locked" | "hidden"))
        || (f.recall_last
            && (!f.move_objects
                || f.recall_previous
                || f.hidden.contains(&f.seed)
                || f.locked.contains(&f.seed)
                || (f.seed == 1 && f.layer_mode.is_some())))
        || f.last_steps.len() > 32
        || (!f.last_steps.is_empty() && !f.recall_last)
    {
        return Err(ProbeError::FixtureInvariant("invalid group picking case"));
    }
    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let ids = (0..3)
        .map(|i| {
            Ok(document.add_geometry(Geometry::Line(LineSegment::try_new(
                Point3::try_new(i as f64 * 5.0, 0.0, 0.0)?,
                Point3::try_new(i as f64 * 5.0, 2.0, 0.0)?,
                tolerance,
            )?))?)
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    for group in &f.groups {
        let id = document.add_empty_group(None)?;
        document.add_group_members(id, group.iter().map(|i| ids[*i]))?;
    }
    if f.reverse_bridge {
        let groups = document
            .object(ids[1])
            .unwrap()
            .group_ids()
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        document.set_object_group_memberships(ids[1], groups)?;
    }
    if let Some(mode) = &f.layer_mode {
        let layer = document.add_layer("Bridge", ColorRgb::new(0, 0, 0))?;
        document.set_objects_layer([ids[1]], layer)?;
        if mode == "locked" {
            document.set_layer_locked(layer, true)?;
        } else {
            document.set_layer_visibility(layer, false)?;
        }
    }
    document.set_objects_locked(f.locked.iter().map(|i| ids[*i]), true)?;
    document.set_objects_visibility(f.hidden.iter().map(|i| ids[*i]), false)?;
    // A hidden/locked line cannot be hit by a mouse pick; this is not SelID.
    if document.is_object_selectable(ids[f.seed]) {
        document.select_object(ids[f.seed], SelectionMode::Replace)?;
    }
    let mut value = json!({
        "selected": ids.iter().enumerate().filter(|(_, id)| document.is_selected(**id)).map(|(i, _)| i).collect::<Vec<_>>(),
        "modes": ids.iter().map(|id| {
            let attributes = document.object(*id).unwrap().attributes();
            if attributes.is_locked() { "Locked" } else if !attributes.is_visible() { "Hidden" } else { "Normal" }
        }).collect::<Vec<_>>(),
        "layers": ids.iter().map(|id| {
            let layer = document.layer(document.object(*id).unwrap().attributes().layer_id()).unwrap();
            json!({"visible": layer.is_visible(), "locked": layer.is_locked()})
        }).collect::<Vec<_>>()
    });
    if f.move_objects {
        value["move_succeeded"] = if document.selected_object_count() == 0 {
            Value::Null
        } else {
            registry.execute(&mut document, "Move 0,0,0 0,1,0")?;
            json!(true)
        };
        value["points"] = json!(
            ids.iter()
                .map(|id| {
                    let Geometry::Line(line) = document.object(*id).unwrap().geometry() else {
                        unreachable!()
                    };
                    let point = line.start();
                    [point.x(), point.y(), point.z()]
                })
                .collect::<Vec<_>>()
        );
    }
    if f.recall_previous {
        document.clear_selection();
        document.select_previous(true);
        value["selected"] = json!(
            ids.iter()
                .enumerate()
                .filter(|(_, id)| document.is_selected(**id))
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        );
    }
    if f.recall_last {
        document.clear_selection();
        registry.execute(&mut document, "SelLast DeselectOthersBeforeSelect=Yes")?;
        value["selected"] = json!(
            ids.iter()
                .enumerate()
                .filter(|(_, id)| document.is_selected(**id))
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        );
    }
    if !f.last_steps.is_empty() {
        value["last_states"] = last_selection::run(&mut document, &registry, &ids, &f.last_steps)?;
    }
    Ok((value, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_and_move_history_match_complete_rhino_traces() {
        for (fixture, reference, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/deletion_recall.json"),
                include_str!("../../../tools/rhino_oracle/observations/deletion_recall.json"),
                54,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/last_selection_history.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/last_selection_history.json"
                ),
                16,
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
            let observed: Value = serde_json::from_str(reference).unwrap();
            let response = run_request(&request).unwrap();
            let rows = observed["results"].as_array().unwrap();
            assert_eq!(rows.len(), count);
            assert_eq!(response.results.len(), count);
            for (actual, expected) in response.results.iter().zip(rows) {
                assert_eq!(actual.id, expected["id"].as_str().unwrap());
                assert_eq!(actual.value, expected["value"], "{}", actual.id);
            }
        }
    }

    #[test]
    fn last_selection_after_idle_move_matches_recorded_rhino() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/last_selection.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/last_selection.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let rows = observed["results"].as_array().unwrap();
        assert_eq!(rows.len(), 32);
        assert_eq!(response.results.len(), rows.len());
        for (actual, expected) in response.results.iter().zip(rows) {
            assert_eq!(actual.id, expected["id"].as_str().unwrap());
            assert_eq!(actual.value, expected["value"], "{}", actual.id);
        }
    }

    #[test]
    fn recall_after_real_group_picking_matches_recorded_rhino() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/selection_recall_picking.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/selection_recall_picking.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let rows = observed["results"].as_array().unwrap();
        assert_eq!(rows.len(), 52);
        assert_eq!(response.results.len(), rows.len());
        for (actual, expected) in response.results.iter().zip(rows) {
            assert_eq!(actual.id, expected["id"].as_str().unwrap());
            assert_eq!(actual.value, expected["value"], "{}", actual.id);
        }
    }

    #[test]
    fn recorded_rhino_idle_clicks_and_preselected_move_match_exactly() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/group_picking.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/group_picking.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let records = observed["results"].as_array().unwrap();
        assert_eq!(records.len(), 52);
        assert_eq!(response.results.len(), records.len());
        for (actual, expected) in response.results.iter().zip(records) {
            assert_eq!(actual.id, expected["id"].as_str().unwrap());
            assert_eq!(actual.value, expected["value"], "{}", actual.id);
        }
    }

    #[test]
    fn invalid_group_picking_cases_are_rejected() {
        let base = json!({"groups": [[0,1], [1,2]], "seed": 0});
        for (key, value) in [
            ("seed", json!(3)),
            ("seed", json!(true)),
            ("groups", json!([[0, 0]])),
            ("groups", json!([[3]])),
            ("groups", json!([[true]])),
            ("locked", json!([1, 1])),
            ("layer_mode", json!("other")),
            ("move", json!("Yes")),
            ("recall_last", json!(true)),
            ("recall_last", json!("Yes")),
            ("last_steps", json!([{"kind": "recall"}])),
        ] {
            let mut invalid = base.clone();
            invalid[key] = value;
            if let Ok(fixture) = serde_json::from_value::<GroupPickingFixture>(invalid) {
                assert!(run(&fixture, Tolerance::default()).is_err());
            }
        }
        let fixture: GroupPickingFixture = serde_json::from_value(json!({
            "groups": [], "seed": 0, "locked": [1], "hidden": [1]
        }))
        .unwrap();
        assert!(run(&fixture, Tolerance::default()).is_err());
        for changes in [
            json!({"locked": [0]}),
            json!({"recall_previous": true}),
            json!({"last_steps": [{"kind": "select", "objects": [3]}]}),
            json!({"last_steps": [{"kind": "recall", "deselect_others": "Yes"}]}),
        ] {
            let mut value = base.clone();
            value["move"] = json!(true);
            value["recall_last"] = json!(true);
            value
                .as_object_mut()
                .unwrap()
                .extend(changes.as_object().unwrap().clone());
            if let Ok(fixture) = serde_json::from_value::<GroupPickingFixture>(value) {
                assert!(run(&fixture, Tolerance::default()).is_err());
            }
        }
    }
}
