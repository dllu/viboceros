use super::*;

/// Oriented, uniformly spaced samples of a source control face's limit chart.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDLimitFaceGrid {
    pub face: usize,
    /// None for quads; otherwise the original n-gon corner ordinal.
    pub corner: Option<usize>,
    /// Intervals on each axis. Points have row-major order, u varying fastest.
    pub side: usize,
    pub points: Vec<Point3>,
}

impl SubD {
    /// Exact bicubic NURBS patches for regular source faces. Each slot addresses
    /// the same ordinal as `faces()`; unsupported neighborhoods remain None.
    /// These patches preserve source face winding and use domain [0,1]^2.
    pub fn regular_nurbs_patches(&self) -> Result<Vec<Option<crate::NurbsSurface>>, GeometryError> {
        let mut surfaces = vec![None; self.faces().len()];
        for patch in viboceros_opennurbs::subd_regular_patches(self.archive())? {
            let slot = surfaces.get_mut(patch.face).ok_or_else(|| {
                viboceros_opennurbs::SubDError("Exact patch face outside source table".into())
            })?;
            let points = patch
                .controls
                .into_iter()
                .map(Point3::try_from)
                .collect::<Result<_, _>>()?;
            let knots = vec![0., 0., 0., 0., 1., 1., 1., 1.];
            *slot = Some(crate::NurbsSurface::try_new(
                3,
                3,
                4,
                4,
                points,
                knots.clone(),
                knots,
            )?);
        }
        Ok(surfaces)
    }

    /// Exact limit points at dyadic face coordinates, preserving source winding.
    /// Quads have `2^density` intervals per side; other polygons have one chart
    /// per corner with `2^(density-1)` intervals. Density must be 1..=6.
    /// Sampling has bounded work and does not change this SubD. Interpolating
    /// between these samples has no model-tolerance guarantee.
    pub fn limit_face_grids(&self, density: u32) -> Result<Vec<SubDLimitFaceGrid>, GeometryError> {
        viboceros_opennurbs::subd_face_grids(self.archive(), density)?
            .into_iter()
            .map(|grid| {
                Ok(SubDLimitFaceGrid {
                    face: grid.face,
                    corner: grid.corner,
                    side: grid.side,
                    points: grid
                        .points
                        .into_iter()
                        .map(Point3::try_from)
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect()
    }
}
