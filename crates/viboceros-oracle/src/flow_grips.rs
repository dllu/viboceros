//! Complete Flow owner observations from authored control/edit grip recipes.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_document::{ControlPointId, GripMode};
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default = "yes")]
    pub copy: bool,
    #[serde(default)]
    pub rigid: bool,
    #[serde(default = "yes")]
    pub rigid_groups: bool,
    #[serde(default)]
    pub stretch: bool,
    #[serde(default)]
    pub preserve_structure: bool,
    #[serde(default)]
    pub reverse_base: bool,
    #[serde(default)]
    pub reverse_target: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            copy: true,
            rigid: false,
            rigid_groups: true,
            stretch: false,
            preserve_structure: false,
            reverse_base: false,
            reverse_target: false,
        }
    }
}
impl Options {
    pub fn command(&self) -> viboceros_command::flow::Options {
        viboceros_command::flow::Options {
            copy: self.copy,
            rigid: self.rigid,
            rigid_groups: self.rigid_groups,
            stretch: self.stretch,
            preserve_structure: self.preserve_structure,
            reverse_base: self.reverse_base,
            reverse_target: self.reverse_target,
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub base: ObjectSource,
    pub target: ObjectSource,
    pub source: ObjectSource,
    pub selected: Vec<usize>,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub parent: bool,
    #[serde(default)]
    pub peer: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub postselected: bool,
}
impl Fixture {
    pub fn source_geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        self.source.geometry(tolerance)
    }
    pub fn guides(&self, tolerance: Tolerance) -> Result<[Geometry; 2], ProbeError> {
        Ok([
            self.base.geometry(tolerance)?,
            self.target.geometry(tolerance)?,
        ])
    }
    pub fn mode(&self) -> GripMode {
        if self.edit {
            GripMode::Edit
        } else {
            GripMode::Control
        }
    }
    pub fn observe(
        &self,
        doc: &Document,
        source: ObjectId,
        peer: Option<ObjectId>,
        guides: [ObjectId; 2],
    ) -> Result<Value, ProbeError> {
        let mut value =
            super::orient_surface_grips::snapshot_excluding(doc, source, peer, &guides)?;
        value.as_object_mut().unwrap().remove("guide_selected");
        value.as_object_mut().unwrap().remove("guide_unchanged");
        value["guides_selected"] = json!(guides.map(|id| doc.is_selected(id)));
        let geometry = self.guides(doc.tolerance())?;
        value["guides_unchanged"] = json!(
            guides
                .into_iter()
                .zip(geometry)
                .all(|(id, g)| doc.object(id).is_some_and(|o| o.geometry() == &g))
        );
        Ok(value)
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let [a, b] = f.guides(tolerance)?;
        let guides = [doc.add_geometry(a)?, doc.add_geometry(b)?];
        let source = doc.add_geometry(f.source_geometry(tolerance)?)?;
        let peer = if f.peer {
            Some(doc.add_geometry(Geometry::Point(Point3::try_new(6., 1., 0.)?))?)
        } else {
            None
        };
        if f.group {
            doc.add_group(
                Some("Owned Flow grips".into()),
                std::iter::once(source).chain(peer),
            )?;
        }
        match f.mode() {
            GripMode::Control => doc.enable_control_points([source])?,
            GripMode::Edit => doc.enable_edit_points([source])?,
        };
        let picks = f
            .selected
            .iter()
            .map(|&index| ControlPointId {
                object: source,
                index,
            })
            .collect::<Vec<_>>();
        if !f.postselected {
            doc.select_control_points(picks.iter().copied(), SelectionMode::Add)?;
            doc.select_objects_direct(
                peer.into_iter().chain(f.parent.then_some(source)),
                SelectionMode::Replace,
            )?;
        }
        doc.clear_history()?;
        let before = f.observe(&doc, source, peer, guides)?;
        if f.postselected {
            doc.select_control_points(picks.iter().copied(), SelectionMode::Add)?;
        }
        let sources = peer
            .into_iter()
            .chain(f.parent.then_some(source))
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let grips = picks
            .iter()
            .map(|g| format!("{}:{}", g.object, g.index))
            .collect::<Vec<_>>()
            .join(",");
        let picked = if f.postselected { "Picked" } else { "" };
        let registry = CommandRegistry::with_builtins();
        let error=registry.execute(&mut doc,&format!("Flow Base={} Target={} {} {picked}Sources={sources} {picked}GripSources={grips}",guides[0],guides[1],f.options.command().arguments())).err().map(|e|e.to_string());
        let after = f.observe(&doc, source, peer, guides)?;
        let after_geometry = doc
            .objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>();
        let undo = doc.undo()?;
        let undo_snapshot = f.observe(&doc, source, peer, guides)?;
        let undo_geometry =
            doc.object(source).unwrap().geometry() == &f.source_geometry(tolerance)?;
        let redo = doc.redo()?;
        let redo_snapshot = f.observe(&doc, source, peer, guides)?;
        let redo_geometry = after_geometry
            .iter()
            .all(|(id, g)| doc.object(*id).is_some_and(|o| o.geometry() == g));
        Ok(
            json!({"before":before,"after":after,"command_returned":error.is_none(),"error":error,"undo":undo_snapshot,"redo":redo_snapshot,"undo_label":undo,"redo_label":redo,"undo_restores_source":undo_geometry,"redo_restores_outputs":redo_geometry}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6
            }
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| close(a, b)))
            }
            _ => a == b,
        }
    }
    #[test]
    fn flow_grips_match_seventy_two_native_owner_selection_group_and_history_workflows() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/flow_grips.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/flow_grips_rhino_20261016.json"
        ))
        .unwrap();
        assert_eq!(actual.results.len(), 72);
        assert_eq!(native["results"].as_array().unwrap().len(), 72);
        for (row, n) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(row.id, n["id"]);
            for key in [
                "command_returned",
                "undo_restores_source",
                "redo_restores_outputs",
            ] {
                assert_eq!(row.value[key], true, "{} {key}: {}", row.id, row.value);
            }
            assert_eq!(row.value["undo_label"], "Flow");
            assert_eq!(row.value["redo_label"], "Flow");
            for stage in ["before", "after", "undo", "redo"] {
                let a = &row.value[stage];
                // EndCommand still contains temporary selected guide objects;
                // compare the completed macro's public document state.
                let b = &n["value"][if stage == "after" {
                    "after_script"
                } else {
                    stage
                }];
                for key in ["group_count", "guides_selected", "guides_unchanged"] {
                    assert_eq!(a[key], b[key], "{} {stage} {key}", row.id);
                }
                let aa = a["outputs"].as_array().unwrap();
                let bb = b["outputs"].as_array().unwrap();
                assert_eq!(aa.len(), bb.len(), "{} {stage}", row.id);
                let mut used = BTreeSet::new();
                for o in aa {
                    let (i, _) = bb
                        .iter()
                        .enumerate()
                        .find(|(i, b)| !used.contains(i) && close(o, b))
                        .unwrap_or_else(|| {
                            panic!("{} {stage}: unmatched output {o}; native {b}", row.id)
                        });
                    used.insert(i);
                }
            }
        }
    }
}
