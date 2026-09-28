//! Native side of Rhino's owned-viewport mesh decomposition probes.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MeshPickingFixture {
    groups: Vec<Vec<usize>>,
    seed: usize,
    #[serde(default)]
    reverse_bridge: bool,
    #[serde(default)]
    locked: Vec<usize>,
    #[serde(default)]
    hidden: Vec<usize>,
    layer_mode: Option<String>,
    #[serde(default)]
    connected: Vec<usize>,
    #[serde(default)]
    history: bool,
    #[serde(default, rename = "move")]
    move_objects: bool,
    #[serde(default)]
    recall_previous: bool,
    #[serde(default)]
    recall_last: bool,
    #[serde(default)]
    last_steps: Vec<Value>,
    add_to_group_sources: Option<Vec<usize>>,
}

pub(super) fn run(
    fixture: &MeshPickingFixture,
    tolerance: Tolerance,
    explode: bool,
) -> Result<(Value, u64), ProbeError> {
    let valid = |indices: &[usize]| {
        indices.iter().all(|index| *index < 3)
            && indices.iter().collect::<BTreeSet<_>>().len() == indices.len()
    };
    if fixture.seed >= 3
        || fixture.groups.len() > 16
        || !fixture.groups.iter().all(|group| valid(group))
        || !valid(&fixture.locked)
        || !valid(&fixture.hidden)
        || !valid(&fixture.connected)
        || fixture
            .locked
            .iter()
            .any(|index| fixture.hidden.contains(index))
        || !matches!(
            fixture.layer_mode.as_deref(),
            None | Some("locked" | "hidden")
        )
        || fixture.move_objects
        || fixture.recall_previous
        || fixture.recall_last
        || !fixture.last_steps.is_empty()
        || fixture
            .add_to_group_sources
            .as_ref()
            .is_some_and(|sources| !sources.is_empty())
    {
        return Err(ProbeError::FixtureInvariant("invalid mesh picking case"));
    }

    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let mut ids = Vec::with_capacity(3);
    for source in 0..3 {
        let vertices = [[0., 0.], [2., 0.], [0., 2.], [0., 4.], [2., 4.], [0., 6.]]
            .map(|[x, y]| Point3::try_new(source as f64 * 5. + x, y, 0.))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let second = if fixture.connected.contains(&source) {
            [1, 3, 2]
        } else {
            [3, 4, 5]
        };
        let mesh = TriangleMesh::try_new(vertices, vec![[0, 1, 2], second], tolerance)?;
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name(format!("source-{source}"));
        ids.push(document.add_geometry_with_attributes(Geometry::Mesh(mesh), attributes)?);
    }
    let mut groups = Vec::with_capacity(fixture.groups.len());
    for members in &fixture.groups {
        groups.push(document.add_group(None, members.iter().map(|index| ids[*index]))?);
    }
    if fixture.reverse_bridge {
        let reversed = document
            .object(ids[1])
            .unwrap()
            .group_ids()
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        document.set_object_group_memberships(ids[1], reversed)?;
    }
    if let Some(mode) = fixture.layer_mode.as_deref() {
        let layer = document.add_layer("Restricted source", ColorRgb::BLACK)?;
        document.set_objects_layer([ids[1]], layer)?;
        document.set_layer_locked(layer, mode == "locked")?;
        document.set_layer_visibility(layer, mode != "hidden")?;
    }
    document.set_objects_locked(fixture.locked.iter().map(|index| ids[*index]), true)?;
    document.set_objects_visibility(fixture.hidden.iter().map(|index| ids[*index]), false)?;
    if document.is_object_selectable(ids[fixture.seed]) {
        document.select_object(ids[fixture.seed], SelectionMode::Replace)?;
    }

    let selected = ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
        .collect::<Vec<_>>();
    let mut value = json!({
        "selected": selected,
        "modes": ids.iter().map(|id| object_mode(&document, *id)).collect::<Vec<_>>(),
        "layers": ids.iter().map(|id| {
            let layer = document.layer(document.object(*id).unwrap().attributes().layer_id()).unwrap();
            json!({"visible": layer.is_visible(), "locked": layer.is_locked()})
        }).collect::<Vec<_>>()
    });
    let success_key = if explode {
        "explode_succeeded"
    } else {
        "split_succeeded"
    };
    value[success_key] = if selected.is_empty() {
        Value::Null
    } else {
        registry.execute(
            &mut document,
            if explode {
                "Explode"
            } else {
                "SplitDisjointMesh"
            },
        )?;
        json!(true)
    };
    value["outputs"] = json!(output_records(&document, &ids, &groups));
    if fixture.history {
        let mut history = Vec::with_capacity(2);
        for command in ["Undo", "Redo"] {
            registry.execute(&mut document, command)?;
            history.push(history_selection(&document, &ids));
        }
        value["history_selection"] = json!(history);
    }
    Ok((value, 0))
}

