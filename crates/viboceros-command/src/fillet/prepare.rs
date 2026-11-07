//! Compute once, then admit the same candidate atomically.
use super::*;
use std::sync::Arc;
use viboceros_document::{LayerId, Object};

#[derive(Clone, Debug)]
pub struct Prepared {
    sources: [Object; 2],
    preferences: Preferences,
    tolerance: Tolerance,
    layer: LayerId,
    outputs: Arc<[Geometry]>,
}

pub fn prepare(
    document: &Document,
    picks: [(ObjectId, Point3); 2],
    preferences: Preferences,
) -> Result<Prepared, CommandError> {
    let preferences = preferences.validate()?;
    if picks[0].0 == picks[1].0 {
        return Err(CommandError::FilletRequiresTwoCurves);
    }
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
    Ok(Prepared {
        sources,
        preferences,
        tolerance: document.tolerance(),
        layer: document.current_layer_id(),
        outputs: outputs.into(),
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
        let first = self.sources[0].id();
        let second = self.sources[1].id();
        let selected = document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        document.release_command_selection_on_history_replay(selected)?;
        document.clear_selection();
        let mut outputs = Vec::new();
        if self.preferences.trim {
            if self.preferences.join {
                document.replace_object_geometries([(first, self.outputs[0].clone())])?;
                document.delete_object(second)?;
                outputs.push(first);
            } else {
                document.replace_object_geometries([
                    (first, self.outputs[0].clone()),
                    (second, self.outputs[1].clone()),
                ])?;
                outputs.extend([first, second]);
                for geometry in &self.outputs[2..] {
                    outputs.push(document.add_geometry(geometry.clone())?);
                }
            }
        } else {
            for geometry in self.outputs.iter() {
                outputs.push(document.add_geometry(geometry.clone())?);
            }
        }
        document.clear_selection();
        Ok(outputs)
    }
}
