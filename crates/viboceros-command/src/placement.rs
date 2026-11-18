//! Shared object and grip placement for interactive layout commands.
use crate::CommandError;
use crate::component_selection::{ComponentPick, ComponentSelectionKind};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use viboceros_document::{
    ControlPointId, Document, Geometry, GeometrySnapshot, GripMode, Object, ObjectId, SelectionMode,
};
use viboceros_geometry::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, Point3, Tolerance, Vector3,
};

#[derive(Clone, Debug)]
pub(crate) enum Unit {
    Objects(Vec<ObjectId>),
    Grip(ControlPointId),
}

#[derive(Debug)]
struct Grip {
    point: Point3,
    mode: GripMode,
    aliases: Vec<usize>,
}

#[derive(Debug)]
struct Captured {
    units: Vec<Unit>,
    objects: BTreeMap<ObjectId, Object>,
    grips: BTreeMap<ControlPointId, Grip>,
    selection: Vec<ObjectId>,
    grip_selection: Vec<(ControlPointId, Point3)>,
    grip_modes: BTreeMap<ObjectId, GripMode>,
    tolerance: Tolerance,
    subd_control_net: Option<bool>,
    grip_owners: BTreeSet<ObjectId>,
}

#[derive(Clone, Debug)]
pub struct Sources(Arc<Captured>);

#[derive(Clone, Debug)]
pub struct Placement {
    sources: Sources,
    translations: BTreeMap<ObjectId, AffineTransform3>,
    edits: BTreeMap<ObjectId, GeometrySnapshot>,
    moved_grips: Vec<ControlPointId>,
}

pub(crate) enum PickPolicy {
    Preserve,
    ReleaseMoved,
    Clear,
}

impl Sources {
    /// A displayed grip takes precedence over a component alias on the same
    /// owner. Grip owners take precedence over simultaneously selected parents.
    pub fn capture(document: &Document, components: &[ComponentPick]) -> Self {
        Self::capture_with_subd_controls(document, components, false)
    }

