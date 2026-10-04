//! Transient control-point display and picking, shared by UI and commands.
use super::*;
use std::sync::Arc;
use viboceros_geometry::Point3;
mod geometry;
mod transform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ControlPointId {
    pub object: ObjectId,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub(super) struct ControlPoints {
    geometry: GeometrySnapshot,
    points: Arc<[Point3]>,
    selected: Arc<BTreeSet<usize>>,
    display_override: bool,
}

impl Geometry {
    pub fn supports_control_points(&self) -> bool {
        !matches!(self, Self::Point(_) | Self::PointCloud(_))
            && !matches!(self, Self::Brep(brep) if brep.faces().len() != 1)
    }

    fn grip_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        match self {
            Self::Brep(brep) if brep.faces().len() == 1 => {
                Ok(brep.faces()[0].surface().extract_point_locations())
            }
            _ => self.extract_point_locations(),
        }
    }
}

impl Document {
    /// Enables grips without changing geometry, Undo labels, or Redo. All IDs
    /// and generated locations are validated before display or selection changes.
    pub fn enable_control_points(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        let indices = self.resolve_object_indices(ids)?;
        let mut staged = Vec::new();
        for index in indices {
            let object = &self.objects[index];
            if !self.object_is_selectable(object) {
                return Err(DocumentError::ObjectNotSelectable(object.id));
            }
            if object.geometry.supports_control_points() {
                staged.push((
                    object.id,
                    ControlPoints {
                        geometry: object.geometry.clone(),
                        points: object.geometry.grip_locations()?.into(),
                        selected: Arc::default(),
                        display_override: false,
                    },
                ));
            }
        }
        let count = staged.len();
        for (id, state) in staged {
            self.control_points
                .entry(id)
                .and_modify(|current| {
                    if !current.geometry.shares_storage_with(&state.geometry) {
                        *current = state.clone();
                    }
                })
                .or_insert(state);
            self.selection.remove(&id);
            self.selection_order.retain(|old| *old != id);
        }
        Ok(count)
    }

    pub fn disable_control_points(&mut self) -> usize {
        let active = self
            .control_points
            .keys()
            .copied()
            .filter(|&id| self.object(id).is_some())
            .collect::<Vec<_>>();
        let count = active.len();
        for id in active {
            self.control_points.remove(&id);
        }
        count
    }

    pub fn control_point_locations(&self, id: ObjectId) -> Option<&[Point3]> {
        let state = self.control_points.get(&id)?;
        let object = self.object(id)?;
        (self.object_is_selectable(object) && state.geometry.shares_storage_with(&object.geometry))
            .then_some(state.points.as_ref())
    }

    /// Change displayed grip positions without replacing owner geometry or
    /// recording history. Subsequent geometry edits use the owner's controls;
    /// history replay restores their display. All picks and maps are staged.
    pub fn transform_control_point_display(
        &mut self,
        grips: impl IntoIterator<Item = ControlPointId>,
        transform: AffineTransform3,
    ) -> Result<usize, DocumentError> {
        let mut staged = BTreeMap::new();
        for grip in grips {
            let points = self
                .control_point_locations(grip.object)
                .filter(|p| grip.index < p.len())
                .ok_or(DocumentError::InvalidControlPointSelection {
                    object: grip.object,
                    index: grip.index,
                })?;
            self.ensure_object_editable(self.object(grip.object).unwrap())?;
            staged.insert(grip, transform.transform_point(points[grip.index])?);
        }
        let count = staged.len();
        for (grip, point) in staged {
            let state = self.control_points.get_mut(&grip.object).unwrap();
            Arc::make_mut(&mut state.points)[grip.index] = point;
            state.display_override = true;
        }
        Ok(count)
    }

