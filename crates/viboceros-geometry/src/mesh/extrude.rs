//! In-place extrusion of mesh faces and naked topology edges.
use super::*;
use crate::Vector3;

#[derive(Clone, Debug, PartialEq)]
pub enum MeshExtrudeSelection {
    AllFaces,
    Faces(Vec<usize>),
    BoundaryEdges(Vec<usize>),
}

#[derive(Clone, Copy, Debug)]
pub enum MeshExtrudeDirection {
    World(UnitVector3),
    VertexNormals,
    EdgeOutward,
}

impl TriangleMesh {
    /// Replace selected faces with displaced caps and boundary walls, or extend
    /// selected naked edges with quads. Indices use the original face/topology
    /// edge tables. Existing faces and surviving n-gons keep their winding.
    pub fn extrude_mesh(
        &self,
        selection: &MeshExtrudeSelection,
        distance: Real,
        direction: MeshExtrudeDirection,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "mesh extrusion distance",
            });
        }
        let topology = self.topology_data();
        let edge_table = topology.edges.values().collect::<Vec<_>>();
        let mut selected = vec![false; self.faces.len()];
        let mut selected_edges = Vec::new();
        match selection {
            MeshExtrudeSelection::AllFaces => selected.fill(true),
            MeshExtrudeSelection::Faces(indices) => {
                if indices.is_empty() {
                    return Err(GeometryError::Degenerate {
                        context: "mesh extrusion selection",
                    });
                }
                for &index in indices {
                    let face =
                        selected
                            .get_mut(index)
                            .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                                face: index,
                                face_count: self.faces.len(),
                            })?;
                    *face = true;
                }
            }
            MeshExtrudeSelection::BoundaryEdges(indices) => {
                if indices.is_empty() {
                    return Err(GeometryError::Degenerate {
                        context: "mesh extrusion selection",
                    });
                }
                for &index in indices {
                    let edge = edge_table.get(index).ok_or(
                        GeometryError::MeshTopologyEdgeIndexOutOfRange {
                            edge: index,
                            edge_count: edge_table.len(),
                        },
                    )?;
                    if edge.count != 1 {
                        return Err(GeometryError::Degenerate {
                            context: "mesh extrusion requires naked edges",
                        });
                    }
                    if !selected_edges.contains(&index) {
                        selected_edges.push(index);
                    }
                }
            }
        }
        if matches!(direction, MeshExtrudeDirection::EdgeOutward) && selected_edges.is_empty() {
            return Err(GeometryError::Degenerate {
                context: "edge outward extrusion requires naked edges",
            });
        }
        let normals = self.polygon_face_normals()?;
        let mut vertex_directions =
            vec![Vector3::try_new(0.0, 0.0, 0.0)?; topology.topological_vertex_count];
        if matches!(direction, MeshExtrudeDirection::VertexNormals) {
            let mut counts = vec![0usize; topology.topological_vertex_count];
            let source_faces: Vec<_> = if selected_edges.is_empty() {
                selected
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &yes)| yes.then_some(i))
                    .collect()
            } else {
                selected_edges
                    .iter()
                    .map(|&i| edge_table[i].first_use.unwrap().face)
                    .collect()
            };
            for face_index in source_faces {
                for &raw in self.faces[face_index].indices() {
                    let vertex = topology.topological_vertices[raw as usize];
                    let sum = vertex_directions[vertex].to_array();
                    let normal = normals[face_index].as_vector().to_array();
                    vertex_directions[vertex] =
                        Vector3::try_from(std::array::from_fn(|i| sum[i] + normal[i]))?;
                    counts[vertex] += 1;
                }
            }
            for (vector, count) in vertex_directions.iter_mut().zip(counts) {
                if count > 0 {
                    *vector = vector.normalized_nonzero()?.as_vector();
                }
            }
        }
        let mut displaced = BTreeMap::<(u32, usize), u32>::new();
        let mut vertices = self.vertices.clone();
        let mut colors = self.vertex_colors.clone();
        let mut add_displaced =
            |raw: u32, edge_vector: Option<(Vector3, usize)>| -> Result<u32, GeometryError> {
                let key = (raw, edge_vector.map_or(0, |(_, index)| index + 1));
                if let Some(&index) = displaced.get(&key) {
                    return Ok(index);
                }
                let vector = match direction {
                    MeshExtrudeDirection::World(vector) => vector.as_vector(),
                    MeshExtrudeDirection::VertexNormals => {
                        vertex_directions[topology.topological_vertices[raw as usize]]
                    }
                    MeshExtrudeDirection::EdgeOutward => {
                        edge_vector
                            .ok_or(GeometryError::Degenerate {
                                context: "edge outward extrusion direction",
                            })?
                            .0
                    }
                };
                let point = self.vertices[raw as usize].translated(vector.scaled(distance)?)?;
                let index = u32::try_from(vertices.len())
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                vertices.push(point);
                if let (Some(source), Some(output)) = (&self.vertex_colors, &mut colors) {
                    output.push(source[raw as usize]);
                }
                displaced.insert(key, index);
                Ok(index)
            };
        let mut faces = Vec::new();
        let mut face_map = vec![None; self.faces.len()];
        if selected_edges.is_empty() {
            for (i, &face) in self.faces.iter().enumerate() {
                if !selected[i] {
                    face_map[i] = Some(faces.len() as u32);
                    faces.push(face);
                }
            }
            for (i, &face) in self.faces.iter().enumerate() {
                if selected[i] {
                    let cap = match face {
                        MeshFace::Triangle([a, b, c]) => MeshFace::Triangle([
                            add_displaced(a, None)?,
                            add_displaced(b, None)?,
                            add_displaced(c, None)?,
                        ]),
                        MeshFace::Quad([a, b, c, d]) => MeshFace::Quad([
                            add_displaced(a, None)?,
                            add_displaced(b, None)?,
                            add_displaced(c, None)?,
                            add_displaced(d, None)?,
                        ]),
                    };
                    face_map[i] = Some(faces.len() as u32);
                    faces.push(cap);
                }
            }
            for edge in topology.edges.values() {
                let uses: Vec<_> = edge.uses().filter(|usage| selected[usage.face]).collect();
                if uses.len() != 1 {
                    continue;
                }
                let usage = uses[0];
                let indices = self.faces[usage.face].indices();
                let a = indices[usage.side];
                let b = indices[(usage.side + 1) % indices.len()];
                faces.push(MeshFace::Quad([
                    a,
                    b,
                    add_displaced(b, None)?,
                    add_displaced(a, None)?,
                ]));
            }
        } else {
            faces.extend_from_slice(&self.faces);
            for (i, slot) in face_map.iter_mut().enumerate() {
                *slot = Some(i as u32);
            }
            for &edge_index in &selected_edges {
                let edge = edge_table[edge_index];
                let usage = edge.first_use.unwrap();
                let indices = self.faces[usage.face].indices();
                let a = indices[usage.side];
                let b = indices[(usage.side + 1) % indices.len()];
                let outward = if matches!(direction, MeshExtrudeDirection::EdgeOutward) {
                    Some((
                        self.vertices[a as usize]
                            .vector_to(self.vertices[b as usize])?
                            .normalized_nonzero()?
                            .as_vector()
                            .cross(normals[usage.face].as_vector())?
                            .normalized_nonzero()?
                            .as_vector(),
                        edge_index,
                    ))
                } else {
                    None
                };
                faces.push(MeshFace::Quad([
                    b,
                    a,
                    add_displaced(a, outward)?,
                    add_displaced(b, outward)?,
                ]));
            }
        }
        if faces.len() > u32::MAX as usize {
            return Err(GeometryError::TooManyMeshFaces);
        }
        let mut mesh =
            Self::try_new_faces(vertices, faces, tolerance)?.try_with_vertex_colors(colors)?;
        let ngons = self
            .ngons
            .iter()
            .filter_map(|ngon| {
                let mapped = ngon
                    .faces
                    .iter()
                    .map(|&i| face_map[i as usize])
                    .collect::<Option<Vec<_>>>()?;
                mesh.ngon_from_faces(mapped)
            })
            .collect();
        mesh = mesh.try_with_ngons(ngons)?;
        Ok(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn selected_face_of_closed_box_remains_closed_and_oriented() {
        let frame = Frame3::try_from_directions(
            point(0., 0., 0.),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mesh = TriangleMesh::try_box_grid(
            frame,
            [[0., 2.], [0., 2.], [0., 2.]],
            1,
            1,
            1,
            Tolerance::DEFAULT,
        );
        // The selected cap replaces the source face, and its four boundary
        // walls connect to the five unchanged sides.
        let mesh = mesh.unwrap();
        let normal = mesh.polygon_face_normals().unwrap()[0];
        let out = mesh
            .extrude_mesh(
                &MeshExtrudeSelection::Faces(vec![0]),
                1.,
                MeshExtrudeDirection::World(normal),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(out.face_count(), mesh.face_count() + 4);
        assert!(out.topology().is_solid());
    }

    #[test]
    fn boundary_edge_requires_naked_topology_and_keeps_vertex_colors() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(1., 1., 0.),
                point(0., 1., 0.),
            ],
            vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some(vec![[1, 2, 3, 4]; 4]))
        .unwrap();
        let edges = mesh.topology_edge_points();
        let interior = edges
            .iter()
            .position(|edge| edge.contains(&point(0., 0., 0.)) && edge.contains(&point(1., 1., 0.)))
            .unwrap();
        assert!(
            mesh.extrude_mesh(
                &MeshExtrudeSelection::BoundaryEdges(vec![interior]),
                1.,
                MeshExtrudeDirection::VertexNormals,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        let boundary = edges
            .iter()
            .position(|edge| edge.contains(&point(0., 0., 0.)) && edge.contains(&point(1., 0., 0.)))
            .unwrap();
        let out = mesh
            .extrude_mesh(
                &MeshExtrudeSelection::BoundaryEdges(vec![boundary]),
                1.,
                MeshExtrudeDirection::VertexNormals,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(out.face_count(), 3);
        assert_eq!(out.vertex_colors().unwrap().len(), 6);
        assert_eq!(&out.vertex_colors().unwrap()[4..], &[[1, 2, 3, 4]; 2]);
    }
}