fn object_mode(document: &Document, id: ObjectId) -> &'static str {
    let attributes = document.object(id).unwrap().attributes();
    if !attributes.is_visible() {
        "Hidden"
    } else if attributes.is_locked() {
        "Locked"
    } else {
        "Normal"
    }
}

fn output_records(
    document: &Document,
    originals: &[ObjectId],
    groups: &[viboceros_document::GroupId],
) -> Vec<Value> {
    let mut records = document
        .objects()
        .map(|object| {
            let Geometry::Mesh(mesh) = object.geometry() else {
                unreachable!("mesh decomposition probe only creates meshes")
            };
            let attributes = object.attributes();
            let layer = document.layer(attributes.layer_id()).unwrap();
            let source = attributes.name().unwrap().to_owned();
            let vertices = mesh
                .vertices()
                .iter()
                .map(|point| [point.x(), point.y(), point.z()])
                .collect::<Vec<_>>();
            let record = json!({
                "source": source,
                "original_identity": originals.contains(&object.id()),
                "selected": document.is_selected(object.id()),
                "mode": object_mode(document, object.id()),
                "layer_visible": layer.is_visible(),
                "layer_locked": layer.is_locked(),
                "groups": object.group_ids().iter().map(|group| groups.iter().position(|candidate| candidate == group).unwrap()).collect::<Vec<_>>(),
                "faces": mesh.face_count(),
                "vertices": vertices,
            });
            (source, vertices, record)
        })
        .collect::<Vec<_>>();
    records.sort_by(|a, b| {
        a.0.cmp(&b.0).then_with(|| {
            a.1.iter()
                .flatten()
                .zip(b.1.iter().flatten())
                .map(|(x, y)| x.total_cmp(y))
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| a.1.len().cmp(&b.1.len()))
        })
    });
    records.into_iter().map(|(_, _, record)| record).collect()
}

fn history_selection(document: &Document, originals: &[ObjectId]) -> Vec<Value> {
    let mut records = document
        .objects()
        .map(|object| {
            (
                object.attributes().name().unwrap().to_owned(),
                originals.contains(&object.id()),
                document.is_selected(object.id()),
            )
        })
        .collect::<Vec<_>>();
    records.sort();
    records
        .into_iter()
        .map(|(source, original_identity, selected)| {
            json!({"source": source, "original_identity": original_identity, "selected": selected})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_mesh_decomposition_picks_match_recorded_rhino() {
        for (request, observation) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/mesh_split_picking.json"),
                include_str!("../../../tools/rhino_oracle/observations/mesh_split_picking.json"),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/mesh_explode_picking.json"),
                include_str!("../../../tools/rhino_oracle/observations/mesh_explode_picking.json"),
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(request).unwrap();
            let observation: Value = serde_json::from_str(observation).unwrap();
            let response = run_request(&request).unwrap();
            let expected = observation["results"].as_array().unwrap();
            assert_eq!(response.results.len(), expected.len());
            for (actual, expected) in response.results.iter().zip(expected) {
                assert_eq!(actual.id, expected["id"]);
                crate::test_json::close(
                    &actual.value,
                    &expected["value"],
                    &actual.id,
                    1e-10,
                    1e-12,
                );
            }
        }
    }

    #[test]
    fn mesh_pick_rejects_invalid_indices_and_combined_commands() {
        let mut request: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/mesh_split_picking.json"
        ))
        .unwrap();
        request["operations"] = json!([request["operations"][0].clone()]);
        for (field, value) in [("connected", json!([3])), ("move", json!(true))] {
            request["operations"][0][field] = value;
            let parsed: ProbeRequest = serde_json::from_value(request.clone()).unwrap();
            assert!(matches!(
                run_request(&parsed),
                Err(ProbeError::FixtureInvariant("invalid mesh picking case"))
            ));
            request["operations"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
    }
}
