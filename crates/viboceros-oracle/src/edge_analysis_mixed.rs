//! Shared mixed operands through real diagnostic commands and document history.
use super::*;
use viboceros_document::ObjectAttributes;
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub order: Vec<String>,
    pub preselect: bool,
    pub flat: bool,
    pub grouped: bool,
    pub extra_point: bool,
    pub command: String,
    pub actions: Vec<String>,
    #[serde(default)]
    pub membership: bool,
    #[serde(default)]
    pub panel_only: bool,
    #[serde(default)]
    pub sequential: bool,
}
impl Fixture {
    pub fn input_name(&self) -> String {
        format!(
            "{}{}",
            self.order.join("_"),
            if self.extra_point { "_point" } else { "" }
        )
    }
    pub fn kinds(&self) -> Vec<&str> {
        self.order
            .iter()
            .map(String::as_str)
            .chain(self.extra_point.then_some("point"))
            .collect()
    }
    pub fn validate(&self) -> Result<(), ProbeError> {
        if self.order.len() != 3
            || self
                .order
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != BTreeSet::from(["brep", "mesh", "subd"])
            || !matches!(self.command.as_str(), "ShowEdges" | "ZoomNaked")
            || self.actions.len() > 32
            || self
                .actions
                .iter()
                .any(|s| !matches!(s.as_str(), "All" | "Current" | "Mark" | "Next" | "Previous"))
            || (self.command == "ShowEdges" && !self.actions.is_empty())
            || (self.membership
                && (!self.preselect
                    || self.command != "ShowEdges"
                    || self.extra_point
                    || self.grouped))
            || ((self.panel_only || self.sequential) && !self.membership)
            || (self.panel_only && self.sequential)
        {
            return Err(ProbeError::FixtureInvariant(
                "owned mixed diagnostic recipe required",
            ));
        }
        Ok(())
    }
}

pub fn sources(f: &Fixture, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
    f.validate()?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/edge_analysis_mixed_inputs")
        .join(format!("{}.3dm", f.input_name()));
    let mut geometries = read_3dm_file(path, tolerance)?
        .objects
        .into_iter()
        .map(|o| match o.geometry {
            ThreeDmGeometry::Brep(g) => Ok(Geometry::Brep(g)),
            ThreeDmGeometry::Mesh(g) => Ok(Geometry::Mesh(g)),
            ThreeDmGeometry::SubD(g) => Ok(Geometry::SubD(g)),
            ThreeDmGeometry::Point(g) => Ok(Geometry::Point(g)),
            _ => Err(ProbeError::FixtureInvariant("unsupported mixed operand")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = Vec::new();
    for kind in f.kinds() {
        let position = geometries
            .iter()
            .position(|g| match g {
                Geometry::Brep(_) => kind == "brep",
                Geometry::Mesh(_) => kind == "mesh",
                Geometry::SubD(_) => kind == "subd",
                Geometry::Point(_) => kind == "point",
                _ => false,
            })
            .ok_or(ProbeError::FixtureInvariant("mixed source type missing"))?;
        result.push(geometries.remove(position));
    }
    if !geometries.is_empty() {
        return Err(ProbeError::FixtureInvariant("unexpected mixed operand"));
    }
    Ok(result)
}

fn geometry(g: &Geometry) -> Result<Value, ProbeError> {
    Ok(match g {
        Geometry::SubD(_) => subd_direction::definition(g)?,
        Geometry::Brep(b) => json!({"kind":"brep","value":brep_interchange::geometry_record(b)?}),
        Geometry::Mesh(m) => {
            json!({"kind":"mesh","points":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices()).collect::<Vec<_>>()})
        }
        Geometry::Point(p) => json!({"kind":"point","point":p.to_array()}),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "owned mixed geometry required",
            ));
        }
    })
}

pub fn setup(f: &Fixture, tolerance: Tolerance) -> Result<(Document, Vec<ObjectId>), ProbeError> {
    let mut doc = Document::new(tolerance);
    let mut ids = Vec::new();
    for (g, kind) in sources(f, tolerance)?.into_iter().zip(f.kinds()) {
        ids.push(doc.add_geometry_with_attributes(
            g,
            ObjectAttributes::on_layer(doc.current_layer_id()).with_name(kind),
        )?);
    }
    if f.grouped {
        doc.add_group(Some("Owned diagnostics".into()), ids.iter().copied())?;
    }
    if f.flat {
        doc.set_subd_control_net(true);
    }
    doc.clear_history()?;
    Ok((doc, ids))
}

