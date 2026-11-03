//! Unpacked base patches and extraordinary-vertex continuity processing.
use super::*;
use crate::NurbsSurface;
mod continuity;
mod g1x;
mod g1xx;
mod stencil;
mod topology;

/// Extraordinary-vertex processing, using Rhino's named continuity modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubDNurbsContinuity {
    G0,
    G1,
    G1x,
    G1xx,
    G2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubDNurbsPatch {
    pub face: usize,
    pub corner: Option<usize>,
    pub surface: NurbsSurface,
}

impl SubD {
    /// Unpacked NURBS patches with local extraordinary-vertex processing.
    pub fn to_nurbs_patches(
        &self,
        mode: SubDNurbsContinuity,
    ) -> Result<Vec<SubDNurbsPatch>, GeometryError> {
        let mut patches = self.to_nurbs_patches_g0()?;
        if mode == SubDNurbsContinuity::G1xx {
            g1xx::apply(self, &mut patches)?;
        } else {
            continuity::apply(self, &mut patches, mode)?;
        }
        Ok(patches)
    }

    pub fn to_nurbs_brep(
        &self,
        mode: SubDNurbsContinuity,
        tolerance: crate::Tolerance,
    ) -> Result<crate::Brep, GeometryError> {
        topology::assemble(self, self.to_nurbs_patches(mode)?, tolerance)
    }

    /// Unpacked G0 B-rep with shared topology inherited from source component
    /// identities. N-gon sectors split their shared control edges at midpoints;
    /// adjacent quad faces retain one surface with the corresponding trim split.
    pub fn to_nurbs_brep_g0(
        &self,
        tolerance: crate::Tolerance,
    ) -> Result<crate::Brep, GeometryError> {
        topology::assemble(self, self.to_nurbs_patches_g0()?, tolerance)
    }

    /// Unpacked G0 cubic approximation in normalized source-face charts.
    /// Regular faces retain exact bicubic patches. Other quads use eight spans
    /// per axis; n-gon corner sectors use four. Corners interpolate exact limit
    /// points. This is a finite SubD approximation, without a continuous error
    /// certificate. Soft sharpness is not supported by this base constructor.
    pub fn to_nurbs_patches_g0(&self) -> Result<Vec<SubDNurbsPatch>, GeometryError> {
        if self.edge_sharpness().iter().flatten().any(|&v| v != 0.) {
            return Err(viboceros_opennurbs::SubDError(
                "Soft-sharp SubD NURBS patches are not implemented".into(),
            )
            .into());
        }
        let mut exact = self.regular_nurbs_patches()?;
        if exact.iter().all(Option::is_some) {
            return Ok(exact
                .into_iter()
                .enumerate()
                .map(|(face, surface)| SubDNurbsPatch {
                    face,
                    corner: None,
                    surface: surface.unwrap(),
                })
                .collect());
        }
        let stencils = viboceros_opennurbs::subd_patch_stencils(self.archive(), 3)?;
        stencils
            .into_iter()
            .map(|net| {
                let surface = if net.corner.is_none() {
                    exact
                        .get_mut(net.face)
                        .ok_or_else(|| {
                            viboceros_opennurbs::SubDError("Patch face outside source table".into())
                        })?
                        .take()
                        .map(Ok)
                        .unwrap_or_else(|| stencil::surface(&net))?
                } else {
                    stencil::surface(&net)?
                };
                Ok(SubDNurbsPatch {
                    face: net.face,
                    corner: net.corner,
                    surface,
                })
            })
            .collect()
    }
}