    pub(crate) fn capture_with_subd_controls(
        document: &Document,
        components: &[ComponentPick],
        control_components: bool,
    ) -> Self {
        let grip_selection = document.selected_control_points().collect::<Vec<_>>();
        let grip_modes = grip_selection
            .iter()
            .filter_map(|(id, _)| document.grip_mode(id.object).map(|mode| (id.object, mode)))
            .collect::<BTreeMap<_, _>>();
        let control_owners = if control_components {
            components
                .iter()
                .filter(|p| p.kind == ComponentSelectionKind::SubDVertex)
                .map(|p| p.object)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let mut grips = BTreeMap::new();
        let mut grip_units = Vec::new();
        let mut add_grip = |id, point, mode, aliases| {
            if let std::collections::btree_map::Entry::Vacant(entry) = grips.entry(id) {
                entry.insert(Grip {
                    point,
                    mode,
                    aliases,
                });
                grip_units.push(Unit::Grip(id));
            }
        };
        for &(id, point) in &grip_selection {
            if control_owners.contains(&id.object)
                && let Some(Geometry::SubD(subd)) = document.object(id.object).map(Object::geometry)
                && let Some(&control) = subd.control_points().get(id.index)
            {
                add_grip(id, control, GripMode::Control, Vec::new());
            } else if let Some(&mode) = grip_modes.get(&id.object) {
                add_grip(id, point, mode, Vec::new());
            }
        }
        let mut mesh_topology = BTreeMap::new();
        let mut subd_control_net = None;
        for pick in components {
            if !document.is_object_selectable(pick.object) {
                continue;
            }
            let Some(owner) = document.object(pick.object) else {
                continue;
            };
            let (point, mode, index, aliases) = match (pick.kind, owner.geometry()) {
                (ComponentSelectionKind::MeshVertex, Geometry::Mesh(mesh)) => {
                    let topology = mesh_topology
                        .entry(pick.object)
                        .or_insert_with(|| mesh.topology_vertex_raw_indices());
                    let Some(raw) = topology.get(pick.index) else {
                        continue;
                    };
                    let index = raw[0];
                    (
                        mesh.vertices()[index],
                        GripMode::Control,
                        index,
                        raw[1..].to_vec(),
                    )
                }
                (ComponentSelectionKind::SubDVertex, Geometry::SubD(subd)) => {
                    let (mode, points) = if control_components {
                        (GripMode::Control, subd.control_points())
                    } else if let Some(mode) = document.grip_mode(pick.object) {
                        (mode, document.control_point_locations(pick.object).unwrap())
                    } else {
                        let control_net =
                            *subd_control_net.get_or_insert(document.subd_control_net());
                        if control_net {
                            (GripMode::Control, subd.control_points())
                        } else {
                            (GripMode::Edit, subd.surface_points())
                        }
                    };
                    let Some(&point) = points.get(pick.index) else {
                        continue;
                    };
                    (point, mode, pick.index, Vec::new())
                }
                _ => continue,
            };
            add_grip(
                ControlPointId {
                    object: pick.object,
                    index,
                },
                point,
                mode,
                aliases,
            );
        }

        let grip_owners = grips.keys().map(|id| id.object).collect::<BTreeSet<_>>();
        let mut objects = BTreeMap::new();
        let mut units = Vec::new();
        for group in crate::layout_units::selected_units(document) {
            let members = group.into_iter().filter(|o| !grip_owners.contains(&o.id()));
            let ids = members
                .map(|object| {
                    objects.insert(object.id(), object.clone());
                    object.id()
                })
                .collect::<Vec<_>>();
            if ids
                .iter()
                .any(|id| objects[id].geometry().bounds().is_some())
            {
                units.push(Unit::Objects(ids));
            }
        }
        for &id in &grip_owners {
            if let Some(owner) = document.object(id) {
                objects.insert(id, owner.clone());
            }
        }
        units.extend(grip_units);
        Self(Arc::new(Captured {
            units,
            objects,
            grips,
            selection: document.selected_object_ids().collect(),
            grip_selection,
            grip_modes,
            tolerance: document.tolerance(),
            subd_control_net,
            grip_owners,
        }))
    }

    pub fn unit_count(&self) -> usize {
        self.0.units.len()
    }

    pub fn is_current(&self, document: &Document) -> bool {
        self.0.tolerance == document.tolerance()
            && self
                .0
                .subd_control_net
                .is_none_or(|flat| flat == document.subd_control_net())
            && document
                .selected_object_ids()
                .eq(self.0.selection.iter().copied())
            && document
                .selected_control_points()
                .eq(self.0.grip_selection.iter().copied())
            && self.0.objects.iter().all(|(&id, original)| {
                document.object(id).is_some_and(|object| {
                    object
                        .geometry_snapshot()
                        .shares_storage_with(original.geometry_snapshot())
                        && object.attributes() == original.attributes()
                        && object.group_ids() == original.group_ids()
                })
            })
            && self
                .0
                .grip_modes
                .iter()
                .all(|(&id, &mode)| document.grip_mode(id) == Some(mode))
    }

    pub(crate) fn units(&self) -> &[Unit] {
        &self.0.units
    }

    pub(crate) fn individual_units(&self) -> Vec<Unit> {
        let mut units = self
            .0
            .selection
            .iter()
            .filter(|id| {
                self.0
                    .objects
                    .get(id)
                    .is_some_and(|o| o.geometry().bounds().is_some())
                    && !self.0.grip_owners.contains(id)
            })
            .map(|&id| Unit::Objects(vec![id]))
            .collect::<Vec<_>>();
        units.extend(self.0.units.iter().filter_map(|unit| match unit {
            Unit::Grip(id) => Some(Unit::Grip(*id)),
            _ => None,
        }));
        units
    }

    pub(crate) fn tolerance(&self) -> Tolerance {
        self.0.tolerance
    }

    pub(crate) fn includes_owner(&self, id: ObjectId) -> bool {
        self.0.objects.contains_key(&id)
    }

    pub(crate) fn origin(&self, unit: &Unit) -> Result<Point3, GeometryError> {
        match unit {
            Unit::Grip(id) => Ok(self.0.grips[id].point),
            Unit::Objects(ids) => ids
                .iter()
                .find_map(|id| self.0.objects[id].geometry().bounds())
                .ok_or(GeometryError::EmptyPointSet)?
                .center(),
        }
    }

    pub(crate) fn bounds(&self, unit: &Unit, frame: Frame3) -> Result<BoundingBox3, GeometryError> {
        match unit {
            Unit::Objects(ids) => crate::object_bounds::local_bounds(
                ids.iter().map(|id| self.0.objects[id].geometry()),
                frame,
                self.0.tolerance,
            ),
            Unit::Grip(id) => {
                let point = Point3::try_from(frame.coordinates_of(self.0.grips[id].point)?)?;
                BoundingBox3::from_points([point])
            }
        }
    }

    pub(crate) fn layout_placement<'a>(
        &self,
        movements: impl IntoIterator<Item = Result<(&'a Unit, Vector3), GeometryError>>,
    ) -> Result<Placement, CommandError> {
        let captured = &self.0;
        let mut translations = BTreeMap::new();
        let mut grip_targets: BTreeMap<ObjectId, (GripMode, BTreeMap<usize, Point3>)> =
            BTreeMap::new();
        let mut moved_grips = Vec::new();
        for movement in movements {
            let (unit, translation) = movement?;
            if translation.to_array() == [0.0; 3] {
                continue;
            }
            match unit {
                Unit::Objects(ids) => {
                    let map = AffineTransform3::from_translation(translation);
                    translations.extend(ids.iter().map(|&id| (id, map)));
                }
                Unit::Grip(id) => {
                    let grip = &captured.grips[id];
                    let (mode, targets) = grip_targets
                        .entry(id.object)
                        .or_insert_with(|| (grip.mode, BTreeMap::new()));
                    if *mode != grip.mode {
                        return Err(GeometryError::UnsupportedControlPointEdit {
                            context: "mixed grip bases on one layout source",
                        }
                        .into());
                    }
                    let target = grip.point.translated(translation)?;
                    if target != grip.point {
                        moved_grips.push(*id);
                    }
                    targets.insert(id.index, target);
                    targets.extend(grip.aliases.iter().map(|&index| (index, target)));
                }
            }
        }
        let edits = grip_targets
            .into_iter()
            .map(|(id, (mode, targets))| {
                let edited = captured.objects[&id].geometry().with_positioned_grips(
                    &targets,
                    captured.tolerance,
                    mode,
                )?;
                Ok((id, GeometrySnapshot::from(edited)))
            })
            .collect::<Result<BTreeMap<_, _>, CommandError>>()?;
        Ok(Placement {
            sources: self.clone(),
            translations,
            edits,
            moved_grips,
        })
    }
}

impl Placement {
    pub fn translations(&self) -> impl Iterator<Item = (ObjectId, AffineTransform3)> + '_ {
        self.translations.iter().map(|(&id, &map)| (id, map))
    }

