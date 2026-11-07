//! Contact and shape edits preserve accepted state until the new preview succeeds.
use super::*;
impl Input {
    pub fn flip_source(&mut self, id: ObjectId) -> Result<(), CommandError> {
        let point = self
            .endpoints
            .iter()
            .filter_map(Endpoint::curve)
            .find(|g| g.source.object_id() == id)
            .ok_or(CommandError::Usage("Select one of the blend sources"))?
            .point()?;
        self.flip_source_at(id, point)
    }
    /// Repeated references to one source are distinguished by their captured
    /// contacts, rather than the shared document object identity.
    pub fn flip_source_at(&mut self, id: ObjectId, point: Point3) -> Result<(), CommandError> {
        let mut selected = None;
        let mut nearest = Real::INFINITY;
        for (i, g) in self
            .endpoints
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.curve().map(|g| (i, g)))
        {
            if g.source.object_id() == id {
                let distance = g.point()?.distance_to(point)?;
                if distance < nearest {
                    nearest = distance;
                    selected = Some(i);
                }
            }
        }
        let mut next = self.clone();
        let g = next.endpoints
            [selected.ok_or(CommandError::Usage("Select one of the blend sources"))?]
        .curve_mut()
        .unwrap();
        if g.surface.is_some() {
            return Err(CommandError::Usage(
                "Surface-edge direction is controlled by AtAngle",
            ));
        }
        g.flip = !g.flip;
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn flip_both(&mut self) -> Result<(), CommandError> {
        let mut next = self.clone();
        for g in next.endpoints.iter_mut().filter_map(Endpoint::curve_mut) {
            if g.surface.is_none() {
                g.flip = !g.flip;
            }
        }
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    pub fn finish_adjustment(&mut self) {
        self.active = None;
        self.symmetric = false;
        self.angle_editing = false;
        self.shape_origin = None;
        self.shape_distance = None;
    }
    /// The fixed tangent plane used by an Alt-selected surface shape handle.
    pub fn angle_constraint(&self) -> Option<viboceros_geometry::Plane> {
        if !self.angle_editing {
            return None;
        }
        let (i, _) = self.active?.shape()?;
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
            let (i, _) = self
                .active
                .and_then(Handle::shape)
                .ok_or(CommandError::Usage(USAGE))?;
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
        let i = self
            .active
            .and_then(Handle::shape)
            .ok_or(CommandError::Usage(USAGE))?
            .0
            + 1;
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
