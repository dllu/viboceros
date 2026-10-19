//! Replay surface-row getter selection against native saved surfaces.
use super::*;
use viboceros_command::select_uv_rows::{Options, Session};
use viboceros_document::ControlPointId;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    actions: Vec<Value>,
    artifact_path: PathBuf,
    #[serde(default)]
    grips_on: bool,
    #[serde(default)]
    selected: Vec<usize>,
    #[serde(default)]
    preselected: bool,
    #[serde(default)]
    cancel_surface: bool,
    next_actions: Option<Vec<Value>>,
}
pub fn snapshot(doc: &Document, id: ObjectId, peer: ObjectId) -> Value {
    json!({"selected":doc.is_selected(id),"visible":doc.object(id).unwrap().attributes().is_visible(),
        "grips_on":doc.control_point_locations(id).is_some(),
        "grips":doc.control_points().filter(|(g,_,_)|g.object==id).map(|(g,p,s)|
            json!({"index":g.index,"point":p.to_array(),"selected":s,"hidden":false})).collect::<Vec<_>>(),
        "peer_selected":doc.is_selected(peer),"peer_visible":doc.object(peer).unwrap().attributes().is_visible()})
}
pub(super) fn run(f: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    if f.actions.len() > 24 || f.next_actions.as_ref().is_some_and(|a| a.len() > 24) {
        return Err(ProbeError::FixtureInvariant("SelUV recipe size"));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, _, _, _) = viboceros_command::open_3dm_with_views(
            f.artifact_path
                .to_str()
                .ok_or(ProbeError::FixtureInvariant("SelUV path"))?,
        )?;
        let id = doc
            .objects()
            .find(|o| viboceros_command::select_uv_rows::accepts(o.geometry()))
            .ok_or(ProbeError::FixtureInvariant("SelUV source"))?
            .id();
        let peer = doc
            .objects()
            .find(|o| matches!(o.geometry(), Geometry::Point(_)))
            .ok_or(ProbeError::FixtureInvariant("SelUV peer"))?
            .id();
        doc.clear_selection();
        doc.clear_history()?;
        if f.grips_on {
            doc.enable_control_points([id])?;
            doc.select_control_points(
                f.selected
                    .iter()
                    .map(|&index| ControlPointId { object: id, index }),
                SelectionMode::Add,
            )?;
        }
        let marker = doc.add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.)?))?;
        doc.undo()?;
        if f.preselected {
            doc.select_objects_direct([id], SelectionMode::Add)?;
        }
        doc.select_objects_direct([peer], SelectionMode::Add)?;
        let original = doc.object(id).unwrap().clone();
        let points = original
            .geometry()
            .grip_locations(viboceros_document::GripMode::Control)?;
        let before = snapshot(&doc, id, peer);
        let mut options = Options::default();
        let registry = CommandRegistry::with_builtins();
        let mut command = |doc: &mut Document,
                           actions: &[Value],
                           cancel_surface: bool|
         -> Result<Value, ProbeError> {
            if cancel_surface {
                doc.clear_selection();
            } else {
                let mut batch = doc.clone();
                let preselected = doc.is_selected(id);
                let mut words = vec!["SelUV Points".to_owned()];
                let session = Session::start(doc, id, doc.is_selected(id))?;
                for a in actions {
                    if let Some(word) = a.as_str() {
                        if !options.accept(word)? {
                            return Err(ProbeError::FixtureInvariant("SelUV option"));
                        }
                        words.push(word.into());
                    } else {
                        let point = if let Some(index) = a["grip"].as_u64() {
                            *points
                                .get(index as usize)
                                .ok_or(ProbeError::FixtureInvariant("SelUV index"))?
                        } else if a.get("parameter").is_some() {
                            let uv = serde_json::from_value::<[f64; 2]>(a["parameter"].clone())?;
                            viboceros_command::surface_reference::surface(original.geometry(), None)
                                .unwrap()
                                .0
                                .evaluate(uv[0], uv[1])?
                        } else {
                            Point3::try_from(serde_json::from_value::<[f64; 3]>(
                                a["world"].clone(),
                            )?)?
                        };
                        session.pick(doc, point, options)?;
                        words.push(format!("{},{},{}", point.x(), point.y(), point.z()));
                    }
                }
                let after = snapshot(doc, id, peer);
                session.finish(doc)?;
                batch.select_objects_direct([id], SelectionMode::Add)?;
                if preselected {
                    registry.execute(&mut batch, &words.join(" "))?;
                } else {
                    registry.execute_postselected(
                        &mut batch,
                        &words.join(" "),
                        Default::default(),
                    )?;
                }
                if snapshot(&batch, id, peer) != snapshot(doc, id, peer)
                    || batch.object(id) != Some(&original)
                    || !batch.can_redo()
                {
                    return Err(ProbeError::FixtureInvariant(
                        "SelUV batch/session selection or history differs",
                    ));
                }
                return Ok(after);
            }
            Ok(snapshot(doc, id, peer))
        };
        let after = command(&mut doc, &f.actions, f.cancel_surface)?;
        let mut value =
            json!({"before":before,"after":after,"after_script":snapshot(&doc,id,peer)});
        if let Some(actions) = &f.next_actions {
            let before = snapshot(&doc, id, peer);
            let after = command(&mut doc, actions, false)?;
            value["again"] =
                json!({"before":before,"after":after,"after_script":snapshot(&doc,id,peer)});
        }
        if doc.object(id) != Some(&original) {
            return Err(ProbeError::FixtureInvariant("SelUV mutated source"));
        }
        doc.redo()?;
        value["redo_retained"] = json!(doc.object(marker).is_some());
        Ok(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_native_seluv_selection_display_and_redo_states() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let q: Value = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/select_uv_rows.json")).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_str(
            &fs::read_to_string(root.join("observations/select_uv_rows.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(r["results"].as_array().unwrap().len(), 125);
        let mut failures = Vec::new();
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut fixture: Fixture = serde_json::from_value(op.clone()).unwrap();
            fixture.artifact_path = root
                .join("fixtures/select_uv_rows")
                .join(fixture.artifact_path.file_name().unwrap());
            let (actual, _) = run(&fixture, 1).unwrap();
            if std::panic::catch_unwind(|| {
                for key in ["before", "after", "after_script", "redo_retained"] {
                    crate::test_json::close(
                        &actual[key],
                        &row["value"][key],
                        row["id"].as_str().unwrap(),
                        1e-8,
                        1e-12,
                    );
                }
                if fixture.next_actions.is_some() {
                    for key in ["before", "after", "after_script"] {
                        crate::test_json::close(
                            &actual["again"][key],
                            &row["value"]["again"][key],
                            row["id"].as_str().unwrap(),
                            1e-8,
                            1e-12,
                        );
                    }
                }
            })
            .is_err()
            {
                failures.push(row["id"].clone());
            }
        }
        assert!(failures.is_empty(), "SelUV differences: {failures:?}");
    }
}
