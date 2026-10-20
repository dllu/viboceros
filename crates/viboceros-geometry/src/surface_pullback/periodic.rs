//! Periodic chart events propose independent UV limits at one spatial seam.
//! Every accepted branch still goes through the complete surface-image proof.
use super::*;
#[cfg(test)]
mod tests;

pub(super) struct Cut {
    pub parameter: Real,
    pub points: [Point2; 2],
}

pub(super) struct Context<'a> {
    surface: &'a NurbsSurface,
    closed: [bool; 2],
    numerical: Tolerance,
    tolerance: Tolerance,
}
impl<'a> Context<'a> {
    pub fn new(
        surface: &'a NurbsSurface,
        closed: [bool; 2],
        numerical: Tolerance,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            surface,
            closed,
            numerical,
            tolerance,
        }
    }
    fn boundary(&self, axis: usize, p: Point2, high: bool) -> Result<Point2, GeometryError> {
        let domain = if axis == 0 {
            self.surface.domain_u()
        } else {
            self.surface.domain_v()
        };
        let value = if high { *domain.end() } else { *domain.start() };
        Point2::try_new(
            if axis == 0 { value } else { p.x() },
            if axis == 1 { value } else { p.y() },
        )
    }
    pub fn endpoint_branch(
        &self,
        point: Point2,
        interior: Point2,
        model: Point3,
    ) -> Result<Point2, GeometryError> {
        let mut result = point;
        for axis in 0..2 {
            if !self.closed[axis] {
                continue;
            }
            let high = coordinate(self.surface, axis, interior)? > 0.5;
            let proposal = self.boundary(axis, result, high)?;
            if self
                .surface
                .evaluate(proposal.x(), proposal.y())?
                .distance_to(model)?
                <= self.tolerance.absolute()
            {
                result = proposal;
            }
        }
        Ok(result)
    }
    pub fn near_seam(&self, point: Point2, model: Point3) -> Result<bool, GeometryError> {
        for axis in 0..2 {
            if !self.closed[axis] {
                continue;
            }
            let seam = self.boundary(axis, point, false)?;
            if self
                .surface
                .evaluate(seam.x(), seam.y())?
                .distance_to(model)?
                <= self.tolerance.absolute()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn cut(
        &self,
        interval: [Real; 2],
        endpoints: [Point2; 2],
        mut point: impl FnMut(Real) -> Result<Point3, GeometryError>,
    ) -> Result<Option<Cut>, GeometryError> {
        for axis in 0..2 {
            if !self.closed[axis] {
                continue;
            }
            let a = coordinate(self.surface, axis, endpoints[0])?;
            let b = coordinate(self.surface, axis, endpoints[1])?;
            if (a - b).abs() <= 0.5 {
                continue;
            }
            let descending = a > b;
            let [mut lo, mut hi] = interval;
            let mut best: Option<(Real, Cut)> = None;
            // This only proposes an event. A large endpoint difference might
            // cross the chart interior instead; its seam residual then fails.
            for iteration in 0..64 {
                let t = lo.mul_add(0.5, hi * 0.5);
                if t <= lo || t >= hi {
                    break;
                }
                let model = point(t)?;
                let q = pullback_parameters(self.surface, model, self.numerical)?;
                let f = coordinate(self.surface, axis, q)?;
                // Ordinary long UV spans can cross the chart interior. Let
                // their normal fit and certificate decide before subdividing.
                let margin = (a - b).abs() * 0.1;
                if iteration == 0 && f > a.min(b) + margin && f < a.max(b) - margin {
                    break;
                }
                let score = f.min(1. - f).abs();
                let mut points = [
                    self.boundary(axis, q, descending)?,
                    self.boundary(axis, q, !descending)?,
                ];
                // Both periodic coordinates can cross the same spatial
                // corner. Propose one shared event, with both independent
                // limits, instead of two nearly coincident native knot times.
                for other in 0..2 {
                    if other == axis || !self.closed[other] {
                        continue;
                    }
                    let first = coordinate(self.surface, other, endpoints[0])?;
                    let last = coordinate(self.surface, other, endpoints[1])?;
                    if (first - last).abs() <= 0.5 {
                        continue;
                    }
                    let together = [
                        self.boundary(other, points[0], first > last)?,
                        self.boundary(other, points[1], first <= last)?,
                    ];
                    if together.iter().all(|q| {
                        self.surface
                            .evaluate(q.x(), q.y())
                            .and_then(|p| p.distance_to(model))
                            .is_ok_and(|d| d <= self.tolerance.absolute() * 0.8)
                    }) {
                        points = together;
                    }
                }
                if best.as_ref().is_none_or(|(old, _)| score < *old)
                    && points.iter().all(|q| {
                        self.surface
                            .evaluate(q.x(), q.y())
                            .and_then(|p| p.distance_to(model))
                            .is_ok_and(|d| d <= self.tolerance.absolute() * 0.8)
                    })
                {
                    best = Some((
                        score,
                        Cut {
                            parameter: t,
                            points,
                        },
                    ));
                }
                if (f > 0.5) == descending {
                    lo = t;
                } else {
                    hi = t;
                }
            }
            if let Some((_, cut)) = best {
                return Ok(Some(cut));
            }
        }
        Ok(None)
    }
}

fn coordinate(surface: &NurbsSurface, axis: usize, p: Point2) -> Result<Real, GeometryError> {
    let domain = if axis == 0 {
        surface.domain_u()
    } else {
        surface.domain_v()
    };
    crate::remap_scalar(
        if axis == 0 { p.x() } else { p.y() },
        [*domain.start(), *domain.end()],
        [0., 1.],
    )
}

pub(super) fn changes_branch(
    surface: &NurbsSurface,
    a: Point2,
    b: Point2,
    closed: [bool; 2],
) -> Result<bool, GeometryError> {
    for (axis, closed) in closed.into_iter().enumerate() {
        if closed && (coordinate(surface, axis, a)? - coordinate(surface, axis, b)?).abs() > 0.5 {
            return Ok(true);
        }
    }
    Ok(false)
}
