//! Surface placement from authored control/edit grip owner recipes.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_command::CommandContext;
use viboceros_document::{ControlPointId, GripMode};
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    #[serde(flatten)]
    pub placement: super::orient_surface::Fixture,
    pub source: ObjectSource,
    pub selected: Vec<usize>,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub parent: bool,
    #[serde(default)]
    pub peer: bool,
}
impl Fixture {
    pub fn source_geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        self.source.geometry(tolerance)
    }
    pub fn observe(
        &self,
        doc: &Document,
        source: ObjectId,
        peer: Option<ObjectId>,
        guide: ObjectId,
    ) -> Result<Value, ProbeError> {
        let mut value = snapshot(doc, source, peer, guide)?;
        value["guide_unchanged"] = json!(doc.object(guide).is_some_and(|o| {
            self.placement
                .guide(doc.tolerance())
                .is_ok_and(|g| o.geometry() == &g)
        }));
        Ok(value)
    }
    pub fn mode(&self) -> GripMode {
        if self.edit {
            GripMode::Edit
        } else {
            GripMode::Control
        }
    }
}
pub(super) fn snapshot(
    doc: &Document,
    source: ObjectId,
    peer: Option<ObjectId>,
    guide: ObjectId,
) -> Result<Value, ProbeError> {
    let groups = doc.groups().map(|g| g.id()).collect::<Vec<_>>();
    let mut outputs = Vec::new();
    for o in doc.objects().filter(|o| o.id() != guide) {
        let mut row = json!({"original":o.id()==source,"peer":Some(o.id())==peer,"selected":doc.is_selected(o.id()),"group_count":o.group_ids().len(),"groups":o.group_ids().iter().filter_map(|g|groups.iter().position(|v|v==g)).collect::<Vec<_>>(),"grips_on":doc.grip_mode(o.id()).is_some(),"grips":doc.control_points().filter(|(id,_,_)|id.object==o.id()).map(|(id,p,selected)|json!({"index":id.index,"point":p.to_array(),"selected":selected})).collect::<Vec<_>>()});
        match o.geometry() {
            Geometry::Point(p) => row["point"] = json!(p.to_array()),
            Geometry::Mesh(m) => row["mesh"] = polygon_mesh_value(m),
            Geometry::NurbsSurface(s) => row["surface"] = nurbs_surface_definition_value(s),
            Geometry::Brep(b) if b.faces().len() == 1 => {
                row["surface"] = nurbs_surface_definition_value(b.faces()[0].surface())
            }
            g => {
                row["curve"] = nurbs_curve_definition_value(
                    &g.nurbs_curve_representation()?
                        .ok_or(ProbeError::FixtureInvariant("grip owner curve"))?,
                );
            }
        }
        // RhinoCommon exposes compact knots. The native observer pads the
        // two absent outer values by duplicating its first/last exposed knot.
        // Normalize only those unobservable padding entries, never source data.
        for family in ["curve", "surface"] {
            for key in ["knots", "knots_u", "knots_v"] {
                if let Some(knots) = row
                    .get_mut(family)
                    .and_then(|v| v.get_mut(key))
                    .and_then(Value::as_array_mut)
                    && knots.len() >= 2
                {
                    let last = knots.len() - 1;
                    knots[0] = knots[1].clone();
                    knots[last] = knots[last - 1].clone();
                }
            }
        }
        outputs.push(row);
    }
    Ok(
        json!({"outputs":outputs,"group_count":groups.len(),"guide_selected":doc.is_selected(guide),"guide_unchanged":true}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let guide_geometry = f.placement.guide(tolerance)?;
        let guide = doc.add_geometry(guide_geometry.clone())?;
        let source = doc.add_geometry(f.source_geometry(tolerance)?)?;
        let peer = if f.peer {
            Some(doc.add_geometry(Geometry::Point(Point3::try_new(1., 1., 1.)?))?)
        } else {
            None
        };
        if f.placement.group {
            doc.add_group(
                Some("Owned grip placement".into()),
                std::iter::once(source).chain(peer),
            )?;
        }
        match f.mode() {
            GripMode::Control => doc.enable_control_points([source])?,
            GripMode::Edit => doc.enable_edit_points([source])?,
        };
        let grips = f
            .selected
            .iter()
            .map(|&index| ControlPointId {
                object: source,
                index,
            })
            .collect::<Vec<_>>();
        if !f.placement.postselected {
            doc.select_control_points(grips.iter().copied(), SelectionMode::Add)?;
            doc.select_objects_direct(
                peer.into_iter().chain(f.parent.then_some(source)),
                SelectionMode::Replace,
            )?;
        }
        doc.clear_history()?;
        let before = snapshot(&doc, source, peer, guide)?;
        // Complete the source getter before the point-phase history starts.
        if f.placement.postselected {
            doc.select_control_points(grips.iter().copied(), SelectionMode::Add)?;
            doc.select_objects_direct(peer, SelectionMode::Add)?;
        }
        let sources = peer
            .into_iter()
            .chain(f.parent.then_some(source))
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let picks = grips
            .iter()
            .map(|g| format!("{}:{}", g.object, g.index))
            .collect::<Vec<_>>()
            .join(",");
        let mut group = doc.begin_history_group("OrientOnSrf")?;
        let registry = CommandRegistry::with_builtins();
        let placements = std::iter::once((f.placement.uv, f.placement.options.options())).chain(
            f.placement.batch.iter().map(|b| {
                let mut o = f.placement.options.options();
                o.scale = b.scale.unwrap_or(o.scale);
                o.rotation = b.rotation.unwrap_or(o.rotation);
                (b.uv, o)
            }),
        );
        for (uv, options) in placements {
            let point = |p: [f64; 3]| p.map(|x| x.to_string()).join(",");
            let picked = if f.placement.postselected {
                "Picked"
            } else {
                ""
            };
            let command = format!(
                "OrientOnSrf {} {} {} Surface={guide} Face=0 {} {picked}Sources={sources} {picked}GripSources={picks}",
                point(f.placement.base),
                point(f.placement.reference),
                point(f.placement.target(uv, tolerance)?.to_array()),
                options.arguments()
            );
            registry.execute_in_history_group(
                &mut doc,
                &command,
                CommandContext::default(),
                &mut group,
            )?;
        }
        let mut after = snapshot(&doc, source, peer, guide)?;
        after["guide_unchanged"] = json!(doc.object(guide).unwrap().geometry() == &guide_geometry);
        let after_geometry = doc
            .objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>();
        let undo = doc.undo()?;
        let undo_geometry =
            doc.object(source).unwrap().geometry() == &f.source_geometry(tolerance)?;
        let redo = doc.redo()?;
        let redo_geometry = after_geometry
            .iter()
            .all(|(id, g)| doc.object(*id).is_some_and(|o| o.geometry() == g));
        Ok(
            json!({"before":before,"after":after,"undo_label":undo,"redo_label":redo,"undo_restores_source":undo_geometry,"redo_restores_outputs":redo_geometry}),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orient_surface_grips_replay_native_geometry_selection_and_group_memberships() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/orient_surface_grips.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/orient_surface_grips_rhino_20261016.json"
        ))
        .unwrap();
        assert_eq!(
            actual.results.len(),
            native["results"].as_array().unwrap().len()
        );
        for (a, b) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(a.id, b["id"]);
            for k in ["undo_restores_source", "redo_restores_outputs"] {
                assert_eq!(a.value[k], true, "{} {k}", a.id);
            }
            let local = &a.value["after"];
            let expected = &b["value"]["after"];
            for k in ["group_count", "guide_selected", "guide_unchanged"] {
                assert_eq!(local[k], expected[k], "{} {k}", a.id);
            }
            let outputs = local["outputs"].as_array().unwrap();
            let refs = expected["outputs"].as_array().unwrap();
            assert_eq!(outputs.len(), refs.len(), "{}", a.id);
            let mut used = BTreeSet::new();
            for o in outputs {
                let (i, e) = refs
                    .iter()
                    .enumerate()
                    .find(|(i, e)| !used.contains(i) && close(o, e))
                    .unwrap_or_else(|| panic!("{} output {} expected {}", a.id, o, expected));
                used.insert(i);
                assert!(close(o, e));
            }
        }
    }
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
}
