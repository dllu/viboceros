//! Naked-boundary filling and planar cap policy, using shared polygon geometry.
use super::*;

impl TriangleMesh {
    /// Fills the closed naked boundary containing one topology edge.
    ///
    /// The selected index follows [`Self::wireframe_lines`]. `None` means the
    /// edge is not naked, its boundary branches or does not close, or its
    /// projected polygon cannot be constrained-triangulated. Boundary points
    /// are copied into a separate raw-vertex patch, including Rhino's unused
    /// closing duplicate in the joined result. Exact-location topology still
    /// joins the patch to the source. Generated winding is made opposite the
    /// face at the selected edge so a consistently oriented input remains
    /// consistently oriented after filling.
    pub fn fill_topology_hole(
        &self,
        edge_index: usize,
        tolerance: Tolerance,
    ) -> Result<Option<MeshHoleFill>, GeometryError> {
        let data = self.topology_data();
        self.fill_topology_hole_with_data(edge_index, tolerance, &data)
    }

    fn fill_topology_hole_with_data(
        &self,
        edge_index: usize,
        tolerance: Tolerance,
        data: &MeshTopologyData,
    ) -> Result<Option<MeshHoleFill>, GeometryError> {
        let edge_count = data.edges.len();
        let Some((&selected_edge, selected_incidence)) = data.edges.iter().nth(edge_index) else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        if selected_incidence.count != 1 {
            return Ok(None);
        }

        let naked_edges = data
            .edges
            .iter()
            .filter_map(|(&edge, incidence)| (incidence.count == 1).then_some(edge))
            .collect::<Vec<_>>();
        let Some(selected_naked_edge) = naked_edges
            .iter()
            .position(|&candidate| candidate == selected_edge)
        else {
            return Ok(None);
        };
        let mut adjacency = vec![Vec::new(); data.topological_vertex_count];
        for (naked_edge_index, &(first, second)) in naked_edges.iter().enumerate() {
            adjacency[first].push(naked_edge_index);
            adjacency[second].push(naked_edge_index);
        }

        let mut boundary = Vec::new();
        let start = selected_edge.0;
        let mut current = start;
        let mut current_edge = selected_naked_edge;
        let mut used = vec![false; naked_edges.len()];
        loop {
            if used[current_edge] {
                return Ok(None);
            }
            used[current_edge] = true;
            boundary.push(current);
            let (first, second) = naked_edges[current_edge];
            let next = if current == first {
                second
            } else if current == second {
                first
            } else {
                return Ok(None);
            };
            if next == start {
                break;
            }
            if adjacency[next].len() != 2 {
                return Ok(None);
            }
            let Some(next_edge) = adjacency[next]
                .iter()
                .copied()
                .find(|&candidate| !used[candidate])
            else {
                return Ok(None);
            };
            current = next;
            current_edge = next_edge;
        }
        if boundary.len() < 3
            || adjacency[start].len() != 2
            || boundary.iter().copied().collect::<BTreeSet<_>>().len() != boundary.len()
        {
            return Ok(None);
        }

        let boundary_points = boundary
            .iter()
            .map(|&vertex| data.topological_points[vertex])
            .collect::<Vec<_>>();
        let Some(projected) = project_mesh_hole_boundary(&boundary_points)? else {
            return Ok(None);
        };
        let Some(mut patch_triangles) = triangulate_projected_mesh_hole(&projected)? else {
            return Ok(None);
        };
        let boundary_area = projected_polygon_doubled_area(&projected);
        let selected_forward = selected_incidence
            .first_use
            .expect("a naked edge records one face use")
            .forward;
        let desired_positive = (boundary_area > 0.0) != selected_forward;
        for triangle in &mut patch_triangles {
            let points = triangle.map(|vertex| projected[vertex as usize]);
            let positive = mesh_hole_cross(points[0], points[1], points[2]) > 0.0;
            if positive != desired_positive {
                triangle.swap(1, 2);
            }
        }

        let patch_faces = patch_triangles
            .iter()
            .copied()
            .map(MeshFace::Triangle)
            .collect::<Vec<_>>();
        let mut raw_by_topology = vec![None; data.topological_vertex_count];
        for incidence in data.edges.values().filter(|incidence| incidence.count == 1) {
            for raw in incidence.first_use.unwrap().raw_vertices {
                raw_by_topology[data.topological_vertices[raw as usize]].get_or_insert(raw);
            }
        }
        let boundary_raw = boundary
            .iter()
            .map(|&topology| {
                raw_by_topology[topology].expect("a naked boundary vertex belongs to a naked edge")
                    as usize
            })
            .collect::<Vec<_>>();
        let boundary_colors = self.vertex_colors.as_ref().map(|colors| {
            boundary_raw
                .iter()
                .map(|&raw| colors[raw])
                .collect::<Vec<_>>()
        });
        let mut patch = Self::try_new_faces(boundary_points.clone(), patch_faces, tolerance)?
            .try_with_vertex_colors(boundary_colors.clone())?;
        patch.vertex_channels = self.vertex_channels.remapped(boundary_raw.iter().copied());
        patch.recompute_stored_normals()?;

        let added_vertex_count = boundary_points.len().saturating_add(1);
        let total_vertex_count = self
            .vertices
            .len()
            .checked_add(added_vertex_count)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if total_vertex_count
            .checked_sub(1)
            .is_some_and(|last| u32::try_from(last).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let offset = u32::try_from(self.vertices.len())
            .expect("the checked appended mesh vertex range starts within u32");
        let mut vertices = self.vertices.clone();
        vertices
            .try_reserve(added_vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.extend(boundary_points.iter().copied());
        vertices.push(boundary_points[0]);
        let mut faces = self.faces.clone();
        faces
            .try_reserve(patch_triangles.len())
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for triangle in patch_triangles {
            let mapped = triangle.map(|vertex| {
                offset
                    .checked_add(vertex)
                    .expect("the reserved mesh vertex range fits in u32")
            });
            faces.push(MeshFace::Triangle(mapped));
        }
        let mut filled = Self::try_new_faces(vertices, faces, tolerance)?;
        if let Some(boundary_colors) = boundary_colors {
            let mut colors = self.vertex_colors.as_ref().unwrap().clone();
            colors.extend_from_slice(&boundary_colors);
            colors.push(boundary_colors[0]);
            filled.vertex_colors = Some(colors);
        }
        filled.vertex_channels = self.vertex_channels.remapped(
            (0..self.vertices.len())
                .chain(boundary_raw.iter().copied())
                .chain([boundary_raw[0]]),
        );
        filled.recompute_stored_normals()?;
        filled.ngons = self.ngons.clone();
        Ok(Some(MeshHoleFill { filled, patch }))
    }

    /// Fills every unambiguous closed naked boundary with triangles.
    ///
    /// Boundaries are processed in deterministic topology-edge order and the
    /// search restarts after each patch because topology indices change. This
    /// mirrors Rhino's all-holes operation, including treating outer naked
    /// borders as holes, while omitting the singular operation's unused
    /// closing duplicate after each boundary. The returned count is the
    /// number of filled boundary loops.
    pub fn fill_holes(&self, tolerance: Tolerance) -> Result<(Self, usize), GeometryError> {
        let mut filled = self.clone();
        let mut filled_hole_count = 0_usize;
        loop {
            let data = filled.topology_data();
            let mut next = None;
            for (edge_index, _) in data
                .edges
                .iter()
                .enumerate()
                .filter(|(_, (_, incidence))| incidence.count == 1)
            {
                if let Some(fill) =
                    filled.fill_topology_hole_with_data(edge_index, tolerance, &data)?
                {
                    next = Some(fill.filled);
                    break;
                }
            }
            let Some(mut next) = next else {
                break;
            };
            let closing_duplicate = next
                .vertices
                .pop()
                .expect("a filled boundary appends an unused closing vertex");
            next.vertex_channels.pop();
            if let Some(colors) = &mut next.vertex_colors {
                colors.pop();
            }
            debug_assert!(
                !next
                    .faces
                    .iter()
                    .flat_map(MeshFace::indices)
                    .any(|&vertex| vertex as usize == next.vertices.len()),
                "the closing boundary duplicate is unused"
            );
            debug_assert!(
                next.vertices.contains(&closing_duplicate),
                "the removed vertex duplicates the boundary start"
            );
            filled = next;
            filled_hole_count = filled_hole_count
                .checked_add(1)
                .ok_or(GeometryError::TooManyMeshFaces)?;
        }
        Ok((filled, filled_hole_count))
    }

    /// Caps closed, unambiguous naked boundaries whose vertices lie within the
    /// absolute modelling tolerance of a common plane. Other openings remain
    /// untouched. Disjoint coplanar inner loops are triangulated together with
    /// their outer loop; simple loops use the mesh hole filler's winding and
    /// constrained triangulation. Unused closing vertices are removed.
    pub fn cap_planar_holes(&self, tolerance: Tolerance) -> Result<(Self, usize), GeometryError> {
        self.cap_planar_holes_with_crease(tolerance, true)
    }

    /// Caps planar openings, optionally welding cap boundary vertices to the
    /// source mesh. `crease = true` keeps separate raw vertices at the join;
    /// exact-location topology is closed in either case.
    pub fn cap_planar_holes_with_crease(
        &self,
        tolerance: Tolerance,
        crease: bool,
    ) -> Result<(Self, usize), GeometryError> {
        self.cap_planar_holes_with_options(tolerance, crease, true)
    }

    /// When `triangles` is false, each simply connected planar cap receives
    /// one logical n-gon over its triangle faces. Caps with inner boundaries
    /// retain separate triangles.
    pub fn cap_planar_holes_with_options(
        &self,
        tolerance: Tolerance,
        crease: bool,
        triangles: bool,
    ) -> Result<(Self, usize), GeometryError> {
        let mut capped = self.clone();
        let mut count = 0_usize;
        let mut simple_cap_face_ranges = Vec::new();
        loop {
            let data = capped.topology_data();
            if let Some((next, filled_boundaries)) =
                planar_cap::try_cap_annular(&capped, &data, tolerance)?
            {
                capped = next;
                count = count
                    .checked_add(filled_boundaries)
                    .ok_or(GeometryError::TooManyMeshFaces)?;
                continue;
            }
            let mut next = None;
            for (edge, _) in data
                .edges
                .iter()
                .enumerate()
                .filter(|(_, (_, incidence))| incidence.count == 1)
            {
                let Some(fill) = capped.fill_topology_hole_with_data(edge, tolerance, &data)?
                else {
                    continue;
                };
                let boundary = fill.patch.vertices();
                let plane = PointProjection3::onto_best_fit_plane(boundary)?;
                let planar = boundary.iter().copied().try_fold(true, |planar, point| {
                    Ok::<_, GeometryError>(
                        planar && point.distance_to(plane.project(point)?)? <= tolerance.absolute(),
                    )
                })?;
                if planar && !coplanar_mesh_boundary_overlaps(&data, boundary, &plane, tolerance)? {
                    next = Some(fill.filled);
                    break;
                }
            }
            let Some(mut next) = next else {
                break;
            };
            next.vertices.pop();
            next.vertex_channels.pop();
            if let Some(colors) = &mut next.vertex_colors {
                colors.pop();
            }
            simple_cap_face_ranges.push(capped.faces.len()..next.faces.len());
            capped = next;
            count = count
                .checked_add(1)
                .ok_or(GeometryError::TooManyMeshFaces)?;
        }
        if !crease && count > 0 {
            capped = planar_cap::weld_cap_boundary(self, capped, tolerance)?;
        }
        if count > 0 && (!self.ngons.is_empty() || !triangles) {
            let mut ngons = Vec::with_capacity(self.ngons.len() + simple_cap_face_ranges.len());
            for original in &self.ngons {
                let ngon = if crease {
                    original.clone()
                } else {
                    capped
                        .ngon_from_faces(original.faces.clone())
                        .ok_or(GeometryError::InvalidMeshNgon { ngon: ngons.len() })?
                };
                ngons.push(ngon);
            }
            if !triangles {
                for range in simple_cap_face_ranges {
                    let faces = range
                        .map(|index| {
                            u32::try_from(index).map_err(|_| GeometryError::TooManyMeshFaces)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    ngons.push(
                        capped
                            .ngon_from_faces(faces)
                            .ok_or(GeometryError::InvalidMeshNgon { ngon: ngons.len() })?,
                    );
                }
            }
            capped = capped.try_with_ngons(ngons)?;
        }
        Ok((capped, count))
    }
}

/// Prevent a simple disk fallback when another coplanar boundary overlaps its
/// bounds. The annular path handles proven nested loops first; ambiguous cases
/// stay open instead of creating overlapping faces.
fn coplanar_mesh_boundary_overlaps(
    data: &MeshTopologyData,
    boundary: &[Point3],
    plane: &PointProjection3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let naked = data
        .edges
        .iter()
        .filter(|(_, incidence)| incidence.count == 1)
        .map(|(&edge, _)| edge)
        .collect::<Vec<_>>();
    if naked.len() == boundary.len() {
        return Ok(false);
    }
    let mut parents = (0..data.topological_vertex_count).collect::<Vec<_>>();
    let mut naked_vertices = BTreeSet::new();
    for (first, second) in naked {
        union_indices_keep_earlier(&mut parents, first, second);
        naked_vertices.insert(first);
        naked_vertices.insert(second);
    }
    let source_vertex = data
        .topological_points
        .iter()
        .position(|point| *point == boundary[0])
        .expect("a filled boundary uses mesh topology vertices");
    let source_root = index_root(&mut parents, source_vertex);
    let mut other_components = BTreeMap::<usize, Vec<Point3>>::new();
    for vertex in naked_vertices {
        let root = index_root(&mut parents, vertex);
        if root != source_root {
            other_components
                .entry(root)
                .or_default()
                .push(data.topological_points[vertex]);
        }
    }
    let source_bounds = BoundingBox3::from_points(boundary.iter().copied())?;
    let source_min = source_bounds.min().to_array();
    let source_max = source_bounds.max().to_array();
    for other in other_components.values() {
        let bounds = BoundingBox3::from_points(other.iter().copied())?;
        let other_min = bounds.min().to_array();
        let other_max = bounds.max().to_array();
        if (0..3)
            .all(|axis| source_min[axis] <= other_max[axis] && other_min[axis] <= source_max[axis])
            && other.iter().copied().try_fold(true, |coplanar, point| {
                Ok::<_, GeometryError>(
                    coplanar && point.distance_to(plane.project(point)?)? <= tolerance.absolute(),
                )
            })?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod cap_tests;
#[cfg(test)]
mod tests;