    pub fn grip_edits(&self) -> impl Iterator<Item = (ObjectId, &GeometrySnapshot)> {
        self.edits.iter().map(|(&id, geometry)| (id, geometry))
    }

    pub(crate) fn apply(
        self,
        document: &mut Document,
        pick_policy: PickPolicy,
    ) -> Result<usize, CommandError> {
        let tolerance = self.sources.0.tolerance;
        let mut replacements = self
            .translations()
            .map(|(id, map)| {
                let geometry = self.sources.0.objects[&id]
                    .geometry()
                    .transformed(map, tolerance)?;
                Ok((id, geometry))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        replacements.extend(
            self.edits
                .into_iter()
                .map(|(id, geometry)| (id, (*geometry).clone())),
        );
        if matches!(pick_policy, PickPolicy::ReleaseMoved) && !replacements.is_empty() {
            let captured = &self.sources.0;
            let selected_grips = captured
                .grip_selection
                .iter()
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>();
            document
                .release_command_selection_on_history_replay(captured.selection.iter().copied())?;
            document.release_control_point_selection_on_history_replay(
                selected_grips.iter().copied(),
            )?;
            document.select_objects_direct(
                replacements
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| document.is_selected(*id))
                    .collect::<Vec<_>>(),
                SelectionMode::Remove,
            )?;
            document.select_control_points(
                self.moved_grips
                    .into_iter()
                    .filter(|id| selected_grips.contains(id)),
                SelectionMode::Remove,
            )?;
        }
        let changed =
            document.replace_object_geometries_preserving_control_selection(replacements)?;
        if matches!(pick_policy, PickPolicy::Clear) {
            document.clear_selection();
        }
        Ok(changed)
    }
}
