//! Public-SDK topology evidence for the actual edge-analysis command session.
use super::*;
use crate::object_source::ObjectSource;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub sources: Vec<ObjectSource>,
    #[serde(default)]
    pub workflow: Option<Workflow>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Workflow {
    pub command: String,
    pub actions: Vec<String>,
    #[serde(default)]
    pub undo_redo: bool,
}

fn point_record(doc: &Document, ids: &[ObjectId]) -> Value {
    let points = doc
        .objects()
        .filter_map(|o| match o.geometry() {
            Geometry::Point(point) if !ids.contains(&o.id()) => Some(point.to_array()),
            _ => None,
        })
        .collect::<Vec<_>>();
    json!({"points":points,"input_count":ids.iter().filter(|id|doc.object(**id).is_some()).count()})
}

pub(super) fn run(f: &Fixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if f.sources.is_empty() || f.sources.len() > 64 {
        return Err(ProbeError::FixtureInvariant(
            "invalid edge analysis sources",
        ));
    }
    let mut doc = Document::new(tolerance);
    let ids = f
        .sources
        .iter()
        .map(|source| {
            doc.add_geometry(source.geometry(tolerance)?)
                .map_err(ProbeError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    let registry = CommandRegistry::with_builtins();
    if let Some(workflow) = &f.workflow {
        if !matches!(workflow.command.as_str(), "ZoomNaked" | "ZoomNonManifold")
            || workflow.actions.len() > 32
            || workflow
                .actions
                .iter()
                .any(|n| !matches!(n.as_str(), "All" | "Current" | "Next" | "Previous" | "Mark"))
        {
            return Err(ProbeError::FixtureInvariant(
                "invalid edge analysis workflow",
            ));
        }
        let command = format!("{} {}", workflow.command, workflow.actions.join(" "));
        let succeeded = registry.execute(&mut doc, &command).is_ok();
        let mut value = point_record(&doc, &ids);
        value["succeeded"] = json!(succeeded);
        value["command"] = json!(command);
        if workflow.undo_redo {
            let undone = doc.undo().is_ok();
            let mut undo = point_record(&doc, &ids);
            undo["succeeded"] = json!(undone);
            value["undo"] = undo;
            let redone = doc.redo().is_ok();
            let mut redo = point_record(&doc, &ids);
            redo["succeeded"] = json!(redone);
            value["redo"] = redo;
        }
        return Ok((value, 0));
    }
    let start = std::time::Instant::now();
    registry.execute(&mut doc, "ShowEdges Show=All")?;
    let view = registry
        .edge_analysis_view(&doc)?
        .ok_or(ProbeError::FixtureInvariant(
            "missing edge analysis session",
        ))?;
    let elapsed =
        u64::try_from(start.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let mut sources = Vec::new();
    for id in ids {
        let mut edges = Vec::new();
        for edge in view.edges.iter().filter(|edge| edge.object == id) {
            let domain = edge.curve.domain();
            let mut points = Vec::new();
            for i in 0..=16 {
                let t = match i {
                    0 => *domain.start(),
                    16 => *domain.end(),
                    _ => *domain.start() + (domain.end() - domain.start()) * i as f64 / 16.,
                };
                points.push(edge.curve.evaluate(t)?.to_array());
            }
            edges.push(json!({"index":edge.index,"domain":[domain.start(),domain.end()],
                "points":points,"all":edge.all,"naked":edge.naked,"non_manifold":edge.non_manifold}));
        }
        sources.push(json!({"edges":edges}));
    }
    Ok((json!({"sources":sources}), elapsed))
}
