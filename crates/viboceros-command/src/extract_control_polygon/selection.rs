//! Complete geometry and source identities are staged before a transaction.
use super::*;
use viboceros_document::{GeometrySnapshot, GroupId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Whole(ObjectId),
    CurveRange(ObjectId, [Real; 2]),
}
impl Target {
    pub fn object(self) -> ObjectId {
        match self {
            Self::Whole(id) | Self::CurveRange(id, _) => id,
        }
    }
}
#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    geometry: GeometrySnapshot,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
}
#[derive(Clone, Debug)]
struct Output {
    source: ObjectId,
    geometry: Geometry,
    inherit_attributes: bool,
}
#[derive(Clone, Debug)]
pub struct Selection {
    sources: Vec<Source>,
    outputs: Vec<Output>,
    tolerance: Tolerance,
    output_layer: Option<viboceros_document::LayerId>,
    copy_to_current: bool,
}
impl Selection {
    pub fn prepare(
        document: &Document,
        targets: impl IntoIterator<Item = Target>,
        options: Options,
    ) -> Result<Self, CommandError> {
        let mut targets = targets
            .into_iter()
            .take(MAX_SPAN_OUTPUT_OBJECTS + 1)
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if targets.len() > MAX_SPAN_OUTPUT_OBJECTS {
            return Err(too_many_span_outputs("ExtractControlPolygon"));
        }
        // Explicit portions replace whole-object selection on their owner.
        let portions = targets
            .iter()
            .filter(|t| !matches!(t, Target::Whole(_)))
            .map(|t| t.object())
            .collect::<BTreeSet<_>>();
        targets.retain(|t| !matches!(t,Target::Whole(id) if portions.contains(id)));
        let mut sources = Vec::new();
        let mut outputs = Vec::new();
        let mut seen = BTreeSet::new();
        let mut whole = BTreeSet::new();
        let by_id = document
            .selectable_objects()
            .map(|o| (o.id(), o))
            .collect::<BTreeMap<_, _>>();
        let tolerance = document.tolerance();
        let mut eligible = false;
        for target in targets {
            if let Target::Whole(id) = target
                && !whole.insert(id)
            {
                continue;
            }
            let id = target.object();
            let source = by_id.get(&id).ok_or(CommandError::ControlPolygonStale)?;
            let geometry = source.geometry();
            if matches!(target, Target::Whole(_)) && !accepts(geometry) {
                continue;
            }
            eligible = true;
            let output = match target {
                Target::Whole(_) => control_geometry(geometry, options, tolerance)?,
                Target::CurveRange(_, parameters) => {
                    if parameters.iter().any(|p| !p.is_finite()) {
                        return Err(CommandError::Usage(options::USAGE));
                    }
                    let curve = geometry
                        .curve_ref()
                        .ok_or(CommandError::UnsupportedExtractControlPolygonGeometry)?;
                    let piece = Geometry::from(
                        curve
                            .to_owned()
                            .try_subcurve(parameters[0], parameters[1])?,
                    );
                    control_geometry(&piece, options, tolerance)?
                }
            };
            if let Some(output) = output {
                outputs.push(Output {
                    source: id,
                    geometry: output,
                    inherit_attributes: !matches!(target, Target::CurveRange(..)),
                });
            }
            if seen.insert(id) {
                sources.push(Source {
                    id,
                    geometry: source.geometry_snapshot().clone(),
                    attributes: source.attributes().clone(),
                    groups: source.group_ids().to_vec(),
                });
            }
        }
        if outputs.is_empty() {
            return Err(if eligible {
                CommandError::NoExtractableControlPolygons
            } else {
                CommandError::UnsupportedExtractControlPolygonGeometry
            });
        }
        let needs_current_layer = options.output_layer == OutputLayer::Current
            || outputs.iter().any(|o| !o.inherit_attributes);
        Ok(Self {
            sources,
            outputs,
            tolerance,
            output_layer: needs_current_layer.then_some(document.current_layer_id()),
            copy_to_current: options.output_layer == OutputLayer::Current,
        })
    }
    pub fn commit(self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "ExtractControlPolygon", move |doc| {
            self.apply(doc)
        })
    }
    pub(super) fn apply(self, document: &mut Document) -> Result<String, CommandError> {
        let by_id = document
            .selectable_objects()
            .map(|o| (o.id(), o))
            .collect::<BTreeMap<_, _>>();
        if document.tolerance() != self.tolerance
            || self
                .output_layer
                .is_some_and(|id| document.current_layer_id() != id || document.layer(id).is_none())
            || self.sources.iter().any(|s| {
                by_id.get(&s.id).is_none_or(|o| {
                    o.geometry_snapshot() != &s.geometry
                        || o.attributes() != &s.attributes
                        || o.group_ids() != s.groups
                })
            })
        {
            return Err(CommandError::ControlPolygonStale);
        }
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let owners = self.sources.iter().map(|s| s.id).collect::<BTreeSet<_>>();
        let count = self.outputs.len();
        document.release_command_selection_on_history_replay(
            selected.iter().copied().chain(owners.iter().copied()),
        )?;
        let mut outputs = Vec::with_capacity(count);
        let mut pending = self.outputs.into_iter().peekable();
        while let Some(output) = pending.next() {
            if output.inherit_attributes {
                // Batch consecutive source copies without changing result order.
                let mut pieces = vec![(output.source, output.geometry)];
                while pending.peek().is_some_and(|o| o.inherit_attributes) {
                    let next = pending.next().unwrap();
                    pieces.push((next.source, next.geometry));
                }
                let ids = document.copy_object_pieces_into_source_groups(pieces)?;
                if self.copy_to_current
                    && let Some(layer) = self.output_layer
                {
                    document.set_objects_layer(ids.iter().copied(), layer)?;
                }
                outputs.extend(ids);
            } else {
                // Native temporary references have default attributes and no groups.
                outputs.push(document.add_geometry(output.geometry)?);
            }
        }
        document.release_command_selection_on_history_replay(outputs.iter().copied())?;
        document.select_command_results(
            selected
                .into_iter()
                .filter(|id| !owners.contains(id))
                .chain(outputs),
        )?;
        Ok(format!(
            "Extracted {count} control polygon(s) from {} object(s)",
            self.sources.len()
        ))
    }
}