pub fn snapshot(doc: &Document, ids: &[ObjectId], f: &Fixture) -> Result<Value, ProbeError> {
    let rows=ids.iter().zip(f.kinds()).map(|(&id,kind)| {
        let o=doc.object(id).ok_or(ProbeError::FixtureInvariant("mixed source identity changed"))?;
        Ok(json!({"kind":kind,"name":o.attributes().name(),"groups":o.group_ids().len(),"geometry":geometry(o.geometry())?,"selected":doc.is_selected(id)}))
    }).collect::<Result<Vec<Value>,ProbeError>>()?;
    let points = doc
        .objects()
        .filter(|o| !ids.contains(&o.id()))
        .filter_map(|o| match o.geometry() {
            Geometry::Point(p) => Some(p.to_array()),
            _ => None,
        })
        .collect::<Vec<_>>();
    Ok(json!({"sources":rows,"points":points,"objects":doc.objects().len()}))
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "one mixed diagnostic iteration required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, ids) = setup(f, tolerance)?;
        let commands = CommandRegistry::with_builtins();
        commands.execute(&mut doc, "Point 40,50,60")?;
        commands.execute(&mut doc, "Undo")?;
        if f.preselect {
            let selected = if f.membership && !f.panel_only {
                &ids[..1]
            } else {
                &ids
            };
            doc.select_objects_direct(selected.iter().copied(), SelectionMode::Replace)?;
        }
        let before = snapshot(&doc, &ids, f)?;
        if !f.preselect {
            let picked = ids
                .iter()
                .copied()
                .filter(|id| {
                    viboceros_command::ObjectSelectionFilter::DiagnosticEdges
                        .accepts_object(doc.object(*id).unwrap())
                })
                .collect::<Vec<_>>();
            doc.set_transient_selection(picked)?;
        }
        let command = if f.command == "ShowEdges" {
            f.command.clone()
        } else {
            format!("{} {} Enter", f.command, f.actions.join(" "))
        };
        if f.preselect {
            commands.execute(&mut doc, &command)?;
        } else {
            commands.execute_postselected(&mut doc, &command, Default::default())?;
        }
        let mut stages = Vec::new();
        if f.membership {
            stages.push(json!({"action":"Start","state":snapshot(&doc,&ids,f)?}));
            if !f.panel_only {
                let batches = if f.sequential {
                    ids[1..].chunks(1).collect::<Vec<_>>()
                } else {
                    vec![&ids[1..]]
                };
                for (i, batch) in batches.into_iter().enumerate() {
                    let select = if f.sequential {
                        format!("Select{}", i + 1)
                    } else {
                        "SelectRest".into()
                    };
                    let add = if f.sequential {
                        format!("Add{}", i + 1)
                    } else {
                        "Add".into()
                    };
                    doc.select_objects_direct(batch.iter().copied(), SelectionMode::Replace)?;
                    stages.push(json!({"action":select,"state":snapshot(&doc,&ids,f)?}));
                    commands.execute(&mut doc, "ShowEdges Add")?;
                    stages.push(json!({"action":add,"state":snapshot(&doc,&ids,f)?}));
                }
                doc.select_objects_direct([ids[0]], SelectionMode::Replace)?;
                stages.push(json!({"action":"SelectFirst","state":snapshot(&doc,&ids,f)?}));
                commands.execute(&mut doc, "ShowEdges Remove")?;
                stages.push(json!({"action":"Remove","state":snapshot(&doc,&ids,f)?}));
            }
            commands.execute(&mut doc, "ShowEdges Show=Naked")?;
            let name = commands
                .begin_edge_analysis_zoom(&mut doc)?
                .ok_or(ProbeError::FixtureInvariant("panel zoom unavailable"))?;
            let mut group = doc.begin_history_group(name)?;
            commands.execute_edge_zoom_actions(&mut doc, "All", &mut group)?;
            commands.execute_edge_zoom_actions(&mut doc, "Mark", &mut group)?;
            commands.clear_edge_zoom();
            stages.push(json!({"action":"Zoom","state":snapshot(&doc,&ids,f)?}));
        }
        let after = snapshot(&doc, &ids, f)?;
        let (undo, redo) = if after["points"].as_array().unwrap().is_empty() {
            commands.execute(&mut doc, "Redo")?;
            let redo = snapshot(&doc, &ids, f)?;
            commands.execute(&mut doc, "Undo")?;
            (snapshot(&doc, &ids, f)?, redo)
        } else {
            commands.execute(&mut doc, "Undo")?;
            let undo = snapshot(&doc, &ids, f)?;
            commands.execute(&mut doc, "Redo")?;
            (undo, snapshot(&doc, &ids, f)?)
        };
        let mut result = json!({"before":before,"after":after,"undo":undo,"redo":redo});
        if f.membership {
            result["stages"] = json!(stages);
        }
        Ok(result)
    })
}