    /// Returns exact source identities and Euclidean locations; rational
    /// weights do not weight a grip's contribution to a Circle fit.
    pub fn control_points(&self) -> impl Iterator<Item = (ControlPointId, Point3, bool)> + '_ {
        self.objects
            .iter()
            .take(if self.control_points.is_empty() {
                0
            } else {
                self.objects.len()
            })
            .flat_map(|object| {
                let state = self.control_points.get(&object.id).filter(|state| {
                    self.object_is_selectable(object)
                        && state.geometry.shares_storage_with(&object.geometry)
                });
                state.into_iter().flat_map(move |state| {
                    state.points.iter().enumerate().map(move |(index, point)| {
                        (
                            ControlPointId {
                                object: object.id,
                                index,
                            },
                            *point,
                            state.selected.contains(&index),
                        )
                    })
                })
            })
    }

    pub fn selected_control_points(&self) -> impl Iterator<Item = (ControlPointId, Point3)> + '_ {
        self.objects
            .iter()
            .take(if self.control_points.is_empty() {
                0
            } else {
                self.objects.len()
            })
            .flat_map(|object| {
                let state = self.control_points.get(&object.id).filter(|state| {
                    self.object_is_selectable(object)
                        && state.geometry.shares_storage_with(&object.geometry)
                });
                state.into_iter().flat_map(move |state| {
                    state.selected.iter().map(move |&index| {
                        (
                            ControlPointId {
                                object: object.id,
                                index,
                            },
                            state.points[index],
                        )
                    })
                })
            })
    }

    pub fn clear_control_point_selection(&mut self) -> usize {
        let count = self
            .control_points
            .values()
            .map(|state| state.selected.len())
            .sum();
        for state in self.control_points.values_mut() {
            if !state.selected.is_empty() {
                state.selected = Arc::default();
            }
        }
        count
    }

    /// A complete batch is checked before changing either grip or owner picks.
    pub fn select_control_points(
        &mut self,
        picks: impl IntoIterator<Item = ControlPointId>,
        mode: SelectionMode,
    ) -> Result<usize, DocumentError> {
        let picks = picks.into_iter().collect::<BTreeSet<_>>();
        for pick in &picks {
            if self
                .control_point_locations(pick.object)
                .is_none_or(|points| pick.index >= points.len())
            {
                return Err(DocumentError::InvalidControlPointSelection {
                    object: pick.object,
                    index: pick.index,
                });
            }
        }
        if mode == SelectionMode::Replace {
            self.clear_selection();
        }
        for pick in picks {
            let selected =
                Arc::make_mut(&mut self.control_points.get_mut(&pick.object).unwrap().selected);
            match mode {
                SelectionMode::Replace | SelectionMode::Add => {
                    selected.insert(pick.index);
                }
                SelectionMode::Remove => {
                    selected.remove(&pick.index);
                }
                SelectionMode::Toggle => {
                    if !selected.insert(pick.index) {
                        selected.remove(&pick.index);
                    }
                }
            }
        }
        Ok(self.selected_control_points().count())
    }

    pub fn select_all_control_points(&mut self) -> usize {
        if self.control_points.is_empty() {
            return 0;
        }
        let ids = self
            .control_points()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>();
        self.select_control_points(ids, SelectionMode::Add)
            .expect("current grips are validated")
    }

    pub(super) fn synchronize_control_points(&mut self) {
        if self.control_points.is_empty() {
            return;
        }
        for object in &self.objects {
            if let Some(state) = self.control_points.get_mut(&object.id)
                && !state.geometry.shares_storage_with(&object.geometry)
            {
                if let Ok(points) = object.geometry.grip_locations() {
                    state.points = points.into();
                    state.geometry = object.geometry.clone();
                    state.display_override = false;
                }
                state.selected = Arc::default();
            }
        }
    }

    pub(super) fn reset_control_point_display_overrides(&mut self) {
        for state in self
            .control_points
            .values_mut()
            .filter(|state| state.display_override)
        {
            state.points = state
                .geometry
                .grip_locations()
                .expect("enabled grip geometry is valid")
                .into();
            state.display_override = false;
        }
    }

    pub(super) fn prune_control_point_selection(&mut self) {
        if self.control_points.is_empty() {
            return;
        }
        for object in &self.objects {
            if !self.object_is_selectable(object)
                && let Some(state) = self.control_points.get_mut(&object.id)
            {
                state.selected = Arc::default();
            }
        }
    }
}

#[cfg(test)]
mod tests;
