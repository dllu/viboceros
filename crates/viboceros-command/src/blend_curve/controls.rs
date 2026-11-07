//! Control identity and modifiers are captured once at selection.
use super::*;
impl Handle {
    pub(super) fn shape(self) -> Option<(usize, u8)> {
        match self {
            Self::StartShape => Some((0, 1)),
            Self::EndShape => Some((1, 1)),
            Self::StartControl(k) => Some((0, k)),
            Self::EndControl(k) => Some((1, k)),
            _ => None,
        }
    }
    fn from_shape(side: usize, order: u8) -> Self {
        match (side, order) {
            (0, 1) => Self::StartShape,
            (1, 1) => Self::EndShape,
            (0, k) => Self::StartControl(k),
            (_, k) => Self::EndControl(k),
        }
    }
}
impl Input {
    /// Alt on a higher surface control rotates its endpoint's primary direction.
    pub fn handle_for_control_point(&self, index: usize, angle: bool) -> Option<Handle> {
        let last = self
            .preview
            .as_ref()?
            .control_points()
            .len()
            .checked_sub(1)?;
        if index > last {
            return None;
        }
        if index == 0 {
            return Some(Handle::Start);
        }
        if index == last {
            return Some(Handle::End);
        }
        let (side, k) = if index <= usize::from(self.continuity[0]) {
            (0, index)
        } else if last - index <= usize::from(self.continuity[1]) {
            (1, last - index)
        } else {
            return None;
        };
        let rotate = angle
            && self.endpoints[side]
                .curve()
                .is_some_and(|g| g.surface.is_some());
        Some(Handle::from_shape(side, if rotate { 1 } else { k as u8 }))
    }
    pub fn control_points(&self) -> Vec<(Handle, Point3)> {
        let Some(c) = &self.preview else {
            return vec![];
        };
        c.control_points()
            .iter()
            .enumerate()
            .filter_map(|(k, p)| {
                self.handle_for_control_point(k, false)
                    .map(|h| (h, p.point()))
            })
            .collect()
    }
    pub fn select_handle(&mut self, handle: Handle) -> Result<(), CommandError> {
        self.select_handle_with_symmetry(handle, false)
    }
    pub fn select_handle_with_symmetry(
        &mut self,
        handle: Handle,
        symmetric: bool,
    ) -> Result<(), CommandError> {
        self.select_handle_with_modifiers(handle, symmetric, false)
    }
    pub fn select_handle_with_modifiers(
        &mut self,
        handle: Handle,
        symmetric: bool,
        angle: bool,
    ) -> Result<(), CommandError> {
        let point = self
            .control_points()
            .into_iter()
            .find(|(h, _)| *h == handle)
            .map(|(_, p)| p)
            .ok_or(CommandError::Usage(USAGE))?;
        self.active = Some(handle);
        self.symmetric = symmetric;
        self.angle_editing = angle
            && handle.shape().is_some_and(|(i, _)| {
                self.endpoints[i]
                    .curve()
                    .is_some_and(|g| g.surface.is_some())
            });
        self.shape_origin = Some(point);
        self.shape_distance = None;
        Ok(())
    }
    pub fn paired_handle(&self) -> Option<Handle> {
        if self.angle_editing || !self.symmetric || self.continuity[0] != self.continuity[1] {
            return None;
        }
        let (i, k) = self.active?.shape()?;
        Some(Handle::from_shape(1 - i, k))
    }
}
