//! Standalone selection lifecycle from owned grid recipes, without native output input.
use super::*;
use viboceros_command::{
    ComponentPick, ComponentSelectionKind as Kind,
    component_selection::ComponentAction,
    edge_pattern_selection::{Pattern, Session},
};
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    kind: String,
    scene: String,
    pair: [usize; 2],
    initial: Vec<[usize; 2]>,
    steps: Vec<Step>,
    finish: String,
    #[serde(default)]
    whole: bool,
    component: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Step {
    kind: String,
    modifiers: String,
    pair: [usize; 2],
}
fn snapshot(
    doc: &Document,
    registry: &CommandRegistry,
    id: ObjectId,
    ids: Option<&viboceros_geometry::SubDComponentIds>,
) -> Result<Value, ProbeError> {
    let mut picks = registry
        .component_selection()
        .checked_picks(doc)
        .map_err(ProbeError::FixtureInvariant)?;
    picks.sort();
    let ordinals = picks
        .iter()
        .filter(|p| matches!(p.kind, Kind::MeshEdge | Kind::SubDEdge))
        .map(|p| p.index)
        .collect::<Vec<_>>();
    let components = picks
        .iter()
        .map(|p| {
            let (kind, index) = match p.kind {
                Kind::MeshEdge => ("MeshTopologyEdge", p.index),
                Kind::MeshFace => ("MeshFace", p.index),
                Kind::MeshVertex => ("MeshVertex", p.index),
                Kind::SubDEdge => ("SubdEdge", ids.unwrap().edges[p.index] as usize),
                Kind::SubDFace => ("SubdFace", ids.unwrap().faces[p.index] as usize),
                Kind::SubDVertex => ("SubdVertex", ids.unwrap().vertices[p.index] as usize),
                _ => unreachable!(),
            };
            json!({"kind":kind,"index":index})
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"whole":doc.is_selected(id),"components":components,"ordinals":ordinals,"objects":doc.objects().len()}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_pattern(f, iterations, tolerance, Pattern::Loop)
}
pub(super) fn run_pattern(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    pattern: Pattern,
) -> Result<(Value, u64), ProbeError> {
    if !matches!(f.kind.as_str(), "mesh" | "subd")
        || f.initial.len() > 24
        || f.steps.len() > 24
        || !matches!(f.finish.as_str(), "Enter" | "Cancel")
        || f.component
            .as_ref()
            .is_some_and(|s| !matches!(s.as_str(), "face" | "vertex"))
        || f.steps
            .iter()
            .any(|s| s.kind != "click" || !matches!(s.modifiers.as_str(), "plain" | "ctrl"))
    {
        return Err(ProbeError::FixtureInvariant(
            "owned edge selection workflow required",
        ));
    }
    measure_probe(iterations, || {
        let mesh = f.kind == "mesh";
        let (geometry, source_points) =
            edge_pattern_source::geometry(&f.kind, &f.scene, tolerance)?;
        let mut vertices = std::collections::BTreeMap::new();
        for (i, p) in source_points.iter().enumerate() {
            vertices.entry(p.to_array().map(f64::to_bits)).or_insert(i);
        }
        let edges = match &geometry {
            Geometry::Mesh(m) => m
                .topology_edge_points()
                .iter()
                .map(|e| e.map(|p| vertices[&p.to_array().map(f64::to_bits)]))
                .collect::<Vec<_>>(),
            Geometry::SubD(s) => s
                .edges()
                .iter()
                .map(|e| {
                    e.map(|v| {
                        vertices[&s.control_points()[v as usize].to_array().map(f64::to_bits)]
                    })
                })
                .collect(),
            _ => unreachable!(),
        };
        let ids = match &geometry {
            Geometry::SubD(s) => Some(s.component_ids()?),
            _ => None,
        };
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(geometry)?;
        doc.clear_history()?;
        let original = doc.object(id).unwrap().geometry_snapshot().clone();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 40,50,60")?;
        registry.execute(&mut doc, "Undo")?;
        if f.whole {
            doc.select_objects_direct([id], SelectionMode::Add)?;
        }
        let edge = |pair: [usize; 2]| -> Result<ComponentPick, ProbeError> {
            let index = edges
                .iter()
                .position(|e| *e == pair || *e == [pair[1], pair[0]])
                .ok_or(ProbeError::FixtureInvariant("source edge pair required"))?;
            Ok(ComponentPick {
                object: id,
                index,
                kind: if mesh { Kind::MeshEdge } else { Kind::SubDEdge },
            })
        };
        edge(f.pair)?;
        let mut initial = f
            .initial
            .iter()
            .map(|&p| edge(p))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(c) = &f.component {
            initial.push(ComponentPick {
                object: id,
                index: 0,
                kind: match (c.as_str(), mesh) {
                    ("face", true) => Kind::MeshFace,
                    ("face", false) => Kind::SubDFace,
                    (_, true) => Kind::MeshVertex,
                    (_, false) => Kind::SubDVertex,
                },
            });
        }
        registry
            .component_selection()
            .replace(&doc, initial)
            .map_err(ProbeError::FixtureInvariant)?;
        let before = snapshot(&doc, &registry, id, ids.as_ref())?;
        let mut trace = Vec::new();
        if !f.initial.is_empty() {
            if !f.steps.is_empty() {
                return Err(ProbeError::FixtureInvariant(
                    "preselected loops complete immediately",
                ));
            }
            registry.execute(&mut doc, pattern.command())?;
        } else {
            registry.component_selection().clear();
            let mut session = Session::new(pattern);
            for step in &f.steps {
                let p = edge(step.pair)?;
                if !doc.is_selected(id) {
                    session.pick(
                        &doc,
                        &[p],
                        if step.modifiers == "ctrl" {
                            ComponentAction::Remove
                        } else {
                            ComponentAction::Add
                        },
                    )?;
                }
                registry
                    .component_selection()
                    .replace(&doc, session.seeds())
                    .map_err(ProbeError::FixtureInvariant)?;
                trace.push(snapshot(&doc, &registry, id, ids.as_ref())?);
            }
            let selected = if f.finish == "Enter" {
                session.finish(&doc)?
            } else {
                Vec::new()
            };
            registry
                .component_selection()
                .replace(&doc, selected)
                .map_err(ProbeError::FixtureInvariant)?;
        }
        let after = snapshot(&doc, &registry, id, ids.as_ref())?;
        registry.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, &registry, id, ids.as_ref())?;
        registry.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, &registry, id, ids.as_ref())?;
        Ok(
            json!({"before":before,"trace":trace,"after":after,"redo":redo,"undo":undo,"success":f.finish=="Enter","source_unchanged":doc.object(id).unwrap().geometry_snapshot()==&original}),
        )
    })
}
