//! Naked mesh sides must belong to true naked edges on their source B-rep face.

use super::*;

struct NakedEdgeSamples {
    exact: BTreeSet<[u64; 3]>,
    chords: Vec<LineSegment>,
    bounds: Option<BoundingBox3>,
    envelope_tolerance: Real,
    linear_complete: bool,
}

impl NakedEdgeSamples {
    fn may_contain(&self, point: Point3) -> bool {
        let Some(bounds) = self.bounds else {
            return true;
        };
        let min = bounds.min().to_array();
        let max = bounds.max().to_array();
        point.to_array().into_iter().enumerate().all(|(i, x)| {
            x >= min[i] - self.envelope_tolerance && x <= max[i] + self.envelope_tolerance
        })
    }
}

fn key(point: Point3) -> [u64; 3] {
    point
        .to_array()
        .map(|x| if x == 0.0 { 0 } else { x.to_bits() })
}

impl Brep {
    pub(in crate::brep) fn mesh_boundary_conforms(
        &self,
        mesh: &TriangleMesh,
        face_sources: &[usize],
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<bool, GeometryError> {
        if mesh.faces().len() != face_sources.len()
            || face_sources.iter().any(|&i| i >= self.faces.len())
        {
            return invalid("mesh boundary audit requires a source for every polygon face");
        }
        let mut uses = vec![(0_usize, 0_usize); self.edges.len()];
        for face in &self.faces {
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                if let Some(edge) = trim.edge {
                    uses[edge].0 += 1;
                    uses[edge].1 += usize::from(trim.reversed_3d ^ face.reversed);
                }
            }
        }
        let (topology, boundary) = mesh.topology_with_boundary();
        let closed = uses.iter().all(|u| u.0 == 2);
        let manifold = uses.iter().all(|u| u.0 <= 2);
        let oriented = manifold && uses.iter().all(|u| u.0 != 2 || u.1 == 1);
        if (closed && !topology.is_closed())
            || (manifold && !topology.is_manifold())
            || (oriented && !topology.is_oriented())
        {
            return Ok(false);
        }
        let naked_by_face = self
            .faces
            .iter()
            .map(|face| {
                face.loops
                    .iter()
                    .flat_map(|l| &l.trims)
                    .filter_map(|t| t.edge)
                    .filter(|&e| uses[e].0 == 1)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut edge_samples = (0..self.edges.len()).map(|_| None).collect::<Vec<_>>();
        for face in &self.faces {
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                let Some(e) = trim.edge.filter(|&e| uses[e].0 == 1) else {
                    continue;
                };
                let curve = &self.edges[e].curve;
                let sign = curve.control_points()[0].weight().is_sign_positive();
                let bounds = if curve
                    .control_points()
                    .iter()
                    .all(|c| c.weight().is_sign_positive() == sign)
                {
                    Some(BoundingBox3::from_points(
                        curve
                            .control_points()
                            .iter()
                            .map(|c| c.point())
                            .chain(self.edges[e].vertices.map(|v| self.vertices[v].point)),
                    )?)
                } else {
                    None
                };
                let points = self.trim_snap_points(trim, samples_per_span, tolerance)?;
                let mut chords = Vec::new();
                let chord_epsilon = (tolerance.absolute().max(self.edges[e].tolerance) * 0.01)
                    .max(Real::MIN_POSITIVE);
                let mut add_chord = |a: Point3, b: Point3| -> Result<bool, GeometryError> {
                    // This is an absolute model-space check. Applying a
                    // relative tolerance to world coordinates can discard a
                    // perfectly representable short segment far from origin.
                    if a.distance_to(b)? <= chord_epsilon {
                        Ok(false)
                    } else {
                        chords.push(LineSegment::from_validated(a, b, [0.0, 1.0]));
                        Ok(true)
                    }
                };
                for pair in points.windows(2) {
                    add_chord(pair[0].point, pair[1].point)?;
                }
                let mut linear_complete = curve.degree() == 1 && bounds.is_some();
                if linear_complete {
                    // Topological endpoint vertices may differ from the exact
                    // curve within their recorded tolerances. Keep the actual
                    // span loci too, not only vertex-snapped mesh chords.
                    for (start, end) in curve.spans() {
                        linear_complete &= add_chord(
                            curve.evaluate_on_side(start, crate::ParameterSide::Right)?,
                            curve.evaluate_on_side(end, crate::ParameterSide::Left)?,
                        )?;
                    }
                }
                edge_samples[e] = Some(NakedEdgeSamples {
                    exact: points.into_iter().map(|p| key(p.point)).collect(),
                    chords,
                    bounds,
                    linear_complete,
                    envelope_tolerance: tolerance
                        .absolute()
                        .max(self.edges[e].tolerance)
                        .max(self.vertices[trim.vertices[0]].tolerance)
                        .max(self.vertices[trim.vertices[1]].tolerance),
                });
            }
        }
        let mut matched = vec![false; self.edges.len()];
        // Adjacent boundary sides reuse vertices. Cache each point/edge query
        // without sharing membership between unrelated source faces.
        let mut membership = BTreeMap::<(u32, usize), bool>::new();
        for side in boundary {
            let source =
                *face_sources
                    .get(side.face)
                    .ok_or(GeometryError::InvalidBrepTopology {
                        context: "a naked mesh side has no source B-rep face",
                    })?;
            let candidates =
                naked_by_face
                    .get(source)
                    .ok_or(GeometryError::InvalidBrepTopology {
                        context: "a mesh polygon references a missing source B-rep face",
                    })?;
            let mut found = false;
            for &edge_index in candidates {
                let samples = edge_samples[edge_index]
                    .as_ref()
                    .expect("naked edges have sampling data");
                if side
                    .vertices
                    .iter()
                    .any(|&i| !samples.may_contain(mesh.vertices()[i as usize]))
                {
                    continue;
                }
                let mut contains = true;
                for vertex in side.vertices {
                    let near = if let Some(&near) = membership.get(&(vertex, edge_index)) {
                        near
                    } else {
                        let near = self.mesh_point_on_edge(
                            mesh.vertices()[vertex as usize],
                            edge_index,
                            samples,
                            tolerance,
                        )?;
                        membership.insert((vertex, edge_index), near);
                        near
                    };
                    if !near {
                        contains = false;
                        break;
                    }
                }
                if contains {
                    matched[edge_index] = true;
                    found = true;
                    break;
                }
            }
            if !found {
                return Ok(false);
            }
        }
        // A missing hole or boundary component must not become acceptable just
        // because no unexpected naked side remains in the mesh.
        Ok(uses.iter().enumerate().all(|(e, u)| u.0 != 1 || matched[e]))
    }

    fn mesh_point_on_edge(
        &self,
        point: Point3,
        index: usize,
        samples: &NakedEdgeSamples,
        tolerance: Tolerance,
    ) -> Result<bool, GeometryError> {
        if samples.exact.contains(&key(point)) {
            return Ok(true);
        }
        let edge = &self.edges[index];
        let allowed = tolerance.absolute().max(edge.tolerance);
        for &vertex in &edge.vertices {
            let vertex = self.vertices[vertex];
            if point.distance_to(vertex.point)? <= allowed.max(vertex.tolerance) {
                return Ok(true);
            }
        }
        if let Some(bounds) = samples.bounds {
            let min = bounds.min().to_array();
            let max = bounds.max().to_array();
            if point
                .to_array()
                .into_iter()
                .enumerate()
                .any(|(i, x)| x < min[i] - allowed || x > max[i] + allowed)
            {
                return Ok(false);
            }
        }
        let search = Tolerance::try_new(
            (allowed * 0.01).max(Real::MIN_POSITIVE),
            tolerance.relative(),
            tolerance.angular(),
        )?;
        // A constrained triangulation may split a sampled boundary chord at
        // another boundary point (notably tangent loops). Such a point need
        // not lie on the exact curved edge, but must remain on its mesh chord.
        for &chord in &samples.chords {
            if point.distance_to(chord.closest_point(point, search)?)? <= allowed {
                return Ok(true);
            }
        }
        // Every exact same-sign degree-one span was covered above.
        if samples.linear_complete {
            return Ok(false);
        }
        let parameter = edge.curve.closest_parameter(point, search)?;
        Ok(point.distance_to(edge.curve.evaluate(parameter)?)? <= allowed)
    }
}
