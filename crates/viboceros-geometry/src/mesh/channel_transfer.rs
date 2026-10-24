//! Face-owned interpolation retains different channel values at coincident vertices.
use super::*;
use crate::point::PointDistance;

#[cfg(test)]
mod tests;
mod weights;

/// Prepared source triangles for channel transfer to mesh refinements.
/// Preparation reuses OpenNURBS' quad diagonal and face ordering. Each output
/// face must identify an original source face; global nearest-point matching
/// would confuse overlapping faces and texture seams.
pub struct MeshChannelSource<'a> {
    source: &'a TriangleMesh,
    triangles: Vec<([u32; 3], Option<[u32; 3]>)>,
}
impl<'a> MeshChannelSource<'a> {
    pub fn new(source: &'a TriangleMesh) -> Result<Self, GeometryError> {
        let (triangulated, _) = source.triangulate_quads(Tolerance::MESH_VALIDATION)?;
        let expected = source.faces.len() + source.faces.iter().filter(|f| f.is_quad()).count();
        if triangulated.faces.len() != expected || triangulated.faces.iter().any(|f| f.is_quad()) {
            return Err(GeometryError::Degenerate {
                context: "mesh channel source triangulation",
            });
        }
        let mut appended = triangulated.faces[source.faces.len()..].iter();
        let triangles = source
            .faces
            .iter()
            .enumerate()
            .map(|(i, face)| {
                let first = triangulated.faces[i]
                    .indices()
                    .try_into()
                    .expect("triangulated face");
                let second = face.is_quad().then(|| {
                    appended
                        .next()
                        .expect("quad's second triangle")
                        .indices()
                        .try_into()
                        .expect("triangulated face")
                });
                (first, second)
            })
            .collect();
        Ok(Self { source, triangles })
    }

    /// Transfer UVs, stored normals, and RGBA colors through explicit face ownership.
    /// Original vertices retain their stored values. New normals are normalized
    /// linear blends, UVs retain native float storage, and color bytes are rounded.
    /// A vertex splits when incident source faces supply different stored values.
    /// Native raw vertex order and triangulation are not reproduced.
    pub fn transfer(
        &self,
        mut part: TriangleMesh,
        parents: &[usize],
    ) -> Result<TriangleMesh, GeometryError> {
        let invalid = || GeometryError::Degenerate {
            context: "mesh channel source faces",
        };
        if parents.len() != part.faces.len() || parents.iter().any(|&p| p >= self.triangles.len()) {
            return Err(invalid());
        }
        let source = self.source;
        if source.texture_coordinates().is_none()
            && source.vertex_normals().is_none()
            && source.vertex_colors().is_none()
        {
            part.vertex_channels = VertexChannels::default();
            part.vertex_colors = None;
            return Ok(part);
        }
        let mut channels = ChannelBuilder::new(&source.vertex_channels);
        let mut colors = source.vertex_colors.as_ref().map(|_| Vec::new());
        let mut vertices = Vec::new();
        let mut faces = Vec::with_capacity(part.faces.len());
        let mut corners = BTreeMap::new();
        let mut values = BTreeMap::new();
        for (&face, &parent) in part.faces.iter().zip(parents) {
            let mut mapped = [0_u32; 4];
            for (i, &v) in face.indices().iter().enumerate() {
                let output = if let Some(&output) = corners.get(&(v, parent)) {
                    output
                } else {
                    let blends = self.weights(part.vertices[v as usize], parent)?;
                    let color = source.vertex_colors.as_ref().map(|colors| {
                        std::array::from_fn(|c| {
                            blends
                                .iter()
                                .map(|&(i, w)| f64::from(colors[i][c]) * w)
                                .sum::<f64>()
                                .round()
                                .clamp(0., 255.) as u8
                        })
                    });
                    if let [(index, weight)] = blends.as_slice()
                        && *weight == 1.
                    {
                        channels.push(*index);
                    } else {
                        channels.blend(&blends)?;
                    }
                    let key = (v, channels.last_key(), color);
                    let output = if let Some(&output) = values.get(&key) {
                        channels.pop();
                        output
                    } else {
                        let output = u32::try_from(vertices.len())
                            .map_err(|_| GeometryError::TooManyMeshVertices)?;
                        vertices.push(part.vertices[v as usize]);
                        if let Some(colors) = &mut colors {
                            colors.push(color.unwrap());
                        }
                        values.insert(key, output);
                        output
                    };
                    corners.insert((v, parent), output);
                    output
                };
                mapped[i] = output;
            }
            faces.push(match face {
                MeshFace::Triangle(_) => MeshFace::Triangle([mapped[0], mapped[1], mapped[2]]),
                MeshFace::Quad(_) => MeshFace::Quad(mapped),
            });
        }
        let mut result = TriangleMesh::from_validated_parts(vertices, faces)
            .retain_valid_ngons_for_same_faces(&part.ngons);
        result.vertex_channels = channels.finish();
        result.vertex_colors = colors;
        Ok(result)
    }

    fn weights(&self, point: Point3, parent: usize) -> Result<Vec<(usize, Real)>, GeometryError> {
        let (first, second) = self.triangles[parent];
        // Exact source corners must retain even nonunit or zero stored normals.
        if let Some(&v) = self.source.faces[parent]
            .indices()
            .iter()
            .find(|&&v| self.source.vertices[v as usize] == point)
        {
            return Ok(vec![(v as usize, 1.)]);
        }
        let closest = |t: [u32; 3]| -> Result<Point3, GeometryError> {
            let [a, b, c] = t.map(|i| self.source.vertices[i as usize]);
            closest_point_on_triangle(point, a, b, c)
        };
        let mut triangle = first;
        let mut nearest = closest(first)?;
        if let Some(second) = second {
            let candidate = closest(second)?;
            if PointDistance::new(candidate, point)
                .compare(&PointDistance::new(nearest, point), point)
                .is_lt()
            {
                triangle = second;
                nearest = candidate;
            }
        }
        let weights =
            weights::triangle(nearest, triangle.map(|i| self.source.vertices[i as usize]))?;
        Ok(triangle
            .into_iter()
            .zip(weights)
            .filter_map(|(i, w)| (w > 0.).then_some((i as usize, w)))
            .collect())
    }
}
