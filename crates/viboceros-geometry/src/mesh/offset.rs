//! Vertex displacement and boundary walls for mesh offsets.

use super::*;
use crate::Vector3;
use std::collections::BTreeSet;

/// Direction used for every mesh vertex. Coincident raw vertices share a
/// topological offset direction in `VertexNormals` mode.
#[derive(Clone, Copy, Debug)]
pub enum MeshOffsetDirection {
    VertexNormals,
    AverageNormals { fallback: UnitVector3 },
    Vector(UnitVector3),
}

impl TriangleMesh {
    /// Offset an indexed mesh, optionally including both displaced skins and
    /// walls along naked topological edges. Positive distance follows the
    /// stored polygon winding. A solid offset of a closed mesh has two shells.
    pub fn offset_mesh(
        &self,
        distance: Real,
        direction: MeshOffsetDirection,
        both_sides: bool,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "mesh offset distance",
            });
        }
        let directions = self.offset_directions(direction)?;
        let make_skin = |signed_distance: Real| -> Result<Self, GeometryError> {
            let mut vertices = Vec::with_capacity(self.vertices.len());
            for (&point, normal) in self.vertices.iter().zip(&directions) {
                vertices.push(point.translated(normal.scaled(signed_distance)?)?);
            }
            let mut mesh = Self::try_new_faces(vertices, self.faces.clone(), tolerance)?;
            mesh.vertex_colors = self.vertex_colors.clone();
            mesh.ngons = self.ngons.clone();
            Ok(mesh)
        };
        let positive = (both_sides || distance > 0.0)
            .then(|| make_skin(distance.abs()))
            .transpose()?;
        let negative = (both_sides || distance < 0.0)
            .then(|| make_skin(-distance.abs()))
            .transpose()?;
        if !solid {
            return if both_sides {
                Self::try_append(&[negative.as_ref().unwrap(), positive.as_ref().unwrap()])
            } else if distance > 0.0 {
                Ok(positive.unwrap())
            } else {
                Ok(negative.unwrap())
            };
        }
        let topology = self.topology();
        if !topology.is_manifold() || !topology.is_oriented() {
            return Err(GeometryError::Degenerate {
                context: "mesh offset solid topology",
            });
        }
        let (first, second, reverse_first) = if both_sides {
            (negative.unwrap(), positive.unwrap(), true)
        } else if distance < 0.0 {
            (negative.unwrap(), self.clone(), true)
        } else {
            (positive.unwrap(), self.clone(), false)
        };
        self.offset_solid_between(&first, &second, reverse_first, tolerance)
    }

    fn offset_directions(
        &self,
        direction: MeshOffsetDirection,
    ) -> Result<Vec<Vector3>, GeometryError> {
        if let MeshOffsetDirection::Vector(vector) = direction {
            return Ok(vec![vector.as_vector(); self.vertices.len()]);
        }
        // Follow the public ON_Mesh::ComputeVertexNormals and OffsetMesh
        // algorithm in third_party/opennurbs/opennurbs_mesh.cpp: compute
        // float vertex normals, then average them for each exact-location
        // topology vertex before displacing all its raw copies.
        let normals = self.polygon_face_normals()?;
        let mut sums = vec![[0.0_f32; 3]; self.vertices.len()];
        for (face, normal) in self.faces.iter().zip(normals).rev() {
            for &index in face.indices() {
                let index = index as usize;
                for (sum, coordinate) in sums[index].iter_mut().zip(normal.as_vector().to_array()) {
                    *sum += coordinate as f32;
                }
            }
        }
        let world_z = Vector3::try_new(0.0, 0.0, 1.0)?;
        let raw_normals = sums
            .iter()
            .map(|&sum| {
                let vector = Vector3::try_new(sum[0] as Real, sum[1] as Real, sum[2] as Real)?;
                let normalized = vector
                    .normalized_nonzero()
                    .map(UnitVector3::as_vector)
                    .unwrap_or(world_z);
                Vector3::try_from(
                    normalized
                        .to_array()
                        .map(|coordinate| coordinate as f32 as Real),
                )
            })
            .collect::<Result<Vec<Vector3>, GeometryError>>()?;
        match direction {
            MeshOffsetDirection::VertexNormals => {
                let topology = self.topology_data();
                let mut sums = vec![[0.0_f64; 3]; topology.topological_vertex_count];
                for (normal, &vertex) in raw_normals.iter().zip(&topology.topological_vertices) {
                    for (sum, component) in sums[vertex].iter_mut().zip(normal.to_array()) {
                        *sum += component;
                    }
                }
                let directions = sums
                    .into_iter()
                    .map(|sum| {
                        Vector3::try_from(sum).map(|vector| {
                            vector
                                .normalized_nonzero()
                                .map(UnitVector3::as_vector)
                                .unwrap_or(Vector3::try_new(0.0, 0.0, 0.0).unwrap())
                        })
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Ok(topology
                    .topological_vertices
                    .iter()
                    .map(|&index| directions[index])
                    .collect())
            }
            MeshOffsetDirection::AverageNormals { fallback } => {
                let mut sum = [0.0_f64; 3];
                for normal in &raw_normals {
                    for (total, component) in sum.iter_mut().zip(normal.to_array()) {
                        *total += component;
                    }
                }
                let average = if sum.iter().map(|value| value.abs()).fold(0.0, Real::max)
                    <= 32.0 * Real::EPSILON * raw_normals.len() as Real
                {
                    fallback.as_vector()
                } else {
                    Vector3::try_from(sum)?.normalized_nonzero()?.as_vector()
                };
                Ok(vec![average; self.vertices.len()])
            }
            MeshOffsetDirection::Vector(_) => unreachable!(),
        }
    }

    fn offset_solid_between(
        &self,
        first: &Self,
        second: &Self,
        reverse_first: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let source_vertices = self.topology().topological_vertex_count();
        if first.topology().topological_vertex_count() < source_vertices
            || second.topology().topological_vertex_count() < source_vertices
        {
            // Rhino retains both skins but omits the walls when the offset
            // has merged source topology vertices, as on a folded mesh.
            let first_skin = if reverse_first {
                first.reversed()
            } else {
                first.clone()
            };
            let second_skin = if reverse_first {
                second.clone()
            } else {
                second.reversed()
            };
            return Self::try_append(&[&first_skin, &second_skin]);
        }
        let count = self.vertices.len();
        let offset = u32::try_from(count).map_err(|_| GeometryError::TooManyMeshVertices)?;
        let (_, naked) = self.topology_with_boundary();
        count
            .checked_mul(2)
            .and_then(|total| {
                naked
                    .len()
                    .checked_mul(4)
                    .and_then(|walls| total.checked_add(walls))
            })
            .and_then(|total| total.checked_sub(1))
            .filter(|&last| u32::try_from(last).is_ok())
            .ok_or(GeometryError::TooManyMeshVertices)?;
        self.faces
            .len()
            .checked_mul(2)
            .and_then(|count| count.checked_add(naked.len()))
            .and_then(|count| count.checked_sub(1))
            .filter(|&last| u32::try_from(last).is_ok())
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let mut vertices = first.vertices.clone();
        vertices.extend_from_slice(&second.vertices);
        let mut faces = self
            .faces
            .iter()
            .copied()
            .map(|face| if reverse_first { face.reversed() } else { face })
            .collect::<Vec<_>>();
        faces.extend(self.faces.iter().copied().map(|face| {
            let face = if reverse_first { face } else { face.reversed() };
            face.remapped(|i| i + offset)
        }));
        let naked_sides = naked
            .into_iter()
            .map(|edge| {
                let [a, b] = edge.vertices;
                (edge.face, a.min(b), a.max(b))
            })
            .collect::<BTreeSet<_>>();
        let mut boundary_sides = Vec::with_capacity(naked_sides.len());
        for (face_index, face) in self.faces.iter().enumerate() {
            let indices = face.indices();
            // Keep each naked side in source face order before tracing loops.
            for side in 0..indices.len() {
                let a = indices[(side + 1) % indices.len()];
                let b = indices[(side + 2) % indices.len()];
                if naked_sides.contains(&(face_index, a.min(b), a.max(b))) {
                    boundary_sides.push([a, b]);
                }
            }
        }
        let source_topology = self.topology_data();
        let mut outgoing = vec![Vec::new(); source_topology.topological_vertex_count];
        for (index, &[a, _]) in boundary_sides.iter().enumerate() {
            outgoing[source_topology.topological_vertices[a as usize]].push(index);
        }
        let mut used = vec![false; boundary_sides.len()];
        let mut boundary_loops = Vec::new();
        for start in 0..boundary_sides.len() {
            if used[start] {
                continue;
            }
            let mut current = start;
            let start_vertex =
                source_topology.topological_vertices[boundary_sides[start][0] as usize];
            let mut loop_sides = Vec::new();
            while !used[current] {
                used[current] = true;
                let [a, b] = boundary_sides[current];
                loop_sides.push([a, b]);
                let end = source_topology.topological_vertices[b as usize];
                if end == start_vertex {
                    break;
                }
                let Some(next) = outgoing[end].iter().copied().find(|&edge| !used[edge]) else {
                    break;
                };
                current = next;
            }
            let minimum = loop_sides
                .iter()
                .flat_map(|&[a, b]| [a, b])
                .map(|vertex| source_topology.topological_vertices[vertex as usize])
                .min()
                .expect("a boundary trail has at least one side");
            if loop_sides.len() > 1 {
                // Rhino orders loops by their least topology vertex. The
                // incident edge with the least neighbor establishes the
                // starting wall: advance from it for a forward edge, or walk
                // backward from it for a reverse edge (as around a hole).
                let reference = loop_sides
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &[a, b])| {
                        let a = source_topology.topological_vertices[a as usize];
                        let b = source_topology.topological_vertices[b as usize];
                        if a == minimum {
                            Some((b, index, true))
                        } else if b == minimum {
                            Some((a, index, false))
                        } else {
                            None
                        }
                    })
                    .min_by_key(|&(neighbor, index, _)| (neighbor, index))
                    .expect("the minimum boundary vertex has a side");
                let (_, reference, forward) = reference;
                let first = if forward {
                    (reference + 1) % loop_sides.len()
                } else {
                    (reference + loop_sides.len() - 1) % loop_sides.len()
                };
                if forward {
                    loop_sides.rotate_left(first);
                } else {
                    loop_sides.reverse();
                    let reversed_first = loop_sides.len() - 1 - first;
                    loop_sides.rotate_left(reversed_first);
                }
            }
            boundary_loops.push((minimum, loop_sides));
        }
        boundary_loops.sort_by_key(|(minimum, _)| *minimum);
        let mut colors = first
            .vertex_colors
            .as_ref()
            .zip(second.vertex_colors.as_ref())
            .map(|(first_colors, second_colors)| {
                let mut colors = first_colors.clone();
                colors.extend_from_slice(second_colors);
                colors
            });
        for [a, b] in boundary_loops.into_iter().flat_map(|(_, sides)| sides) {
            let base =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.extend([
                first.vertices[a as usize],
                first.vertices[b as usize],
                second.vertices[b as usize],
                second.vertices[a as usize],
            ]);
            if let Some(colors) = &mut colors {
                let first_colors = first.vertex_colors.as_ref().unwrap();
                let second_colors = second.vertex_colors.as_ref().unwrap();
                colors.extend([
                    first_colors[a as usize],
                    first_colors[b as usize],
                    second_colors[b as usize],
                    second_colors[a as usize],
                ]);
            }
            faces.push(if reverse_first {
                MeshFace::Quad([base, base + 1, base + 2, base + 3])
            } else {
                MeshFace::Quad([base, base + 3, base + 2, base + 1])
            });
        }
        let mut mesh = Self::try_new_faces(vertices, faces, tolerance)?;
        mesh.vertex_colors = colors;
        let face_offset =
            u32::try_from(self.faces.len()).map_err(|_| GeometryError::TooManyMeshFaces)?;
        let mut ngons = Vec::with_capacity(self.ngons.len() * 2);
        for source in &self.ngons {
            for shift in [0, face_offset] {
                let faces = source.faces.iter().map(|&face| face + shift).collect();
                if let Some(ngon) = mesh.ngon_from_faces(faces) {
                    ngons.push(ngon);
                }
            }
        }
        mesh.ngons = ngons;
        if !mesh.topology().is_solid() {
            return Err(GeometryError::Degenerate {
                context: "mesh offset solid closure",
            });
        }
        Ok(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> TriangleMesh {
        TriangleMesh::try_new(
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|point| Point3::try_from(point).unwrap())
                .collect(),
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn normal_offset_and_solid_shell_have_expected_geometry() {
        let source = triangle();
        let offset = source
            .offset_mesh(
                2.0,
                MeshOffsetDirection::VertexNormals,
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(offset.face_count(), 1);
        assert!(offset.vertices().iter().all(|point| point.z() == 2.0));
        for distance in [2.0, -2.0] {
            let shell = source
                .offset_mesh(
                    distance,
                    MeshOffsetDirection::VertexNormals,
                    false,
                    true,
                    Tolerance::DEFAULT,
                )
                .unwrap();
            assert_eq!(shell.face_count(), 5);
            assert!(shell.topology().is_solid());
            assert!((shell.signed_volume().unwrap() - 1.0).abs() < 1e-12);
        }
        let both = source
            .offset_mesh(
                2.0,
                MeshOffsetDirection::VertexNormals,
                true,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(both.topology().is_solid());
        assert!((both.signed_volume().unwrap() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn solid_offset_uses_rhino_skin_and_raw_wall_order() {
        let source = triangle();
        for (distance, both_sides, first_z, second_z, reversed_first) in [
            (1.0, false, 1.0, 0.0, false),
            (-1.0, false, -1.0, 0.0, true),
            (1.0, true, -1.0, 1.0, true),
        ] {
            let mesh = source
                .offset_mesh(
                    distance,
                    MeshOffsetDirection::VertexNormals,
                    both_sides,
                    true,
                    Tolerance::DEFAULT,
                )
                .unwrap();
            assert_eq!(mesh.vertices().len(), 18);
            assert_eq!(mesh.faces().len(), 5);
            assert!(
                mesh.vertices()[..3]
                    .iter()
                    .all(|point| point.z() == first_z)
            );
            assert!(
                mesh.vertices()[3..6]
                    .iter()
                    .all(|point| point.z() == second_z)
            );
            assert_eq!(
                mesh.faces()[0],
                if reversed_first {
                    MeshFace::Triangle([0, 2, 1])
                } else {
                    MeshFace::Triangle([0, 1, 2])
                }
            );
            assert_eq!(
                mesh.faces()[1],
                if reversed_first {
                    MeshFace::Triangle([3, 4, 5])
                } else {
                    MeshFace::Triangle([3, 5, 4])
                }
            );
            let wall = if reversed_first {
                MeshFace::Quad([6, 7, 8, 9])
            } else {
                MeshFace::Quad([6, 9, 8, 7])
            };
            assert_eq!(mesh.faces()[2], wall);
            assert_eq!(mesh.vertices()[6].to_array(), [1.0, 0.0, first_z]);
            assert_eq!(mesh.vertices()[7].to_array(), [0.0, 1.0, first_z]);
            assert_eq!(mesh.vertices()[8].to_array(), [0.0, 1.0, second_z]);
            assert_eq!(mesh.vertices()[9].to_array(), [1.0, 0.0, second_z]);
        }

        let quad = TriangleMesh::try_new_faces(
            [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|value| Point3::try_from(value).unwrap())
                .collect(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let solid = quad
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(solid.vertices().len(), 24);
        assert_eq!(solid.faces().len(), 6);
        assert_eq!(solid.faces()[0], MeshFace::Quad([0, 1, 2, 3]));
        assert_eq!(solid.faces()[1], MeshFace::Quad([4, 7, 6, 5]));
        let wall_starts = [1, 2, 3, 0];
        for (wall, &source_vertex) in wall_starts.iter().enumerate() {
            assert_eq!(
                solid.vertices()[8 + 4 * wall].to_array()[..2],
                quad.vertices()[source_vertex].to_array()[..2]
            );
            assert_eq!(
                solid.faces()[2 + wall],
                MeshFace::Quad([
                    (8 + 4 * wall) as u32,
                    (11 + 4 * wall) as u32,
                    (10 + 4 * wall) as u32,
                    (9 + 4 * wall) as u32,
                ])
            );
        }
    }

    #[test]
    fn solid_offset_with_collapsed_skin_matches_rhino_two_skin_fallback() {
        let source = TriangleMesh::try_new_faces(
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
                .into_iter()
                .map(|value| Point3::try_from(value).unwrap())
                .collect(),
            vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 3, 1])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let result = source
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(result.vertices().len(), 8);
        assert_eq!(result.faces().len(), 4);
    }

    #[test]
    fn solid_offset_traces_boundary_across_source_faces() {
        let source = TriangleMesh::try_new(
            [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|value| Point3::try_from(value).unwrap())
                .collect(),
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let solid = source
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(solid.vertices().len(), 24);
        assert_eq!(solid.faces().len(), 8);
        for (wall, source_vertex) in [1, 2, 3, 0].into_iter().enumerate() {
            assert_eq!(
                solid.vertices()[8 + 4 * wall].to_array()[..2],
                source.vertices()[source_vertex].to_array()[..2]
            );
        }
    }

    #[test]
    fn average_opposing_normals_uses_supplied_fallback() {
        let source = triangle();
        let opposed = TriangleMesh::try_append(&[&source, &source.reversed()]).unwrap();
        let fallback = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        let output = opposed
            .offset_mesh(
                3.0,
                MeshOffsetDirection::AverageNormals { fallback },
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        for (before, after) in opposed.vertices().iter().zip(output.vertices()) {
            assert_eq!(after.x() - before.x(), 3.0);
            assert_eq!(after.y(), before.y());
            assert_eq!(after.z(), before.z());
        }
    }

    #[test]
    fn solid_offset_rejects_collapsed_boundary_walls() {
        let source = triangle();
        let direction = UnitVector3::try_new(1.0, 0.0, 0.0, Tolerance::DEFAULT).unwrap();
        assert!(
            source
                .offset_mesh(
                    1.0,
                    MeshOffsetDirection::Vector(direction),
                    false,
                    true,
                    Tolerance::DEFAULT
                )
                .is_err()
        );
    }

    #[test]
    fn closed_mesh_solid_offset_has_two_oriented_shells() {
        let mesh = TriangleMesh::try_new(
            [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
                .into_iter()
                .map(|point| Point3::try_from(point).unwrap())
                .collect(),
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let shell = mesh
            .offset_mesh(
                0.1,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(shell.topology().is_solid());
        assert_eq!(shell.disjoint_pieces().len(), 2);
        assert!(shell.signed_volume().unwrap() > 0.0);
    }

    #[test]
    fn colored_ngon_survives_open_and_solid_offset() {
        let mesh = TriangleMesh::try_new(
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
                .into_iter()
                .map(|point| Point3::try_from(point).unwrap())
                .collect(),
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .try_with_vertex_colors(Some(vec![[10, 20, 30, 0]; 4]))
        .unwrap()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0, 1])])
        .unwrap();
        let open = mesh
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(open.vertex_colors(), mesh.vertex_colors());
        assert_eq!(open.ngons(), mesh.ngons());
        let shell = mesh
            .offset_mesh(
                1.0,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(shell.vertex_colors().unwrap().len(), 24);
        assert_eq!(shell.ngons().len(), 2);
    }

    #[test]
    fn solid_offset_keeps_unwelded_crease_joined() {
        let mesh = TriangleMesh::try_new(
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 0., 1.],
            ]
            .into_iter()
            .map(|point| Point3::try_from(point).unwrap())
            .collect(),
            vec![[0, 1, 2], [4, 3, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let offset = mesh
            .offset_mesh(
                0.25,
                MeshOffsetDirection::VertexNormals,
                false,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(offset.vertices()[0], offset.vertices()[3]);
        assert_eq!(offset.vertices()[1], offset.vertices()[4]);
        let shell = mesh
            .offset_mesh(
                0.25,
                MeshOffsetDirection::VertexNormals,
                false,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert!(shell.topology().is_solid());
    }
}
