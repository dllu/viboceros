//! Native editable SubD offsets, staged before any document mutation.
use crate::*;
use viboceros_document::{GeometrySnapshot, GroupId};
mod arguments;
#[cfg(test)]
mod tests;
pub use arguments::{Input, Options, parse};
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
    options: Options,
    tolerance: Tolerance,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        options: Options,
        flips: &BTreeSet<ObjectId>,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        let mut sources = vec![];
        let mut outputs = vec![];
        let mut archives = 0usize;
        for object in doc.selected_objects() {
            if !doc.is_object_selectable(object.id()) {
                return Err(CommandError::OffsetSubDStale);
            }
            let Geometry::SubD(s) = object.geometry() else {
                return Err(CommandError::UnsupportedOffsetSubDGeometry);
            };
            let distance = if options.flip_all ^ flips.contains(&object.id()) {
                -options.distance
            } else {
                options.distance
            };
            let parts = if options.both_sides && options.solid {
                // Rhino performs a second offset on the first offset skin;
                // its re-evaluated normals intentionally change the far side.
                vec![
                    s.try_offset(distance, false)?
                        .try_offset(-2. * distance, true)?,
                ]
            } else if options.both_sides {
                vec![
                    s.try_offset(distance, false)?,
                    s.try_offset(-distance, false)?,
                ]
            } else {
                vec![s.try_offset(distance, options.solid)?]
            };
            for part in parts {
                archives = archives.saturating_add(part.archive().len());
                if archives > 64 * 1024 * 1024 || outputs.len() >= MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs("OffsetSubD"));
                }
                outputs.push((object.id(), Geometry::SubD(part)));
            }
            sources.push(Source {
                id: object.id(),
                geometry: object.geometry_snapshot().clone(),
                attributes: object.attributes().clone(),
                groups: object.group_ids().to_vec(),
            });
        }
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let source_ids = sources.iter().map(|s| s.id).collect::<BTreeSet<_>>();
        if !flips.is_subset(&source_ids) {
            return Err(CommandError::OffsetSubDStale);
        }
        Ok(Self {
            sources,
            outputs,
            options,
            tolerance: doc.tolerance(),
        })
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        doc.tolerance() == self.tolerance
            && doc
                .selected_objects()
                .map(|o| o.id())
                .eq(self.sources.iter().map(|s| s.id))
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
        let mut edges = vec![];
        for (_, geometry) in &self.outputs {
            let Geometry::SubD(s) = geometry else {
                unreachable!()
            };
            if edges.len().saturating_add(s.edges().len()) > 400_000 {
                return Err(too_many_span_outputs("OffsetSubD"));
            }
            for edge in s.edges() {
                if let Ok(line) = LineSegment::try_new(
                    s.control_points()[edge[0] as usize],
                    s.control_points()[edge[1] as usize],
                    Tolerance::NUMERICAL_VALIDATION,
                ) {
                    edges.push(line);
                }
            }
        }
        Ok(edges)
    }
    pub fn commit(&self, doc: &mut Document, postselected: bool) -> Result<String, CommandError> {
        run_command_transaction(doc, "OffsetSubD", |doc| self.apply(doc, postselected))
    }
    fn apply(&self, doc: &mut Document, postselected: bool) -> Result<String, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::OffsetSubDStale);
        }
        if postselected && self.options.delete_input {
            doc.release_command_selection_on_history_replay(self.sources.iter().map(|s| s.id))?;
            doc.clear_selection();
        }
        doc.copy_object_pieces_into_source_groups(self.outputs.iter().cloned())?;
        if self.options.delete_input {
            doc.delete_objects(self.sources.iter().map(|s| s.id))?;
        }
        Ok(format!("Offset {} SubD object(s)", self.sources.len()))
    }
}
#[derive(Default)]
pub(super) struct OffsetSubDCommand(remembered::Remembered<Options>);
impl Command for OffsetSubDCommand {
    fn name(&self) -> &'static str {
        "OffsetSubD"
    }
    fn offset_subd_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_offset_subd_options(&self, options: Options) -> bool {
        self.0.set(options.remembered());
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, true)
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, false)
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let _ = parse(args, self.0.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: "OffsetSubD",
            filter: ObjectSelectionFilter::SubD,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            choices: vec![],
            menus: vec![],
        }))
    }
}
impl OffsetSubDCommand {
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let input = parse(args, self.0.get())?;
        self.0.set(input.options.remembered());
        Plan::prepare(doc, input.options, &BTreeSet::new())?.apply(doc, postselected)
    }
}
impl CommandRegistry {
    pub fn offset_subd_options(&self) -> Options {
        self.commands
            .iter()
            .find_map(|c| c.offset_subd_options())
            .unwrap_or_default()
    }
    pub fn remember_offset_subd_options(&self, options: Options) {
        for c in &self.commands {
            if c.remember_offset_subd_options(options) {
                break;
            }
        }
    }
}
