//! Transient control-point display and picking, shared by UI and commands.
use super::*;
use std::sync::Arc;
use viboceros_geometry::Point3;
mod deletion;
mod geometry;
mod weight;
#[cfg(test)]
mod weight_tests;
pub use deletion::GripDeletionReport;
#[cfg(test)]
mod morph_tests;
mod transform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ControlPointId {
    pub object: ObjectId,
    pub index: usize,
}

/// The displayed locations and the meaning of a grip index belong together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GripMode {
    Control,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DisplayOverride {
    None,
    ResetOnHistory,
    RetainOnHistory,
}

#[derive(Clone, Debug)]
pub(super) struct ControlPoints {
    mode: GripMode,
    geometry: GeometrySnapshot,
    points: Arc<[Point3]>,
    selected: Arc<BTreeSet<usize>>,
    hidden: Arc<BTreeSet<usize>>,
    display_override: DisplayOverride,
}

impl ControlPoints {
    pub(super) fn deselect(&mut self, index: usize) {
        Arc::make_mut(&mut self.selected).remove(&index);
    }
    pub(super) fn reset_visibility(&mut self) {
        self.hidden = Arc::default();
    }
    pub(super) fn reveal_hidden_selection(&mut self) {
        if self.selected.iter().any(|i| self.hidden.contains(i)) {
            self.reset_visibility();
        }
    }
    /// Point visibility is a live display setting, independent of geometry Undo.
    /// Restoring a selected hidden grip makes this owner's entire display visible.
    pub(super) fn retain_live_visibility(&mut self, current: &Self) {
        if self.mode == current.mode && self.points.len() == current.points.len() {
            if current.display_override == DisplayOverride::RetainOnHistory
                && self.geometry.shares_storage_with(&current.geometry)
            {
                self.points = current.points.clone();
                self.display_override = DisplayOverride::RetainOnHistory;
            }
            self.hidden = if self.selected.iter().any(|i| current.hidden.contains(i)) {
                Arc::default()
            } else {
                current.hidden.clone()
            };
        }
    }

    pub(super) fn retain_selection_if_count_unchanged(&mut self, original: &Self) {
        self.hidden = if self.points.len() == original.points.len() {
            original.hidden.clone()
        } else {
            Arc::default()
        };
        self.selected = if self.points.len() == original.points.len() {
            original.selected.clone()
        } else {
            Arc::default()
        };
    }
}

impl Geometry {
    pub fn supports_control_points(&self) -> bool {
        !matches!(
            self,
            Self::Point(_) | Self::PointCloud(_) | Self::BlockInstance(_)
        ) && !matches!(self, Self::Brep(brep) if brep.faces().len() != 1)
    }

    pub fn supports_edit_points(&self) -> bool {
        self.curve_ref().is_some() || matches!(self, Self::SubD(_))
    }

    /// Source grip coordinates in the selected control/edit basis, independent of display overrides.
    pub fn grip_locations(&self, mode: GripMode) -> Result<Vec<Point3>, GeometryError> {
        if mode == GripMode::Edit {
            if let Self::SubD(subd) = self {
                return Ok(subd.surface_points().to_vec());
            }
            if let Self::PolyCurve(curve) = self {
                return curve.edit_point_locations();
            }
            let curve = self
                .curve_ref()
                .ok_or(GeometryError::UnsupportedControlPointEdit {
                    context: "edit points require a curve",
                })?
                .to_nurbs()?;
            return Ok(curve
                .edit_points()?
                .into_iter()
                .take(curve.grip_count()?)
                .map(|p| p.point)
                .collect());
        }
        match self {
            Self::NurbsCurve(curve) => Ok(curve
                .control_points()
                .iter()
                .take(curve.grip_count()?)
                .map(|p| p.point())
                .collect()),
            Self::Brep(brep) if brep.faces().len() == 1 => {
                Ok(brep.faces()[0].surface().extract_point_locations())
            }
            _ => self.extract_point_locations(),
        }
    }
}

impl Document {
    pub(super) fn commit_object_geometries_with_control_selection(
        &mut self,
        staged: Vec<(usize, Geometry)>,
        history: ReplacementHistory,
        label: &'static str,
    ) -> Result<usize, DocumentError> {
        let before = staged
            .iter()
            .filter_map(|(index, _)| {
                let id = self.objects[*index].id;
                self.control_points
                    .get(&id)
                    .map(|state| (id, state.clone()))
            })
            .collect::<Vec<_>>();
        let owns_transaction = !before.is_empty() && self.history.active.is_none();
        if owns_transaction {
            self.begin_transaction(label)?;
        }
        let result = self.commit_object_geometries(staged, label, label, history, true);
        if result.is_ok() {
            for (id, old) in before {
                let state = self.control_points.get_mut(&id).unwrap();
                if state.geometry.shares_storage_with(&old.geometry) {
                    continue;
                }
                state.selected = Arc::new(
                    old.selected
                        .iter()
                        .copied()
                        .filter(|&index| index < state.points.len())
                        .collect(),
                );
                self.record_edit(
                    label,
                    Edit::ControlPointsChanged {
                        id,
                        stored: Some(old),
                    },
                );
            }
        }
        if owns_transaction {
            if result.is_ok() {
                self.commit_transaction()?;
            } else {
                self.rollback_transaction()?;
            }
        }
        result
    }

