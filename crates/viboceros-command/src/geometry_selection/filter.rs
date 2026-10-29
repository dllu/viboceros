//! Typed predicates shared by noninteractive geometry selectors.
use super::*;

#[derive(Clone, Copy)]
pub(crate) enum GeometrySelectionFilter {
    Curve,
    OpenCurve,
    ClosedCurve,
    PlanarCurve,
    Line,
    Polyline,
    PointCloud,
    Extrusion,
    BlockInstance,
    MirroredBlock,
    Surface,
    OpenSurface,
    ClosedSurface,
    PlanarSurface,
    TrimmedSurface,
    UntrimmedSurface,
    Polysurface,
    OpenPolysurface,
    ClosedPolysurface,
    Mesh,
    OpenMesh,
    ClosedMesh,
    NonManifold,
    SubD,
    OpenSubD,
    ClosedSubD,
}

impl GeometrySelectionFilter {
    pub(super) fn matches(
        self,
        geometry: &Geometry,
        tolerance: Tolerance,
    ) -> Result<bool, GeometryError> {
        let matches = match self {
            Self::Curve => geometry_curve_ref(geometry).is_some(),
            Self::OpenCurve => match geometry_curve_ref(geometry) {
                Some(curve) => !curve.is_closed()?,
                None => false,
            },
            Self::ClosedCurve => match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_closed()?,
                None => false,
            },
            Self::PlanarCurve => match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_planar(tolerance)?,
                None => false,
            },
            Self::Line => match geometry {
                Geometry::Line(_) => true,
                Geometry::NurbsCurve(curve) => {
                    curve.spans().count() == 1 && curve.is_linear_at_zero_tolerance()?
                }
                _ => false,
            },
            Self::Polyline => match geometry {
                Geometry::Polyline(_) => true,
                Geometry::NurbsCurve(curve) => {
                    curve.degree() == 1 && curve.control_points().len() > 2
                }
                _ => false,
            },
            Self::PointCloud => matches!(geometry, Geometry::PointCloud(_)),
            Self::Extrusion => matches!(geometry, Geometry::Brep(brep) if brep.is_extrusion()),
            Self::BlockInstance => matches!(geometry, Geometry::BlockInstance(_)),
            Self::MirroredBlock => match geometry {
                Geometry::BlockInstance(instance) => {
                    instance.reference().transform().orientation_reversing()?
                }
                _ => false,
            },
            Self::Surface => match geometry {
                Geometry::NurbsSurface(_) => true,
                Geometry::Brep(brep) => brep.faces().len() == 1,
                _ => false,
            },
            Self::OpenSurface | Self::ClosedSurface => {
                let closed = match geometry {
                    Geometry::NurbsSurface(surface) => {
                        surface.natural_boundary_curve_loops()?.is_empty()
                    }
                    Geometry::Brep(brep) if brep.faces().len() == 1 => brep.is_closed(),
                    _ => return Ok(false),
                };
                closed == matches!(self, Self::ClosedSurface)
            }
            Self::PlanarSurface => match geometry {
                Geometry::NurbsSurface(surface) => surface.plane(tolerance)?.is_some(),
                Geometry::Brep(brep) if brep.faces().len() == 1 => {
                    brep.faces()[0].surface().plane(tolerance)?.is_some()
                }
                _ => false,
            },
            Self::TrimmedSurface | Self::UntrimmedSurface => {
                let untrimmed = match geometry {
                    Geometry::NurbsSurface(_) => true,
                    Geometry::Brep(brep) if brep.faces().len() == 1 => {
                        brep.faces()[0].is_untrimmed(tolerance)?
                    }
                    _ => return Ok(false),
                };
                untrimmed == matches!(self, Self::UntrimmedSurface)
            }
            Self::Polysurface => match geometry {
                Geometry::Brep(brep) => brep.faces().len() > 1,
                _ => false,
            },
            Self::OpenPolysurface => match geometry {
                Geometry::Brep(brep) => brep.faces().len() > 1 && !brep.is_closed(),
                _ => false,
            },
            Self::ClosedPolysurface => match geometry {
                Geometry::Brep(brep) => brep.faces().len() > 1 && brep.is_closed(),
                _ => false,
            },
            Self::SubD => matches!(geometry, Geometry::SubD(_)),
            Self::OpenSubD | Self::ClosedSubD => match geometry {
                Geometry::SubD(subd) => subd.has_boundary() == matches!(self, Self::OpenSubD),
                _ => false,
            },
            Self::Mesh => matches!(geometry, Geometry::Mesh(_) | Geometry::EmptyMesh),
            Self::OpenMesh => match geometry {
                Geometry::Mesh(mesh) => !mesh.topology().is_closed(),
                _ => false,
            },
            Self::ClosedMesh => match geometry {
                Geometry::Mesh(mesh) => mesh.topology().is_closed(),
                _ => false,
            },
            Self::NonManifold => match geometry {
                Geometry::Mesh(mesh) => !mesh.topology().is_manifold(),
                Geometry::Brep(brep) => !brep.is_manifold(),
                _ => false,
            },
        };
        Ok(matches)
    }
}
