//! Face and naked-edge extrusion staged outside document history.
use crate::*;
use viboceros_document::{GeometrySnapshot, GroupId};
use viboceros_geometry::{SubDExtrudeDirection, SubDExtrudeSelection};
mod arguments;
#[cfg(test)]
mod tests;
pub use arguments::{Basis, Direction, Input, Options, parse};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Whole(ObjectId),
    Face(ObjectId, usize),
    Edge(ObjectId, usize),
}
#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    geometry: GeometrySnapshot,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
}
#[derive(Clone, Debug)]
pub struct Plan {
    sources: Vec<Source>,
    outputs: Vec<(ObjectId, Geometry)>,
    selected: Vec<ObjectId>,
    tolerance: Tolerance,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        targets: impl IntoIterator<Item = Target>,
        options: Options,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        let mut grouped = BTreeMap::<ObjectId, (bool, BTreeSet<usize>, BTreeSet<usize>)>::new();
        for (i, target) in targets.into_iter().enumerate() {
            if i >= 400_000 {
                return Err(too_many_span_outputs("ExtrudeSubD"));
            }
            let id = match target {
                Target::Whole(id) | Target::Face(id, _) | Target::Edge(id, _) => id,
            };
            let entry = grouped.entry(id).or_default();
            match target {
                Target::Whole(_) => entry.0 = true,
                Target::Face(_, f) => {
                    entry.1.insert(f);
                }
                Target::Edge(_, e) => {
                    entry.2.insert(e);
                }
            }
        }
        if grouped.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut sources = vec![];
        let mut outputs = vec![];
        let mut archives = 0usize;
        for (id, (whole, faces, edges)) in grouped {
            let object = doc
                .object(id)
                .filter(|_| doc.is_object_selectable(id))
                .ok_or(CommandError::ExtrudeSubDStale)?;
            let Geometry::SubD(s) = object.geometry() else {
                return Err(CommandError::UnsupportedExtrudeSubDGeometry);
            };
            if faces.iter().any(|&i| i >= s.faces().len())
                || edges
                    .iter()
                    .any(|&i| i >= s.edges().len() || s.edge_face_counts()[i] != 1)
            {
                return Err(CommandError::Usage(arguments::USAGE));
            }
            let selection = if whole {
                SubDExtrudeSelection::AllFaces
            } else {
                SubDExtrudeSelection::Components {
                    faces: faces.into_iter().collect(),
                    edges: edges.into_iter().collect(),
                }
            };
            let output =
                s.try_extrude(&selection, options.distance, options.geometry_direction()?)?;
            archives = archives.saturating_add(output.archive().len());
            if archives > 64 * 1024 * 1024 || outputs.len() >= MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("ExtrudeSubD"));
            }
            outputs.push((id, Geometry::SubD(output)));
            sources.push(Source {
                id,
                geometry: object.geometry_snapshot().clone(),
                attributes: object.attributes().clone(),
                groups: object.group_ids().to_vec(),
            });
        }
        Ok(Self {
            sources,
            outputs,
            selected: doc.selected_objects().map(|o| o.id()).collect(),
            tolerance: doc.tolerance(),
        })
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc
                .selected_objects()
                .map(|o| o.id())
                .eq(self.selected.iter().copied())
            && self.sources.iter().all(|s| {
                doc.is_object_selectable(s.id)
                    && doc.object(s.id).is_some_and(|o| {
                        o.geometry_snapshot() == &s.geometry
                            && o.attributes() == &s.attributes
                            && o.group_ids() == s.groups
                    })
            })
    }
    pub fn preview_edges(&self) -> Result<Vec<LineSegment>, CommandError> {
        let mut lines = vec![];
        for (_, g) in &self.outputs {
            let Geometry::SubD(s) = g else { unreachable!() };
            if lines.len().saturating_add(s.edges().len()) > 400_000 {
                return Err(too_many_span_outputs("ExtrudeSubD"));
            }
            lines.extend(s.edges().iter().filter_map(|e| {
                LineSegment::try_new(
                    s.control_points()[e[0] as usize],
                    s.control_points()[e[1] as usize],
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .ok()
            }));
        }
        Ok(lines)
    }
    pub fn commit(&self, doc: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(doc, "ExtrudeSubD", |doc| self.apply(doc))
    }
    fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::ExtrudeSubDStale);
        }
        doc.replace_object_geometries(self.outputs.iter().cloned())?;
        Ok(format!("Extruded {} SubD object(s)", self.sources.len()))
    }
}
/// Bound explicit component targets before expanding them across selected objects.
pub fn selected_targets(
    doc: &Document,
    selection: Option<&SubDExtrudeSelection>,
) -> Result<Vec<Target>, CommandError> {
    let mut targets = vec![];
    for o in doc.selected_objects() {
        let added = match selection {
            Some(SubDExtrudeSelection::Components { faces, edges }) => {
                faces.len().saturating_add(edges.len())
            }
            _ => 1,
        };
        if targets.len().saturating_add(added) > 400_000 {
            return Err(too_many_span_outputs("ExtrudeSubD"));
        }
        match selection {
            Some(SubDExtrudeSelection::Components { faces, edges }) => {
                targets.extend(faces.iter().map(|&f| Target::Face(o.id(), f)));
                targets.extend(edges.iter().map(|&e| Target::Edge(o.id(), e)));
            }
            _ => targets.push(Target::Whole(o.id())),
        }
    }
    Ok(targets)
}
#[derive(Default)]
pub(super) struct ExtrudeSubDCommand(remembered::Remembered<Options>);
impl Command for ExtrudeSubDCommand {
    fn name(&self) -> &'static str {
        "ExtrudeSubD"
    }
    fn extrude_subd_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_extrude_subd_options(&self, options: Options) -> bool {
        self.0.set(options);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = parse(args, self.0.get())?;
        let targets = selected_targets(doc, input.selection.as_ref())?;
        let plan = Plan::prepare(doc, targets, input.options)?;
        self.0.set(input.options);
        plan.apply(doc)
    }
}
impl CommandRegistry {
    pub fn extrude_subd_options(&self) -> Options {
        self.commands
            .iter()
            .find_map(|c| c.extrude_subd_options())
            .unwrap_or_default()
    }
    pub fn remember_extrude_subd_options(&self, options: Options) {
        for c in &self.commands {
            if c.remember_extrude_subd_options(options) {
                break;
            }
        }
    }
}
