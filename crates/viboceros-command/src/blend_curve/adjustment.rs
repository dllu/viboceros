//! Contact and shape edits preserve accepted state until the new preview succeeds.
use super::*;
impl Input {
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
        if !self.control_points().iter().any(|(h, _)| *h == handle) {
            return Err(CommandError::Usage(USAGE));
        }
        self.active = Some(handle);
        Ok(())
    }
    pub fn finish_adjustment(&mut self) {
        self.active = None;
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
        if let Some(c) = self.shape_constraint() {
            let length = c
                .anchor
                .vector_to(point)?
                .dot(c.direction.unwrap().as_vector())?;
            let i = if self.active == Some(Handle::StartShape) {
                1
            } else {
                2
            };
            return self.option(&format!("Handle{i}={length}")).map(|_| ());
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
