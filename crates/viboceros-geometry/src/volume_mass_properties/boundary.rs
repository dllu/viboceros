//! Shared-reference integration of separately represented volume boundaries.
use super::*;
use crate::{BoundingBox3, Brep, NurbsSurface, Tolerance, TriangleMesh};

/// An oriented boundary piece, not a claim that this piece encloses a solid.
#[derive(Clone, Copy)]
pub enum VolumeBoundary<'a> {
    Mesh(&'a TriangleMesh),
    Brep(&'a Brep),
    Surface(&'a NurbsSurface),
}

impl VolumeBoundary<'_> {
    /// Closure for warning/selection policy; a closed mesh may have inconsistent
    /// winding. This is intentionally distinct from oriented-solid validation.
    pub fn is_closed(self, tolerance: Tolerance) -> Result<bool, GeometryError> {
        Ok(match self {
            Self::Mesh(m) => m.topology().is_closed(),
            Self::Brep(b) => b.is_solid(),
            Self::Surface(s) => Brep::try_surface_face(s.clone(), tolerance)?.is_solid(),
        })
    }

    /// Bounds of participating geometry. Unused mesh vertices never choose the
    /// volume reference point or enlarge the numeric integration frame.
    pub fn bounds(self) -> Result<BoundingBox3, GeometryError> {
        match self {
            Self::Mesh(m) => BoundingBox3::from_points(
                m.faces()
                    .iter()
                    .flat_map(|f| f.indices())
                    .map(|i| m.vertices()[*i as usize]),
            ),
            Self::Brep(b) => Ok(b.bounds()),
            Self::Surface(s) => Ok(s.control_point_bounds()),
        }
    }

    /// Signed cone flux and first moments about an explicit common base point.
    /// Only a consistently oriented, enclosing collection has reference-
    /// independent enclosed-volume meaning. Open-piece results depend on base.
    pub fn volume_flux(
        self,
        base: Point3,
        tolerance: Tolerance,
    ) -> Result<VolumeMassProperties, GeometryError> {
        match self {
            Self::Mesh(m) => m.volume_flux(base),
            Self::Brep(b) => b.volume_flux(base, tolerance),
            Self::Surface(s) => s.volume_flux(base, tolerance),
        }
    }
}

impl VolumeMassProperties {
    /// Integrates a nonempty collection in one shared reference frame. The
    /// center of the union of participating bounds is the default base. This
    /// does not certify that unjoined pieces enclose a volume: callers must
    /// explicitly warn or validate before presenting it as a solid measurement.
    pub fn from_boundaries(
        boundaries: &[VolumeBoundary<'_>],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::boundary_integrals::<true>(boundaries, tolerance)
    }

    /// Signed cone volume using the same reference convention as
    /// `from_boundaries`, without computing first moments. Mesh contributions
    /// and cross-object cancellation remain exact until the final conversion.
    pub fn signed_volume_from_boundaries(
        boundaries: &[VolumeBoundary<'_>],
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        Self::boundary_integrals::<false>(boundaries, tolerance)?.signed_volume()
    }

    fn boundary_integrals<const FIRST: bool>(
        boundaries: &[VolumeBoundary<'_>],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut bounds = None;
        for boundary in boundaries {
            let next = boundary.bounds()?;
            bounds = Some(match bounds {
                Some(previous) => BoundingBox3::union(previous, next)?,
                None => next,
            });
        }
        let base = bounds.ok_or(GeometryError::EmptyPointSet)?.center()?;
        let mut total = Self::default();
        for boundary in boundaries {
            // Closed oriented boundaries are reference independent. Retain
            // their efficient exact origin-based mesh path and well-conditioned
            // individual surface frames, particularly for widely spaced solids.
            let mass = match *boundary {
                VolumeBoundary::Mesh(m) => {
                    let reference = if m.topology().is_solid() {
                        Point3::try_new(0., 0., 0.)?
                    } else {
                        base
                    };
                    m.volume_integrals::<FIRST>(reference)?
                }
                VolumeBoundary::Brep(b) => {
                    let reference = if b.is_solid() {
                        b.bounds().center()?
                    } else {
                        base
                    };
                    b.volume_integrals::<FIRST>(reference, tolerance)?
                }
                VolumeBoundary::Surface(s) => {
                    let b = Brep::try_surface_face(s.clone(), tolerance)?;
                    let reference = if b.is_solid() {
                        b.bounds().center()?
                    } else {
                        base
                    };
                    b.volume_integrals::<FIRST>(reference, tolerance)?
                }
            };
            total.add(&mass);
        }
        Ok(total)
    }
}
