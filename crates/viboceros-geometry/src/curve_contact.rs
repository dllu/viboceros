//! Cached native curve domains for iterative contact solvers.
use crate::{CurveRef, GeometryError, Real, UnitVector3, Vector3};
#[derive(Clone, Copy)]
pub(super) struct Domain {
    pub a: Real,
    pub b: Real,
    pub closed: bool,
}
impl Domain {
    pub fn new(c: CurveRef<'_>) -> Result<Self, GeometryError> {
        Ok(Self {
            a: *c.domain().start(),
            b: *c.domain().end(),
            closed: c.is_closed()?,
        })
    }
    pub fn bound(self, t: Real) -> Real {
        if self.closed {
            self.a + (t - self.a).rem_euclid(self.b - self.a)
        } else {
            t.clamp(self.a, self.b)
        }
    }
}

pub(super) fn project(v: Vector3, normal: Option<UnitVector3>) -> Result<Vector3, GeometryError> {
    let Some(n) = normal else {
        return Ok(v);
    };
    let d = v.dot(n.as_vector())?;
    Vector3::try_from(std::array::from_fn(|i| {
        v.to_array()[i] - d * n.as_vector().to_array()[i]
    }))
}
