//! Contact and shape edits preserve accepted state until the new preview succeeds.
use super::*;
impl Input {
    /// Associate a preview control with an implemented edit. Alt rotation also
    /// applies to higher controls belonging to the same supporting face.
    /// Independent secondary-control shape edits require additional jet freedoms.
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
        if index == 1 && self.continuity[0] != 0 {
            return Some(Handle::StartShape);
        }
        if index + 1 == last && self.continuity[1] != 0 {
            return Some(Handle::EndShape);
        }
        if angle {
            for (i, handle) in [Handle::StartShape, Handle::EndShape]
                .into_iter()
                .enumerate()
            {
                let belongs = if i == 0 {
                    index <= self.continuity[0] as usize
                } else {
                    index >= last - self.continuity[1] as usize
                };
                if belongs
                    && self.endpoints[i]
                        .curve()
                        .is_some_and(|g| g.surface.is_some())
                {
                    return Some(handle);
                }
            }
        }
        None
    }
    pub fn control_points(&self) -> Vec<(Handle, Point3)> {
        let Some(c) = &self.preview else {
            return vec![];
        };
        let p = c.control_points();
        let mut out = vec![
            (Handle::Start, p[0].point()),
            (Handle::End, p[p.len() - 1].point()),
        ];
        if self.continuity[0] != 0 {
            out.push((Handle::StartShape, p[1].point()));
        }
        if self.continuity[1] != 0 {
            out.push((Handle::EndShape, p[p.len() - 2].point()));
        }
        out
    }
    pub fn select_handle(&mut self, handle: Handle) -> Result<(), CommandError> {
        self.select_handle_with_symmetry(handle, false)
    }
    /// Shift selection pairs shape handles when both continuities match.
    /// Contact edits and mismatched endpoint constraints remain independent.
    pub fn select_handle_with_symmetry(
        &mut self,
        handle: Handle,
        symmetric: bool,
    ) -> Result<(), CommandError> {
        self.select_handle_with_modifiers(handle, symmetric, false)
    }
    /// Alt selects a supporting-face angle edit for this adjustment. Ordinary
    /// curve handles and contacts retain their usual constraints.
    pub fn select_handle_with_modifiers(
        &mut self,
        handle: Handle,
        symmetric: bool,
        angle: bool,
    ) -> Result<(), CommandError> {
        if !self.control_points().iter().any(|(h, _)| *h == handle) {
            return Err(CommandError::Usage(USAGE));
        }
        self.active = Some(handle);
        self.symmetric = symmetric;
        let index = match handle {
            Handle::StartShape => Some(0),
            Handle::EndShape => Some(1),
            _ => None,
        };
        self.angle_editing = angle
            && index.is_some_and(|i| {
                self.endpoints[i]
                    .curve()
                    .is_some_and(|g| g.surface.is_some())
            });
        Ok(())
    }
    pub fn paired_handle(&self) -> Option<Handle> {
        if self.angle_editing
            || !self.symmetric
            || self.continuity[0] == 0
            || self.continuity[0] != self.continuity[1]
        {
            return None;
        }
        match self.active? {
            Handle::StartShape => Some(Handle::EndShape),
            Handle::EndShape => Some(Handle::StartShape),
            _ => None,
        }
    }
    pub fn finish_adjustment(&mut self) {
        self.active = None;
        self.symmetric = false;
        self.angle_editing = false;
    }
    /// The fixed tangent plane used by an Alt-selected surface shape handle.
    pub fn angle_constraint(&self) -> Option<viboceros_geometry::Plane> {
        if !self.angle_editing {
            return None;
        }
        let i = match self.active? {
            Handle::StartShape => 0,
            Handle::EndShape => 1,
            _ => return None,
        };
        let guide = self.endpoints[i].curve()?;
        guide.surface.as_ref()?.plane(guide).ok()
    }
    pub fn adjustment_curve(&self) -> Option<&NurbsCurve> {
        let i = match self.active? {
            Handle::Start => 0,
            Handle::End => 1,
            _ => return None,
        };
        Some(self.endpoints[i].curve()?.display())
    }
    pub fn shape_constraint(&self) -> Option<crate::translation::DestinationConstraint> {
        let i = match self.active? {
            Handle::StartShape => 0,
            Handle::EndShape => 1,
            _ => return None,
        };
        let guide = self.endpoints[i].curve()?;
        let mut direction = guide.direction(i).ok()?;
        if i == 1 {
            direction = direction.opposite();
        }
        Some(crate::translation::DestinationConstraint {
            anchor: guide.point().ok()?,
            direction: Some(direction),
            distance: None,
        })
    }
    pub fn adjust_constraint_parameter(&mut self, t: Real) -> Result<(), CommandError> {
        let i = match self.active {
            Some(Handle::Start) => 0,
            Some(Handle::End) => 1,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let parameter = self.endpoints[i]
            .curve()
            .ok_or(CommandError::Usage(
                "A point endpoint has no curve parameter",
            ))?
            .curve_ref()
            .parameter_from_nurbs(t)?;
        self.option(&format!("Parameter{}={parameter}", i + 1))
            .map(|_| ())
    }
    pub fn adjust_point(&mut self, point: Point3) -> Result<(), CommandError> {
        if self.angle_editing {
            let i = if self.active == Some(Handle::StartShape) {
                0
            } else {
                1
            };
            let guide = self.endpoints[i]
                .curve()
                .ok_or(CommandError::Usage(USAGE))?;
            let angle = guide
                .surface
                .as_ref()
                .ok_or(CommandError::Usage(USAGE))?
                .angle_to_point(guide, point)?;
            return self.adjust_shape_angle(angle);
        }
        if let Some(c) = self.shape_constraint() {
            let length = c
                .anchor
                .vector_to(point)?
                .dot(c.direction.unwrap().as_vector())?;
            return self.adjust_shape_length(length);
        }
        let i = match self.active {
            Some(Handle::Start) => 0,
            Some(Handle::End) => 1,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let Some(guide) = self.endpoints[i].curve() else {
            let mut next = self.clone();
            next.freeze_handles();
            next.endpoints[i] = Endpoint::Point(point);
            next.rebuild()?;
            *self = next;
            return Ok(());
        };
        let t = guide.curve_ref().closest_parameter(point, self.tolerance)?;
        self.option(&format!("Parameter{}={t}", i + 1)).map(|_| ())
    }
    pub fn adjust_shape_angle(&mut self, angle: Real) -> Result<(), CommandError> {
        if !self.angle_editing {
            return Err(CommandError::Usage(
                "Select a surface shape handle with Alt",
            ));
        }
        let i = if self.active == Some(Handle::StartShape) {
            1
        } else {
            2
        };
        self.option(&format!("AtAngle_{i}={angle}")).map(|_| ())
    }
    pub(super) fn freeze_handles(&mut self) {
        let Some(c) = &self.preview else { return };
        let p = c.control_points();
        for (i, (a, b)) in [
            (p[0].point(), p[1].point()),
            (p[p.len() - 1].point(), p[p.len() - 2].point()),
        ]
        .into_iter()
        .enumerate()
        {
            if self.continuity[i] != 0 && self.handles[i].is_none() {
                self.handles[i] = a.distance_to(b).ok();
            }
        }
    }
}
