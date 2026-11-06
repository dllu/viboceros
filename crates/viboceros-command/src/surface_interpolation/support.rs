//! Captured finite faces and infinite planes share interpolation ownership.
use super::*;
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Debug)]
pub enum Support {
    Surface(Target),
    Plane {
        plane: Plane,
        frame: Frame3,
        domain: [[Real; 2]; 2],
        parent: Option<(ObjectId, GeometrySnapshot)>,
        tolerance: Tolerance,
    },
}
impl From<Target> for Support {
    fn from(target: Target) -> Self {
        Self::Surface(target)
    }
}
impl Support {
    pub fn capture_plane(
        doc: &Document,
        definition: &str,
        cplane: Frame3,
    ) -> Result<Self, CommandError> {
        let reference = crate::infinite_plane::resolve(doc, definition, cplane)?;
        let frame = crate::infinite_plane::axis_frame(definition, cplane)
            .map(Ok)
            .unwrap_or_else(|| {
                Frame3::try_from_unit_normal(reference.plane.origin(), reference.plane.normal())
            })?;
        let domain = pseudo_plane_domain(doc, frame)?;
        Ok(Self::Plane {
            plane: reference.plane,
            frame,
            domain,
            parent: reference
                .object
                .map(|id| (id, doc.object(id).unwrap().geometry_snapshot().clone())),
            tolerance: doc.tolerance(),
        })
    }
    pub fn capture_frame(doc: &Document, frame: Frame3) -> Result<Self, CommandError> {
        Ok(Self::Plane {
            plane: Plane::new(frame.origin(), frame.z_axis()),
            frame,
            domain: pseudo_plane_domain(doc, frame)?,
            parent: None,
            tolerance: doc.tolerance(),
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        match self {
            Self::Surface(target) => target.current(doc),
            Self::Plane {
                parent, tolerance, ..
            } => {
                *tolerance == doc.tolerance()
                    && parent.as_ref().is_none_or(|(id, old)| {
                        doc.is_object_selectable(*id)
                            && doc
                                .object(*id)
                                .is_some_and(|o| old.shares_storage_with(o.geometry_snapshot()))
                    })
            }
        }
    }
    pub fn point(&self, point: Point3) -> Result<Point3, CommandError> {
        match self {
            Self::Surface(target) => Ok(target.location(point, false)?.point),
            Self::Plane { frame, domain, .. } => {
                let [u, v] = frame.projected_coordinates_of(point)?;
                Ok(frame.point_at([
                    u.clamp(domain[0][0], domain[0][1]),
                    v.clamp(domain[1][0], domain[1][1]),
                    0.,
                ])?)
            }
        }
    }
    pub fn cursor_surface(&self) -> Option<(ObjectId, Option<usize>, bool)> {
        match self {
            Self::Surface(target) => Some((target.id, Some(target.face), false)),
            Self::Plane { .. } => None,
        }
    }
    pub fn frame(&self) -> Option<Frame3> {
        match self {
            Self::Plane { frame, .. } => Some(*frame),
            _ => None,
        }
    }
    pub(super) fn interpolate(
        &self,
        doc: &Document,
        points: &[Point3],
        closed: bool,
    ) -> Result<SurfaceInterpolatedCurve, CommandError> {
        match self {
            Self::Surface(target) => {
                let uv = points
                    .iter()
                    .map(|p| {
                        let l = target.location(*p, false)?;
                        Ok(Point2::try_from(l.uv)?)
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?;
                Ok(viboceros_geometry::try_interpolate_curve_on_surface(
                    &target.surface,
                    &uv,
                    closed,
                    doc.tolerance(),
                )?)
            }
            Self::Plane { plane, .. } => {
                let points = points
                    .iter()
                    .map(|p| self.point(*p))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(viboceros_geometry::try_interpolate_curve_on_plane(
                    *plane,
                    &points,
                    closed,
                    doc.tolerance(),
                )?)
            }
        }
    }
}

// OpenNURBS ON_PlaneSurface::CreatePseudoInfinitePlane projects box corners
// and grows each interval by padding*(length+1). The observed Rhino 8.32
// surface getter uses padding=0.01, including an origin box for an empty model.
fn pseudo_plane_domain(doc: &Document, frame: Frame3) -> Result<[[Real; 2]; 2], CommandError> {
    let (lo, hi) = doc
        .bounds()
        .map(|b| (b.min().to_array(), b.max().to_array()))
        .unwrap_or(([0.; 3], [0.; 3]));
    let mut domain = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for index in 0..8 {
        let p = Point3::try_from(std::array::from_fn::<_, 3, _>(|axis| {
            if index & (1 << axis) == 0 {
                lo[axis]
            } else {
                hi[axis]
            }
        }))?;
        for (axis, value) in frame.projected_coordinates_of(p)?.into_iter().enumerate() {
            domain[axis][0] = domain[axis][0].min(value);
            domain[axis][1] = domain[axis][1].max(value);
        }
    }
    for d in &mut domain {
        let padding = 0.01 * (d[1] - d[0]) + 0.01;
        d[0] -= padding;
        d[1] += padding;
        if !d[0].is_finite() || !d[1].is_finite() || d[0] >= d[1] {
            return Err(CommandError::Usage(
                "InfinitePlane bounds are not representable",
            ));
        }
    }
    Ok(domain)
}
