//! One layout feeds the viewport preview and the accepted transaction.
use super::*;
use crate::component_selection::{ComponentPick, ComponentSelectionKind};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use viboceros_document::{
    ControlPointId, Geometry, GeometrySnapshot, GripMode, Object, ObjectId, SelectionMode,
};
use viboceros_geometry::{AffineTransform3, BoundingBox3};

#[derive(Clone, Debug)]
enum Unit {
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
    tolerance: Tolerance,
    subd_control_net: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Sources(Arc<Captured>);

#[derive(Clone, Debug)]
pub struct Layout {
    sources: Sources,
    translations: BTreeMap<ObjectId, AffineTransform3>,
    edits: BTreeMap<ObjectId, GeometrySnapshot>,
    moved_grips: Vec<ControlPointId>,
    spacing: f64,
    mode: DistributionMode,
}

impl Sources {
    /// A displayed grip takes precedence over a component alias on the same
    /// owner. Grip owners take precedence over simultaneously selected parents.
    pub fn capture(document: &Document, components: &[ComponentPick]) -> Self {
        let grip_selection = document.selected_control_points().collect::<Vec<_>>();
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
            if let Some(mode) = document.grip_mode(id.object) {
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
                    let mode = document.grip_mode(pick.object).unwrap_or_else(|| {
                        let control_net =
                            *subd_control_net.get_or_insert(document.subd_control_net());
                        if control_net {
                            GripMode::Control
                        } else {
                            GripMode::Edit
                        }
                    });
                    let points = document
                        .control_point_locations(pick.object)
                        .unwrap_or_else(|| match mode {
                            GripMode::Control => subd.control_points(),
                            GripMode::Edit => subd.surface_points(),
                        });
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
        for id in grip_owners {
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
            tolerance: document.tolerance(),
            subd_control_net,
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
            && self.0.grip_selection.iter().all(|(id, _)| {
                document
                    .grip_mode(id.object)
                    .is_some_and(|mode| mode == self.0.grips[id].mode)
            })
    }

    pub fn layout(
        &self,
        direction: Direction,
        plane: Frame3,
        settings: DistributionSettings,
    ) -> Result<Layout, CommandError> {
        let captured = &self.0;
        if captured.units.len() < 3 {
            return Err(CommandError::InsufficientDistributionObjects {
                actual: captured.units.len(),
            });
        }
        let anchor = match &captured.units[0] {
            Unit::Grip(id) => captured.grips[id].point,
            Unit::Objects(ids) => ids
                .iter()
                .find_map(|id| captured.objects[id].geometry().bounds())
                .ok_or(GeometryError::EmptyPointSet)?
                .center()?,
        };
        let frame = direction_frame(direction, plane, anchor, captured.tolerance)?;
        let bounds = captured
            .units
            .iter()
            .map(|unit| match unit {
                Unit::Objects(ids) => crate::object_bounds::local_bounds(
                    ids.iter().map(|id| captured.objects[id].geometry()),
                    frame,
                    captured.tolerance,
                ),
                Unit::Grip(id) => {
                    let point = Point3::try_from(frame.coordinates_of(captured.grips[id].point)?)?;
                    BoundingBox3::from_points([point])
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let mut order = (0..captured.units.len()).collect::<Vec<_>>();
        order.sort_by(|a, b| {
            bounds[*a]
                .min()
                .to_array()
                .partial_cmp(&bounds[*b].min().to_array())
                .expect("finite bounds")
        });
        let intervals = order
            .iter()
            .map(|&i| [bounds[i].min().x(), bounds[i].max().x()])
            .collect::<Vec<_>>();
        let (offsets, spacing) = planner::offsets(&intervals, settings.mode, settings.spacing)?;
        let mut translations = BTreeMap::new();
        let mut grip_targets: BTreeMap<ObjectId, (GripMode, BTreeMap<usize, Point3>)> =
            BTreeMap::new();
        let mut moved_grips = Vec::new();
        for (index, distance) in order.into_iter().zip(offsets) {
            if distance == 0.0 {
                continue;
            }
            let translation = frame.x_axis().as_vector().scaled(distance)?;
            match &captured.units[index] {
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
                            context: "mixed grip bases on one distribution source",
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
        Ok(Layout {
            sources: self.clone(),
            translations,
            edits,
            moved_grips,
            spacing,
            mode: settings.mode,
        })
    }
}

impl Layout {
    pub fn translations(&self) -> impl Iterator<Item = (ObjectId, AffineTransform3)> + '_ {
        self.translations.iter().map(|(&id, &map)| (id, map))
    }

    pub fn grip_edits(&self) -> impl Iterator<Item = (ObjectId, &GeometrySnapshot)> {
        self.edits.iter().map(|(&id, geometry)| (id, geometry))
    }

    pub fn commit(
        self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        crate::run_command_transaction(document, "Distribute", |doc| self.apply(doc, postselected))
    }

    pub(super) fn apply(
        self,
        document: &mut Document,
        postselected: bool,
    ) -> Result<String, CommandError> {
        if !self.sources.is_current(document) {
            return Err(CommandError::DistributeStale);
        }
        let tolerance = self.sources.0.tolerance;
        let count = self.sources.unit_count();
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
        if postselected && !replacements.is_empty() {
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
        Ok(format!(
            "Distributed {count} object/group or grip unit(s) with {:?} spacing {:.6}; moved {changed} object(s)",
            self.mode, self.spacing
        ))
    }
}
