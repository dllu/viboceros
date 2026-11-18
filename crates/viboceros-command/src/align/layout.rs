//! Cache the source bounds and projection stations independently of the cursor.
use super::*;
use crate::component_selection::{ComponentPick, ComponentSelectionKind};
use crate::placement::{PickPolicy, Placement, Sources, Unit};
use std::collections::BTreeSet;
use viboceros_document::{Geometry, GeometrySnapshot, ObjectId};
use viboceros_geometry::GeometryError;

#[derive(Clone, Debug)]
enum Stations {
    Bounds {
        frame: Frame3,
        positions: Vec<BoundingBox3>,
        combined: BoundingBox3,
    },
    Projections(Vec<Point3>),
}

#[derive(Clone, Debug)]
pub struct AlignmentSources(Sources);

#[derive(Clone, Debug)]
pub struct AlignmentLayout {
    sources: Sources,
    orientation: Frame3,
    mode: AlignmentMode,
    units: Vec<Unit>,
    stations: Stations,
}

#[derive(Clone, Debug)]
pub struct AlignedPlacement {
    sources: Sources,
    placement: Placement,
    target: Option<(ObjectId, GeometrySnapshot)>,
    mode: AlignmentMode,
    count: usize,
    projected: bool,
}

pub fn supports_component(kind: ComponentSelectionKind) -> bool {
    use ComponentSelectionKind::*;
    matches!(kind, MeshVertex | SubDVertex | SubDEdge | SubDFace)
}

impl AlignmentSources {
    pub fn capture(document: &Document, picks: &[ComponentPick]) -> Self {
        use ComponentSelectionKind::*;
        let mut vertices = BTreeSet::new();
        for &pick in picks {
            if matches!(pick.kind, SubDVertex | MeshVertex) {
                vertices.insert(pick);
                continue;
            }
            let Some(Geometry::SubD(subd)) = document.object(pick.object).map(|o| o.geometry())
            else {
                continue;
            };
            let indices = match pick.kind {
                SubDFace => subd.faces().get(pick.index).map(Vec::as_slice),
                SubDEdge => subd.edges().get(pick.index).map(|edge| edge.as_slice()),
                _ => None,
            };
            if let Some(indices) = indices {
                vertices.extend(indices.iter().map(|&i| ComponentPick {
                    object: pick.object,
                    kind: SubDVertex,
                    index: i as usize,
                }));
            }
        }
        Self(Sources::capture_with_subd_controls(
            document,
            &vertices.into_iter().collect::<Vec<_>>(),
            true,
        ))
    }

    pub fn is_current(&self, document: &Document) -> bool {
        self.0.is_current(document)
    }

    pub fn includes_owner(&self, id: ObjectId) -> bool {
        self.0.includes_owner(id)
    }

