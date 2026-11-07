//! Compute once, then admit the same candidate atomically.
use super::*;
use std::sync::Arc;
use viboceros_document::{LayerId, Object};

#[derive(Clone, Debug)]
pub struct Prepared {
    sources: [Object; 2],
    tolerance: Tolerance,
    layer: LayerId,
    outputs: Arc<[Geometry]>,
    targets: Vec<Target>,
    deleted: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Source(usize),
    New,
    FirstAttributes,
}

pub(super) struct Plan {
    pub outputs: Vec<Geometry>,
    pub targets: Vec<Target>,
    pub deleted: Vec<usize>,
}

pub fn prepare(
    document: &Document,
    picks: [(ObjectId, Point3); 2],
    preferences: Preferences,
) -> Result<Prepared, CommandError> {
    let preferences = preferences.validate()?;
    let sources = picks.map(|(id, _)| {
        if !document.is_object_selectable(id) {
            return Err(CommandError::FilletRequiresTwoCurves);
        }
        document
            .object(id)
            .cloned()
            .ok_or(CommandError::FilletRequiresTwoCurves)
    });
    let [first, second] = sources;
    let sources = [first?, second?];
    let curve = |index: usize| {
        sources[index]
            .geometry()
            .curve_ref()
            .ok_or(CommandError::FilletRequiresTwoCurves)
            .map(|c| c.to_owned())
    };
    let first = curve(0)?;
    let second = curve(1)?;
    let closed = [first.as_ref().is_closed()?, second.as_ref().is_closed()?];
    let plan = if picks[0].0 == picks[1].0 {
        super::same_source::prepare(
            &first,
            picks.map(|(_, p)| p),
            preferences,
            document.tolerance(),
        )?
    } else if closed.iter().any(|c| *c) {
        super::closed::prepare(
            [&first, &second],
            picks.map(|(_, p)| p),
            closed,
            preferences,
            document.tolerance(),
        )?
    } else {
        let outputs = if preferences.join && preferences.trim {
            let joined = super::super::curve_connections::fillet_joined(
                (&first, picks[0].1),
                (&second, picks[1].1),
                preferences.radius,
                preferences.styles,
                document.tolerance(),
            )?;
            let source = first.as_ref();
            let domain = source.domain();
            let a = source.evaluate(*domain.start())?;
            let b = source.evaluate(*domain.end())?;
            let origin = if picks[0].1.distance_to(b)? < picks[0].1.distance_to(a)? {
                *domain.start()
            } else {
                -*domain.end()
            };
            let width = *joined.domain().end() - *joined.domain().start();
            vec![Geometry::PolyCurve(
                joined.try_reparameterized(origin..=origin + width)?,
            )]
        } else {
            let mut parts = super::super::curve_connections::fillet_parts(
                (&first, picks[0].1),
                (&second, picks[1].1),
                preferences.radius,
                preferences.trim,
                preferences.styles,
                document.tolerance(),
            )?;
            if preferences.trim {
                for (source, result) in [&first, &second].into_iter().zip(parts.iter_mut()) {
                    retain_line_domain(source, result, document.tolerance())?;
                }
            }
            parts.into_iter().map(Geometry::from).collect()
        };
        let targets = if preferences.trim {
            if preferences.join {
                vec![Target::Source(0)]
            } else {
                (0..outputs.len())
                    .map(|i| {
                        if i < 2 {
                            Target::Source(i)
                        } else {
                            Target::New
                        }
                    })
                    .collect()
            }
        } else {
            vec![Target::New; outputs.len()]
        };
        Plan {
            outputs,
            targets,
            deleted: if preferences.trim && preferences.join {
                vec![1]
            } else {
                vec![]
            },
        }
    };
    Ok(Prepared {
        sources,
        tolerance: document.tolerance(),
        layer: document.current_layer_id(),
        outputs: plan.outputs.into(),
        targets: plan.targets,
        deleted: plan.deleted,
    })
}

// A trimmed line keeps the source's parameter chart, including when the
// connected-end producer rebuilt an oriented terminal segment.
fn retain_line_domain(
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

impl Prepared {
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
        run_command_transaction(document, "Fillet", |doc| self.apply(doc))
    }
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::Usage(
                "Fillet sources or document settings changed; pick again",
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
