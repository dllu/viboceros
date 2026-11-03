use super::*;
impl SubD {
    /// Regular bicubic limit-patch jets from the public OpenNURBS kernel.
    /// Extraordinary corners without a regular sector remain unavailable.
    pub fn curvature_mesh(&self) -> Result<crate::CurvatureMesh, GeometryError> {
        self.curvature_mesh_with_density(SubDMeshDensity::default())
    }

    pub fn curvature_mesh_with_density(
        &self,
        density: SubDMeshDensity,
    ) -> Result<crate::CurvatureMesh, GeometryError> {
        let mut data =
            viboceros_opennurbs::subd_tessellate_with_jets(self.archive(), density.for_subd(self))?;
        let indices = std::mem::take(&mut data.corner_jet_indices);
        let jets = std::mem::take(&mut data.surface_jets)
            .into_iter()
            .map(|[p, u, v, uu, uv, vv]| {
                Ok(crate::SurfaceJet2 {
                    point: Point3::try_from(p)?,
                    derivative_u: Vector3::try_from(u)?,
                    derivative_v: Vector3::try_from(v)?,
                    derivative_uu: Vector3::try_from(uu)?,
                    derivative_uv: Vector3::try_from(uv)?,
                    derivative_vv: Vector3::try_from(vv)?,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let display = Self::tessellation(data)?;
        crate::CurvatureMesh::from_subd(display.mesh, display.face_corner_normals, jets, indices)
    }
}
