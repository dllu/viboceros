//! Command-boundary probes executed after the Rhino setup script has returned.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Step {
    Select { objects: Vec<usize> },
    Recall { deselect_others: Option<bool> },
    Undo,
    Redo,
    Delete,
    Layer,
}

pub(super) fn run(
    document: &mut Document,
    registry: &CommandRegistry,
    ids: &[ObjectId],
    steps: &[Step],
) -> Result<Value, ProbeError> {
    let mut records = vec![snapshot(document, ids)];
    for (number, step) in steps.iter().enumerate() {
        match step {
            Step::Select { objects } => {
                if objects.iter().any(|i| *i >= ids.len())
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                {
                    return Err(ProbeError::FixtureInvariant(
                        "invalid last-selection indices",
                    ));
                }
                document.select_objects_direct(
                    objects.iter().map(|i| ids[*i]),
                    SelectionMode::Replace,
                )?;
            }
            Step::Recall { deselect_others } => {
                let command = deselect_others
                    .map(|value| {
                        format!(
                            "SelLast DeselectOthersBeforeSelect={}",
                            if value { "Yes" } else { "No" }
                        )
                    })
                    .unwrap_or_else(|| "SelLast".into());
                registry.execute(document, &command)?;
            }
            Step::Layer => {
                document.add_layer(format!("Last layer {number}"), ColorRgb::new(0, 0, 0))?;
            }
            Step::Undo | Step::Redo | Step::Delete => {
                registry.execute(document, &format!("{step:?}"))?;
            }
        }
        records.push(snapshot(document, ids));
    }
    Ok(json!(records))
}

fn snapshot(document: &Document, ids: &[ObjectId]) -> Value {
    json!(ids.iter().map(|id| document.object(*id).map(|object| {
        let Geometry::Line(line) = object.geometry() else { unreachable!() };
        let point = line.start();
        let attributes = object.attributes();
        let layer = document.layer(attributes.layer_id()).unwrap();
        json!({"point": [point.x(), point.y(), point.z()], "selected": document.is_selected(*id),
            "mode": if attributes.is_locked() { "Locked" } else if !attributes.is_visible() { "Hidden" } else { "Normal" },
            "layer_visible": layer.is_visible(), "layer_locked": layer.is_locked()})
    })).collect::<Vec<_>>())
}
