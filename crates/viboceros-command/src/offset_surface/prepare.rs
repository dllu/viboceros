//! A prepared candidate owns its sources and commits without recomputing geometry.
use super::*;
use viboceros_document::Object;

pub struct Prepared {
    sources: Vec<Object>,
    outputs: Vec<(ObjectId, Geometry)>,
    options: Options,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    units: viboceros_geometry::LengthUnitSystem,
    closed_sources: BTreeSet<ObjectId>,
}

pub fn prepare(
    document: &Document,
    options: Options,
    flips: &BTreeSet<ObjectId>,
) -> Result<Prepared, CommandError> {
    let options = options
        .validate()?
        .normalize_tolerance(document.tolerance());
    let sources = document.selected_objects().cloned().collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    if sources.len() > MAX_SPAN_OUTPUT_OBJECTS {
        return Err(too_many_span_outputs("OffsetSrf"));
    }
    if options.loose
        && sources
            .iter()
            .any(|s| !loose_available(s.geometry(), document.tolerance()))
    {
        return Err(viboceros_geometry::GeometryError::UnsupportedLooseSurfaceOffset {
            context: "Loose is available only when every input is a single surface without interior creases",
        }.into());
    }
    let ids = sources.iter().map(Object::id).collect::<BTreeSet<_>>();
    if !flips.is_subset(&ids) {
        return Err(CommandError::OffsetSurfaceStale);
    }
    let mut outputs = Vec::with_capacity(sources.len());
    let mut closed_sources = BTreeSet::new();
    for source in &sources {
        if !document.is_object_selectable(source.id()) {
            return Err(CommandError::OffsetSurfaceStale);
        }
        let mut directed = options;
        if options.loose
            && options.solid
            && match source.geometry() {
                Geometry::Brep(b) => b.is_solid(),
                Geometry::NurbsSurface(s) => {
                    Brep::try_surface_face(s.clone(), document.tolerance())?.is_solid()
                }
                _ => false,
            }
        {
            closed_sources.insert(source.id());
        }
        if options.flip_all ^ flips.contains(&source.id()) {
            directed.distance = -directed.distance;
        }
        // The public command admits separate skins for an open two-sided offset.
        // The kernel's combined two-face result remains useful for solid walls.
        if options.distance == 0. && options.solid {
            outputs.extend(
                zero::wall(
                    source.geometry(),
                    options.both_sides || options.loose,
                    document.tolerance(),
                )?
                .into_iter()
                .map(|g| (source.id(), g)),
            );
        } else if options.both_sides && !options.solid {
            directed.both_sides = false;
            for sign in [1., -1.] {
                outputs.push((
                    source.id(),
                    geometry::offset(
                        source.geometry(),
                        Options {
                            distance: directed.distance * sign,
                            ..directed
                        },
                        document.tolerance(),
                    )?,
                ));
            }
        } else {
            outputs.push((
                source.id(),
                geometry::offset(source.geometry(), directed, document.tolerance())?,
            ));
        }
    }
    if outputs.len() > MAX_SPAN_OUTPUT_OBJECTS {
        return Err(too_many_span_outputs("OffsetSrf"));
    }
    Ok(Prepared {
        sources,
        outputs,
        options,
        tolerance: document.tolerance(),
        layer: document.current_layer_id(),
        units: document.units().clone(),
        closed_sources,
    })
}

impl Prepared {
    /// Source deletion changes admission only; the prepared geometry is reusable.
    pub fn set_delete_input(&mut self, delete: bool) {
        self.options.delete_input = delete;
    }
    pub fn preview(&self, document: &Document) -> Result<Document, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::OffsetSurfaceStale);
        }
        let mut scene = document.clone();
        for (_, geometry) in &self.outputs {
            scene.add_geometry(geometry.clone())?;
        }
        scene.clear_history()?;
        Ok(scene)
    }
    pub fn geometries(&self) -> impl Iterator<Item = &Geometry> {
        self.outputs.iter().map(|(_, g)| g)
    }
    pub fn is_current(&self, document: &Document) -> bool {
        document.tolerance() == self.tolerance
            && document.current_layer_id() == self.layer
            && document.units() == &self.units
            && document
                .selected_objects()
                .map(Object::id)
                .eq(self.sources.iter().map(Object::id))
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
    pub fn commit(
        &self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        run_command_transaction(document, "OffsetSrf", |doc| self.apply(doc, postselected))
    }
    pub(super) fn apply(
        &self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::OffsetSurfaceStale);
        }
        if postselected {
            document
                .release_command_selection_on_history_replay(self.sources.iter().map(Object::id))?;
            document.clear_selection();
        }
        if self.options.solid
            && self.options.delete_input
            && self.options.distance == 0.
            && !self.options.both_sides
            && !self.options.loose
        {
            document.copy_object_pieces_into_source_groups(self.outputs.iter().cloned())?;
            document.delete_objects(self.sources.iter().map(Object::id))?;
        } else {
            let replaces = |id| {
                self.options.solid
                    && (self.options.delete_input || self.closed_sources.contains(&id))
            };
            let replaced = self
                .outputs
                .iter()
                .filter(|(id, _)| replaces(*id))
                .cloned()
                .collect::<Vec<_>>();
            if !replaced.is_empty() {
                document.replace_object_geometries(replaced)?;
            }
            for (_, geometry) in self.outputs.iter().filter(|(id, _)| !replaces(*id)) {
                document.add_geometry(geometry.clone())?;
            }
        }
        Ok(format!("Offset {} surface(s)", self.outputs.len()))
    }
}
