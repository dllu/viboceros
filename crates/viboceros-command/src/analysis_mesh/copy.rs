//! Preserve face topology while separating distinct normal/color sectors.
use super::*;
use std::collections::HashMap;
use viboceros_geometry::MeshFace;

pub fn copy_mesh(
    source: &CurvatureMesh,
    colors: Option<&[[[u8; 3]; 3]]>,
    tolerance: Tolerance,
) -> Result<TriangleMesh, GeometryError> {
    if colors.is_some_and(|c| c.len() != source.mesh.triangles().len()) {
        return Err(GeometryError::Degenerate {
            context: "analysis color channels",
        });
    }
    let normals = normals(source).collect::<Vec<_>>();
    let mut vertices = Vec::new();
    let mut output_normals = Vec::new();
    let mut output_colors = Vec::new();
    let mut sectors = HashMap::new();
    let mut index = |triangle: usize, corner: usize| -> Result<u32, GeometryError> {
        let original = source.mesh.triangles()[triangle][corner];
        let normal = normals[triangle][corner];
        let color = colors.map(|c| c[triangle][corner]);
        let key = (
            original,
            normal
                .to_array()
                .map(|v| if v == 0. { 0 } else { v.to_bits() }),
            color,
        );
        if let Some(i) = sectors.get(&key) {
            return Ok(*i);
        }
        let i = u32::try_from(vertices.len()).map_err(|_| GeometryError::Degenerate {
            context: "analysis mesh vertex count",
        })?;
        sectors.insert(key, i);
        vertices.push(source.mesh.vertices()[original as usize]);
        output_normals.push(normal);
        if let Some([r, g, b]) = color {
            output_colors.push([r, g, b, 0]);
        }
        Ok(i)
    };
    let mut faces = Vec::new();
    let mut triangle = 0;
    for face in source.mesh.faces() {
        let a = [
            index(triangle, 0)?,
            index(triangle, 1)?,
            index(triangle, 2)?,
        ];
        if face.indices().len() == 3 {
            faces.push(MeshFace::Triangle(a));
            triangle += 1;
        } else {
            let b = [
                index(triangle + 1, 0)?,
                index(triangle + 1, 1)?,
                index(triangle + 1, 2)?,
            ];
            if a[0] == b[0] && a[2] == b[1] {
                faces.push(MeshFace::Quad([a[0], a[1], a[2], b[2]]));
            } else {
                faces.extend([MeshFace::Triangle(a), MeshFace::Triangle(b)]);
            }
            triangle += 2;
        }
    }
    TriangleMesh::try_new_faces(vertices, faces, tolerance)?
        .try_with_vertex_normals(Some(output_normals))?
        .try_with_vertex_colors(colors.map(|_| output_colors))
}
