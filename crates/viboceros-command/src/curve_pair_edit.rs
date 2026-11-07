//! Immutable curve candidates and atomic document admission shared by pair commands.
use super::*;
use std::sync::Arc;
use viboceros_document::{LayerId, Object};
use viboceros_geometry::Curve3;

#[derive(Clone, Debug)]
pub struct Prepared {
    command: &'static str,
    sources: [Object; 2],
    tolerance: Tolerance,
    layer: LayerId,
    outputs: Arc<[Geometry]>,
    targets: Vec<Target>,
    deleted: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Target {
    Source(usize),
    New,
    FirstAttributes,
}

pub(crate) struct Plan {
    pub outputs: Vec<Geometry>,
    pub targets: Vec<Target>,
    pub deleted: Vec<usize>,
}

impl Prepared {
    pub(crate) fn new(
        document: &Document,
        sources: [Object; 2],
        plan: Plan,
        command: &'static str,
    ) -> Self {
        Self {
            command,
            sources,
            tolerance: document.tolerance(),
            layer: document.current_layer_id(),
            outputs: plan.outputs.into(),
            targets: plan.targets,
            deleted: plan.deleted,
        }
    }

    pub fn geometries(&self) -> &[Geometry] {
        &self.outputs
    }
    pub fn is_current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance()
            && self.layer == document.current_layer_id()
            && self.sources.iter().all(|old| {
                document.is_object_selectable(old.id())
                    && document.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
    pub fn commit(&self, document: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        run_command_transaction(document, self.command, |doc| self.apply(doc))
    }
    pub(crate) fn apply(&self, document: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::Usage(
                "Curve sources or document settings changed; pick again",
            ));
        }
        let selected = document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        document.release_command_selection_on_history_replay(selected)?;
        document.clear_selection();
        let mut outputs = Vec::new();
        for (target, geometry) in self.targets.iter().zip(self.outputs.iter()) {
            outputs.push(match *target {
                Target::Source(i) => {
                    let id = self.sources[i].id();
                    document.replace_object_geometries([(id, geometry.clone())])?;
                    id
                }
                Target::New => document.add_geometry(geometry.clone())?,
                Target::FirstAttributes => document.add_geometry_with_attributes(
                    geometry.clone(),
                    self.sources[0].attributes().clone(),
                )?,
            });
        }
        for &index in &self.deleted {
            document.delete_object(self.sources[index].id())?;
        }
        document.clear_selection();
        Ok(outputs)
    }
}

// A trimmed line keeps the source's parameter chart, including when the
// connected-end producer rebuilt an oriented terminal segment.
pub(crate) fn retain_line_domain(
    source: &Curve3,
    result: &mut Curve3,
    tolerance: Tolerance,
) -> Result<(), CommandError> {
    if let (Curve3::Line(source), Curve3::Line(line)) = (source, result) {
        let direction = source.direction(tolerance)?.as_vector();
        let length = source.length()?;
        let domain = source.domain();
        let parameter = |p| -> Result<Real, CommandError> {
            if p == source.start() {
                return Ok(*domain.start());
            }
            if p == source.end() {
                return Ok(*domain.end());
            }
            let fraction = source.start().vector_to(p)?.dot(direction)? / length;
            Ok(fraction.mul_add(*domain.end() - *domain.start(), *domain.start()))
        };
        *line = line.try_reparameterized(parameter(line.start())?..=parameter(line.end())?)?;
    }
    Ok(())
}
