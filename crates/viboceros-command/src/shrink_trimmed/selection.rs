//! Prepared whole-object and face targets share one atomic shrink commit.
use super::*;
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Debug)]
struct Plan {
    id: ObjectId,
    source: GeometrySnapshot,
    replacement: Option<Brep>,
    whole: bool,
}

/// Staged geometry and immutable sources, independent of viewport picking.
#[derive(Clone, Debug)]
pub struct ShrinkTrimmedSelection {
    mode: BrepSurfaceShrinkMode,
    tolerance: Tolerance,
    plans: Vec<Plan>,
    retained: Vec<ObjectId>,
    shrunk: usize,
    unchanged: usize,
}

impl ShrinkTrimmedSelection {
    /// Include eligible whole-object document selection and explicit face
    /// targets. Face duplicates are idempotent; a whole target includes all its
    /// faces. Native ToEdge accepts whole objects only. Every source and index
    /// is validated before staging geometry or mutating the document.
    pub fn prepare(
        document: &Document,
        mode: BrepSurfaceShrinkMode,
        faces: impl IntoIterator<Item = (ObjectId, usize)>,
    ) -> Result<Self, CommandError> {
        let mut targets = BTreeMap::<ObjectId, Option<BTreeSet<usize>>>::new();
        let mut order = Vec::new();
        let mut retained = Vec::new();
        for object in document.selected_objects() {
            if ObjectSelectionFilter::SurfaceComponents.accepts_object(object) {
                targets.insert(object.id(), None);
                order.push(object.id());
            } else {
                retained.push(object.id());
            }
        }
        for (count, (id, face)) in faces.into_iter().enumerate() {
            if count >= 100_000 || mode == BrepSurfaceShrinkMode::ToEdge {
                return Err(CommandError::Usage(FACE_USAGE));
            }
            let object = document
                .object(id)
                .filter(|_| document.is_object_selectable(id))
                .ok_or(CommandError::ShrinkTrimmedUnavailable)?;
            let count = match object.geometry() {
                Geometry::Brep(brep) => brep.faces().len(),
                Geometry::NurbsSurface(_) => 1,
                _ => return Err(CommandError::ShrinkTrimmedUnavailable),
            };
            if face >= count {
                return Err(GeometryError::BrepFaceIndexOutOfRange {
                    face,
                    face_count: count,
                }
                .into());
            }
            let target = targets.entry(id).or_insert_with(|| {
                order.push(id);
                Some(BTreeSet::new())
            });
            if let Some(faces) = target {
                faces.insert(face);
            }
        }
        if order.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let tolerance = document.tolerance();
        let mut selection = Self {
            mode,
            tolerance,
            plans: Vec::with_capacity(order.len()),
            retained,
            shrunk: 0,
            unchanged: 0,
        };
        for id in order {
            let object = document.object(id).unwrap();
            let converted;
            let brep = match object.geometry() {
                Geometry::Brep(brep) => brep,
                Geometry::NurbsSurface(surface) => {
                    converted = Brep::try_surface_face_with_native_edge_parameters(
                        surface.clone(),
                        tolerance,
                    )?;
                    &converted
                }
                _ => unreachable!("validated surface target"),
            };
            let faces = targets.remove(&id).unwrap();
            let result = match &faces {
                None => brep.try_shrunk_surfaces(mode, tolerance)?,
                Some(faces) => brep.try_shrunk_surface_faces(
                    &faces.iter().copied().collect::<Vec<_>>(),
                    mode,
                    tolerance,
                )?,
            };
            let changed = result
                .faces()
                .iter()
                .zip(brep.faces())
                .filter(|(a, b)| a.surface() != b.surface())
                .count();
            selection.shrunk += changed;
            selection.unchanged +=
                faces.as_ref().map_or(brep.faces().len(), BTreeSet::len) - changed;
            selection.plans.push(Plan {
                id,
                source: object.geometry_snapshot().clone(),
                replacement: (changed != 0).then_some(result),
                whole: faces.is_none(),
            });
        }
        Ok(selection)
    }

    pub fn commit(
        &self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        run_command_transaction(document, command_name(self.mode), |doc| {
            self.apply(doc, postselected)
        })
    }

    pub(super) fn apply(
        &self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        if document.tolerance() != self.tolerance
            || self.plans.iter().any(|plan| {
                !document.is_object_selectable(plan.id)
                    || document
                        .object(plan.id)
                        .is_none_or(|object| object.geometry_snapshot() != &plan.source)
            })
        {
            return Err(CommandError::ShrinkTrimmedStale);
        }
        let staged = self
            .plans
            .iter()
            .filter_map(|p| {
                p.replacement
                    .as_ref()
                    .map(|b| (p.id, Geometry::Brep(b.clone())))
            })
            .collect::<Vec<_>>();
        let order = staged.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        if postselected && !staged.is_empty() {
            document
                .release_command_selection_on_history_replay(self.plans.iter().map(|p| p.id))?;
            document.clear_selection();
        }
        document.replace_object_geometries(staged)?;
        document.move_objects_to_end_in_order(order)?;
        if postselected {
            document.select_command_results(
                self.retained.iter().copied().chain(
                    self.plans
                        .iter()
                        .filter(|p| p.whole && p.replacement.is_none())
                        .map(|p| p.id),
                ),
            )?;
        }
        Ok(format!(
            "Shrunk {} surface(s); {} already shrunk",
            self.shrunk, self.unchanged
        ))
    }
}
