//! SubD diagnostic curves and real edge-analysis command/history execution.
use super::*;
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub scene: String,
    pub flat: bool,
    pub preselect: bool,
    pub command: String,
    pub actions: Vec<String>,
    #[serde(default)]
    pub shared_input: bool,
    #[serde(default)]
    pub zoom: Vec<String>,
    #[serde(default = "enter")]
    pub finish: String,
}
fn enter() -> String {
    "Enter".into()
}

pub fn source(f: &Fixture, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
    if !matches!(
        f.scene.as_str(),
        "bent_grid"
            | "open_cube"
            | "cube"
            | "creased_cube"
            | "pentagon"
            | "fan7"
            | "asset_variable_both_ends"
            | "asset_hard_soft_chain"
    ) {
        return Err(ProbeError::FixtureInvariant(
            "owned diagnostic source required",
        ));
    }
    if f.shared_input || f.scene.starts_with("asset_") {
        let (folder, name) = if f.shared_input {
            ("subd_edge_analysis_inputs", f.scene.as_str())
        } else {
            ("subd_soft_nurbs_inputs", &f.scene[6..])
        };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/rhino_oracle/fixtures")
            .join(folder)
            .join(format!("{name}.3dm"));
        let mut subds = read_3dm_file(path, tolerance)?
            .objects
            .into_iter()
            .filter_map(|o| match o.geometry {
                ThreeDmGeometry::SubD(s) => Some(s),
                _ => None,
            });
        let s = subds
            .next()
            .ok_or(ProbeError::FixtureInvariant("shared SubD source missing"))?;
        if subds.next().is_some() {
            return Err(ProbeError::FixtureInvariant(
                "one shared SubD source required",
            ));
        }
        return Ok(Geometry::SubD(s));
    }
    subd_direction::source(&f.scene, tolerance)
}

pub fn snapshot(doc: &Document, id: ObjectId) -> Result<Value, ProbeError> {
    let object = doc.object(id).ok_or(ProbeError::FixtureInvariant(
        "diagnostic source identity changed",
    ))?;
    Ok(
        json!({"geometry":subd_direction::definition(object.geometry())?,"selected":doc.is_selected(id),"objects":doc.objects().len(),
        "points":doc.objects().filter_map(|o|match o.geometry(){Geometry::Point(p)=>Some(p.to_array()),_=>None}).collect::<Vec<_>>()}),
    )
}

pub fn curves(doc: &Document, commands: &CommandRegistry) -> Result<Value, ProbeError> {
    let view = commands
        .edge_analysis_view(doc)?
        .or(commands.edge_zoom_view(doc)?)
        .ok_or(ProbeError::FixtureInvariant("diagnostic session missing"))?;
    Ok(json!(view.edges.iter().map(|e| {
        let d=e.curve.domain();
        Ok(json!({"index":e.index,"all":e.all,"naked":e.naked,"non_manifold":e.non_manifold,
            "domain":[d.start(),d.end()],"points":(0..=32).map(|i|e.curve.evaluate(d.start()+(d.end()-d.start())*i as f64/32.).map(|p|p.to_array())).collect::<Result<Vec<_>,_>>()?}))
    }).collect::<Result<Vec<Value>,GeometryError>>()?))
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1
        || !matches!(
            f.command.as_str(),
            "ShowEdges" | "ZoomNaked" | "ZoomNonManifold"
        )
        || !matches!(f.finish.as_str(), "Enter" | "Cancel")
        || f.actions.len() > 32
        || f.zoom.len() > 32
        || f.actions.iter().any(|s| {
            if f.command == "ShowEdges" {
                !matches!(s.as_str(), "Show=All" | "Show=Naked" | "Show=NonManifold")
            } else {
                !matches!(s.as_str(), "All" | "Current" | "Next" | "Previous" | "Mark")
            }
        })
        || f.zoom
            .iter()
            .any(|s| !matches!(s.as_str(), "All" | "Current" | "Next" | "Previous" | "Mark"))
    {
        return Err(ProbeError::FixtureInvariant(
            "owned diagnostic workflow required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let commands = CommandRegistry::with_builtins();
        let id = doc.add_geometry(source(f, tolerance)?)?;
        if f.flat {
            commands.execute(&mut doc, "SubDDisplayToggle")?;
        }
        doc.clear_history()?;
        commands.execute(&mut doc, "Point 40,50,60")?;
        commands.execute(&mut doc, "Undo")?;
        if f.preselect {
            doc.select_objects_direct([id], SelectionMode::Replace)?;
        }
        let before = snapshot(&doc, id)?;
        if !f.preselect {
            doc.set_transient_selection([id])?;
        }
        let command = if f.command == "ShowEdges" {
            "ShowEdges".into()
        } else {
            format!("{} {} {}", f.command, f.actions.join(" "), f.finish)
        };
        let result = if f.preselect {
            commands.execute(&mut doc, &command)
        } else {
            commands.execute_postselected(&mut doc, &command, Default::default())
        };
        match result {
            Ok(_) | Err(CommandError::OperationDeclined) => {}
            Err(e) => return Err(e.into()),
        }
        if f.command == "ShowEdges" {
            for action in &f.actions {
                commands.execute(&mut doc, &format!("ShowEdges {action}"))?;
            }
            if !f.zoom.is_empty() {
                commands.execute(&mut doc, &format!("ShowEdges Zoom {}", f.zoom.join(" ")))?;
            }
        }
        let edges = curves(&doc, &commands)?;
        let counts=commands.edge_analysis_view(&doc)?.or(commands.edge_zoom_view(&doc)?).map(|v|json!({"all":v.edges.iter().filter(|e|e.all).count(),"naked":v.edges.iter().filter(|e|e.naked).count(),"non_manifold":v.edges.iter().filter(|e|e.non_manifold).count()})).unwrap();
        let after = snapshot(&doc, id)?;
        let changed = !after["points"].as_array().unwrap().is_empty();
        let (undo, redo) = if changed {
            commands.execute(&mut doc, "Undo")?;
            let undo = snapshot(&doc, id)?;
            commands.execute(&mut doc, "Redo")?;
            (undo, snapshot(&doc, id)?)
        } else {
            commands.execute(&mut doc, "Redo")?;
            let redo = snapshot(&doc, id)?;
            commands.execute(&mut doc, "Undo")?;
            (snapshot(&doc, id)?, redo)
        };
        Ok(
            json!({"before":before,"after":after,"undo":undo,"redo":redo,"edges":edges,"counts":counts}),
        )
    })
}
