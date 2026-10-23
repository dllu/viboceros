//! Public OpenNURBS quad triangulation and logical polygon remapping.
use super::*;

impl TriangleMesh {
    /// Split quads using OpenNURBS' shortest spatial diagonal and tie policy.
    /// First triangles retain source slots; second triangles append. Vertex
    /// storage/channels and n-gon boundaries survive; native face lists remap.
    /// Small valid features are independent of document tolerance.
    pub fn triangulate_quads(&self, _tolerance: Tolerance) -> Result<(Self, usize), GeometryError> {
        if !self.faces.iter().any(|face| face.is_quad()) {
            return Ok((self.clone(), 0));
        }
        let (data, converted) = viboceros_opennurbs::triangulate_mesh(&self.open_nurbs_data())?;
        Ok((self.decode_native_face_conversion(data)?, converted))
    }
}