    /// Enables grips without changing geometry, Undo labels, or Redo. All IDs
    /// and generated locations are validated before display or selection changes.
    pub fn enable_control_points(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        self.enable_grips(ids, GripMode::Control)
    }

    /// Display curve Greville edit points without replacing the source geometry.
    pub fn enable_edit_points(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<usize, DocumentError> {
        self.enable_grips(ids, GripMode::Edit)
    }

    fn enable_grips(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: GripMode,
    ) -> Result<usize, DocumentError> {
        let indices = self.resolve_object_indices(ids)?;
        let mut staged = Vec::new();
        for index in indices {
            let object = &self.objects[index];
            if !self.object_is_selectable(object) {
                return Err(DocumentError::ObjectNotSelectable(object.id));
            }
            if match mode {
                GripMode::Control => object.geometry.supports_control_points(),
                GripMode::Edit => object.geometry.supports_edit_points(),
            } {
                staged.push((
                    object.id,
                    ControlPoints {
                        mode,
                        geometry: object.geometry.clone(),
                        points: object.geometry.grip_locations(mode)?.into(),
                        selected: Arc::default(),
                        hidden: Arc::default(),
                        display_override: DisplayOverride::None,
                    },
                ));
            }
        }
        let count = staged.len();
        for (id, state) in staged {
            self.control_points
                .entry(id)
                .and_modify(|current| {
                    if current.mode != mode
                        || !current.geometry.shares_storage_with(&state.geometry)
                    {
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

    /// Mode of a visible, selectable grip display matching its owner snapshot.
    pub fn grip_mode(&self, id: ObjectId) -> Option<GripMode> {
        self.control_point_locations(id)?;
        Some(self.control_points[&id].mode)
    }

    /// Active, selectable grip displays in object order, including displays
    /// whose individual grips are all hidden. Resolves each owner once.
    pub fn control_point_displays(&self) -> impl Iterator<Item = (ObjectId, GripMode)> + '_ {
        self.objects.iter().filter_map(|object| {
            let state = self.control_points.get(&object.id)?;
            (self.object_is_selectable(object)
                && state.geometry.shares_storage_with(&object.geometry))
            .then_some((object.id, state.mode))
        })
    }

    /// Change displayed grip positions without replacing owner geometry or
    /// recording history. Subsequent geometry edits use the owner's source layout;
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
            state.display_override = DisplayOverride::ResetOnHistory;
        }
        Ok(count)
    }

    /// Native nonlinear Copy leaves source geometry unchanged while its live
    /// grip display follows the accepted deformation. Each placement measures
    /// the owner's original control/edit basis; repeated copies do not compound
    /// the previous display. This override survives Undo/Redo of copies while
    /// that source geometry remains unchanged.
    pub fn morph_copy_grip_display(
        &mut self,
        grips: impl IntoIterator<Item = ControlPointId>,
        morph: &(impl viboceros_geometry::PointMorph + ?Sized),
    ) -> Result<usize, DocumentError> {
        let mut layouts = BTreeMap::new();
        let mut staged = BTreeMap::new();
        for grip in grips {
            self.control_point_locations(grip.object)
                .filter(|p| grip.index < p.len())
                .ok_or(DocumentError::InvalidControlPointSelection {
                    object: grip.object,
                    index: grip.index,
                })?;
            self.ensure_object_editable(self.object(grip.object).unwrap())?;
            if let std::collections::btree_map::Entry::Vacant(entry) = layouts.entry(grip.object) {
                let state = &self.control_points[&grip.object];
                entry.insert(state.geometry.grip_locations(state.mode)?);
            }
            staged.insert(grip, morph.morph_point(layouts[&grip.object][grip.index])?);
        }
        let count = staged.len();
        for (grip, point) in staged {
            let state = self.control_points.get_mut(&grip.object).unwrap();
            Arc::make_mut(&mut state.points)[grip.index] = point;
            state.display_override = DisplayOverride::RetainOnHistory;
        }
        Ok(count)
    }

    /// Enabled grips on selectable owners, including hidden grips. Indices and
    /// locations keep their original basis when other grips are hidden.
    pub fn all_control_points(&self) -> impl Iterator<Item = (ControlPointId, Point3, bool)> + '_ {
        self.iter_control_points(true)
    }
    /// Visible, pickable grips in stable owner/index order.
    pub fn control_points(&self) -> impl Iterator<Item = (ControlPointId, Point3, bool)> + '_ {
        self.iter_control_points(false)
    }
    pub fn is_control_point_hidden(&self, grip: ControlPointId) -> Option<bool> {
        self.control_point_locations(grip.object)
            .filter(|p| grip.index < p.len())?;
        Some(
            self.control_points[&grip.object]
                .hidden
                .contains(&grip.index),
        )
    }
    /// Borrow hidden indices once for a complete owner query. This avoids
    /// repeating owner validation for every point in a viewport window.
    pub fn hidden_control_point_indices(
        &self,
        id: ObjectId,
    ) -> Option<impl Iterator<Item = usize> + '_> {
        self.control_point_locations(id)?;
        Some(self.control_points[&id].hidden.iter().copied())
    }
    pub fn is_control_point_selectable(&self, grip: ControlPointId) -> bool {
        self.is_control_point_hidden(grip) == Some(false)
    }
    /// Validate the complete identity batch before changing display or picks.
    /// Visibility is transient and never records a model edit or clears Redo.
    pub fn set_control_points_hidden(
        &mut self,
        grips: impl IntoIterator<Item = ControlPointId>,
        hidden: bool,
    ) -> Result<usize, DocumentError> {
        let grips = grips.into_iter().collect::<BTreeSet<_>>();
        for grip in &grips {
            if self.is_control_point_hidden(*grip).is_none() {
                return Err(DocumentError::InvalidControlPointSelection {
                    object: grip.object,
                    index: grip.index,
                });
            }
        }
        let mut changed = 0;
        for grip in grips {
            let state = self.control_points.get_mut(&grip.object).unwrap();
            if hidden {
                changed += usize::from(Arc::make_mut(&mut state.hidden).insert(grip.index));
                Arc::make_mut(&mut state.selected).remove(&grip.index);
            } else {
                changed += usize::from(Arc::make_mut(&mut state.hidden).remove(&grip.index));
            }
        }
        Ok(changed)
    }
    pub fn hide_selected_control_points(&mut self) -> usize {
        let grips = self
            .selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>();
        self.set_control_points_hidden(grips, true)
            .expect("selected grips have valid current identities")
    }
    pub fn show_control_points(&mut self) -> usize {
        let grips = self
            .all_control_points()
            .map(|(g, _, _)| g)
            .filter(|g| self.is_control_point_hidden(*g) == Some(true))
            .collect::<Vec<_>>();
        self.set_control_points_hidden(grips, false)
            .expect("hidden grips have valid current identities")
    }
    fn iter_control_points(
        &self,
        include_hidden: bool,
    ) -> impl Iterator<Item = (ControlPointId, Point3, bool)> + '_ {
        self.objects
            .iter()
            .take(if self.control_points.is_empty() {
                0
            } else {
                self.objects.len()
            })
            .flat_map(move |object| {
                let state = self.control_points.get(&object.id).filter(|state| {
                    self.object_is_selectable(object)
                        && state.geometry.shares_storage_with(&object.geometry)
                });
                state.into_iter().flat_map(move |state| {
                    state
                        .points
                        .iter()
                        .enumerate()
                        .filter(move |(index, _)| include_hidden || !state.hidden.contains(index))
                        .map(move |(index, point)| {
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

    /// Selected visible grips accepted by point and geometry getters. Locations
    /// are Euclidean; rational weights do not weight a grip's Circle fit input.
    pub fn selected_control_points(&self) -> impl Iterator<Item = (ControlPointId, Point3)> + '_ {
        self.selected_logical_control_points()
            .filter(|(g, _)| !self.control_points[&g.object].hidden.contains(&g.index))
    }
    /// Logical selections also include hidden grips selected by U/V navigation.
    pub fn selected_logical_control_points(
        &self,
    ) -> impl Iterator<Item = (ControlPointId, Point3)> + '_ {
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
    /// Select logical identities, including hidden navigation targets. Physical
    /// mouse/getter inputs must first check `is_control_point_selectable`.
    /// Returns the total logical grip selection count.
    pub fn select_control_points(
        &mut self,
        picks: impl IntoIterator<Item = ControlPointId>,
        mode: SelectionMode,
    ) -> Result<usize, DocumentError> {
        let picks = picks.into_iter().collect::<BTreeSet<_>>();
        for pick in &picks {
            if self.is_control_point_hidden(*pick).is_none() {
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
        Ok(self.selected_logical_control_points().count())
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
                if let Ok(points) = object.geometry.grip_locations(state.mode) {
                    if state.points.len() != points.len() {
                        state.hidden = Arc::default();
                    }
                    state.points = points.into();
                    state.geometry = object.geometry.clone();
                    state.display_override = DisplayOverride::None;
                }
                state.selected = Arc::default();
            }
        }
    }

    pub(super) fn reset_control_point_display_overrides(&mut self) {
        for state in self
            .control_points
            .values_mut()
            .filter(|state| state.display_override == DisplayOverride::ResetOnHistory)
        {
            state.points = state
                .geometry
                .grip_locations(state.mode)
                .expect("enabled grip geometry is valid")
                .into();
            state.display_override = DisplayOverride::None;
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
mod edit_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod visibility_tests;
