//! Replay the real directional selection commands against shared 3DM sources.
use super::*;
use viboceros_document::ControlPointId;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    sources: Vec<Source>,
    actions: Vec<Action>,
    artifact_path: PathBuf,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    kind: String,
    mode: String,
    parent: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Action {
    command: String,
    seed: Option<String>,
}

fn snapshot(doc: &Document, ids: &[ObjectId], peer: ObjectId) -> Value {
    json!({"sources":ids.iter().map(|id|json!({
        "selected":doc.is_selected(*id),
        "grips":doc.control_points().filter(|(g,_,_)|g.object==*id).map(|(g,p,s)|
            json!({"index":g.index,"point":p.to_array(),"selected":s})).collect::<Vec<_>>()
    })).collect::<Vec<_>>(),"peer_selected":doc.is_selected(peer)})
}

pub(super) fn run(f: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    if !(1..=4).contains(&f.sources.len()) || !(1..=96).contains(&f.actions.len()) {
        return Err(ProbeError::FixtureInvariant("grip navigation recipe size"));
    }
    for action in &f.actions {
        if !matches!(
            viboceros_command::point_selection::PointSelectionAction::from_input(&action.command)?,
            Some(viboceros_command::point_selection::PointSelectionAction::Navigate(_))
        ) {
            return Err(ProbeError::FixtureInvariant(
                "invalid grip navigation command",
            ));
        }
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, _, _, _) = viboceros_command::open_3dm_with_views(
            f.artifact_path
                .to_str()
                .ok_or(ProbeError::FixtureInvariant("navigation source path"))?,
        )?;
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        if ids.len() != f.sources.len() {
            return Err(ProbeError::FixtureInvariant("navigation source count"));
        }
        doc.clear_selection();
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        for (id, source) in ids.iter().zip(&f.sources) {
            match source.mode.as_str() {
                "control" => {
                    doc.enable_control_points([*id])?;
                }
                "edit" => {
                    doc.select_objects_direct([*id], SelectionMode::Add)?;
                    registry.execute(&mut doc, "EditPtOn")?;
                }
                _ => return Err(ProbeError::FixtureInvariant("invalid navigation grip mode")),
            }
        }
        let peer = doc.add_geometry(Geometry::Point(Point3::try_new(20., 20., 20.)?))?;
        let marker = doc.add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.)?))?;
        doc.undo()?;
        for (id, source) in ids.iter().zip(&f.sources) {
            if source.parent {
                doc.select_objects_direct([*id], SelectionMode::Add)?;
            }
        }
        doc.select_objects_direct([peer], SelectionMode::Add)?;
        let originals = ids
            .iter()
            .map(|id| doc.object(*id).unwrap().clone())
            .collect::<Vec<_>>();
        let before = snapshot(&doc, &ids, peer);
        let mut steps = Vec::new();
        for action in &f.actions {
            if let Some(seed) = &action.seed {
                let mut picks = Vec::new();
                for id in &ids {
                    let n = doc
                        .control_point_locations(*id)
                        .ok_or(ProbeError::FixtureInvariant("missing navigation grips"))?
                        .len();
                    let indices = match seed.as_str() {
                        "first" => vec![0],
                        "last" => vec![n - 1],
                        "middle" => vec![n / 2],
                        "ends" => vec![0, n - 1],
                        "all" => (0..n).collect(),
                        "empty" => Vec::new(),
                        _ => return Err(ProbeError::FixtureInvariant("invalid navigation seed")),
                    };
                    picks.extend(
                        indices
                            .into_iter()
                            .map(|index| ControlPointId { object: *id, index }),
                    );
                }
                doc.clear_control_point_selection();
                doc.select_control_points(picks, SelectionMode::Add)?;
            }
            let initial = snapshot(&doc, &ids, peer);
            let has_inputs = doc.selected_control_points().any(|(grip, _)| {
                viboceros_command::point_selection::Navigation::accepts(
                    doc.object(grip.object).unwrap().geometry(),
                )
            });
            registry.execute(&mut doc, &action.command)?;
            for (id, original) in ids.iter().zip(&originals) {
                if doc.object(*id) != Some(original) {
                    return Err(ProbeError::FixtureInvariant(
                        "navigation mutated source geometry or attributes",
                    ));
                }
            }
            steps.push(json!({"command":action.command,
                "macro":format!("_{}",action.command),
                "before":initial,"after":snapshot(&doc,&ids,peer),"result":if has_inputs {"Success"} else {"Nothing"}}));
        }
        doc.redo()?;
        Ok(json!({"before":before,"steps":steps,"redo_retained":doc.object(marker).is_some()}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directional_grip_selection_matches_complete_native_traces() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let mut request: ProbeRequest = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/grip_navigation.json")).unwrap(),
        )
        .unwrap();
        for operation in &mut request.operations {
            if let Operation::GripNavigation { fixture, .. } = operation {
                fixture.artifact_path = root
                    .join("fixtures/grip_navigation")
                    .join(fixture.artifact_path.file_name().unwrap());
            }
        }
        let expected: ProbeResponse = serde_json::from_str(
            &fs::read_to_string(root.join("observations/grip_navigation.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.engine, "rhino");
        assert_eq!(expected.results.len(), 25);
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), expected.results.len());
        let mut failures = Vec::new();
        for (a, b) in actual.results.iter().zip(&expected.results) {
            assert_eq!(a.id, b.id);
            if std::panic::catch_unwind(|| {
                crate::test_json::close(&a.value, &b.value, &a.id, 1e-8, 1e-12)
            })
            .is_err()
            {
                failures.push(a.id.clone());
            }
        }
        assert!(
            failures.is_empty(),
            "directional selection differences: {failures:?}"
        );
    }
}