    pub fn prepare(
        &self,
        options: AlignmentOptions,
        plane: Frame3,
    ) -> Result<AlignmentLayout, CommandError> {
        let mode = options.mode.ok_or(CommandError::Usage(USAGE))?;
        let orientation = if options.world {
            CommandContext::default().construction_plane
        } else {
            plane
        };
        let units = if options.projects() {
            self.0.individual_units()
        } else {
            self.0.units().to_vec()
        };
        let stations = if options.projects() {
            if mode == AlignmentMode::ToFitPlane {
                let count = units.len();
                if count < 3 {
                    return Err(CommandError::InsufficientPlaneAlignmentObjects { actual: count });
                }
                if count > viboceros_geometry::MAX_PLANE_FIT_POINTS {
                    return Err(GeometryError::PlaneFitResourceLimit {
                        maximum: viboceros_geometry::MAX_PLANE_FIT_POINTS,
                    }
                    .into());
                }
            }
            if units.is_empty() {
                return Err(CommandError::NoObjectsSelected);
            }
            let positions = units
                .iter()
                .map(|unit| {
                    let origin = self.0.origin(unit)?;
                    if matches!(unit, Unit::Grip(_)) {
                        return Ok(origin);
                    }
                    // Each object owns its arithmetic origin, independent of the display plane.
                    let frame = orientation.with_origin(origin);
                    let bounds = self.0.bounds(unit, frame)?;
                    let mut local = bounds.center()?.to_array();
                    local[2] = bounds.min().z();
                    frame.point_at(local)
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            Stations::Projections(positions)
        } else {
            let first = units.first().ok_or(CommandError::NoObjectsSelected)?;
            let frame = orientation.with_origin(self.0.origin(first)?);
            let positions = units
                .iter()
                .map(|unit| self.0.bounds(unit, frame))
                .collect::<Result<Vec<_>, GeometryError>>()?;
            let combined = positions
                .iter()
                .copied()
                .skip(1)
                .try_fold(positions[0], BoundingBox3::union)?;
            Stations::Bounds {
                frame,
                positions,
                combined,
            }
        };
        Ok(AlignmentLayout {
            sources: self.0.clone(),
            orientation,
            mode,
            units,
            stations,
        })
    }
}

impl AlignmentLayout {
    pub fn placement(
        &self,
        document: &Document,
        options: AlignmentOptions,
    ) -> Result<AlignedPlacement, CommandError> {
        if (options.projects() && !options.ready()) || options.mode != Some(self.mode) {
            return Err(CommandError::Usage(USAGE));
        }
        let target = options
            .curve
            .map(|id| {
                if self.sources.includes_owner(id) {
                    return Err(CommandError::AlignmentTargetSelected);
                }
                let object = document
                    .object(id)
                    .ok_or(viboceros_document::DocumentError::ObjectNotFound(id))?;
                if object.geometry().curve_ref().is_none() {
                    return Err(CommandError::AlignmentTargetNotCurve);
                }
                Ok((id, object.geometry_snapshot().clone()))
            })
            .transpose()?;
        let placement = match &self.stations {
            Stations::Bounds {
                frame,
                positions,
                combined,
            } => {
                let target = options
                    .target
                    .map(|p| frame.coordinates_of(p))
                    .transpose()?;
                self.sources
                    .layout_placement(self.units.iter().zip(positions).map(|(unit, bounds)| {
                        let offset = offsets(self.mode, *bounds, *combined, target);
                        Ok((unit, translation(*frame, offset)?))
                    }))?
            }
            Stations::Projections(positions) => {
                // Exactly three grip stations already lie in their fitted plane.
                let destinations = if self.mode == AlignmentMode::ToFitPlane
                    && positions.len() == 3
                    && self.units.iter().all(|unit| matches!(unit, Unit::Grip(_)))
                {
                    positions.clone()
                } else {
                    projection::destinations(
                        positions,
                        options,
                        self.orientation,
                        target.as_ref().map(|(_, value)| &**value),
                        self.sources.tolerance(),
                    )?
                };
                self.sources.layout_placement(
                    self.units
                        .iter()
                        .zip(positions.iter().zip(destinations))
                        .map(|(unit, (&from, to))| Ok((unit, from.vector_to(to)?))),
                )?
            }
        };
        Ok(AlignedPlacement {
            sources: self.sources.clone(),
            placement,
            target,
            mode: self.mode,
            count: self.units.len(),
            projected: matches!(self.stations, Stations::Projections(_)),
        })
    }
}

impl AlignedPlacement {
    pub fn preview(&self) -> &Placement {
        &self.placement
    }

    pub fn target_is_current(&self, document: &Document) -> bool {
        self.target.as_ref().is_none_or(|(id, witness)| {
            document
                .object(*id)
                .is_some_and(|object| object.geometry_snapshot().shares_storage_with(witness))
        })
    }

    pub fn commit(
        self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        crate::run_command_transaction(document, "Align", |doc| self.apply(doc, postselected))
    }

    pub(super) fn apply(
        self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        if !self.sources.is_current(document) || !self.target_is_current(document) {
            return Err(CommandError::AlignStale);
        }
        let changed = self.placement.apply(
            document,
            if postselected {
                PickPolicy::Clear
            } else {
                PickPolicy::Preserve
            },
        )?;
        let label = if self.projected {
            "object(s)"
        } else {
            "object/group unit(s)"
        };
        Ok(format!(
            "Aligned {} {label} {}; moved {changed} object(s)",
            self.count,
            self.mode.label()
        ))
    }
}
