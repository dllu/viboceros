//! Assemble coplanar ruled walls by source incidence, even when edges overlap.
use super::incidence::count_edge_uses;
use super::*;
use std::collections::{HashMap, hash_map::Entry};

/// Output of extruding one planar face to an apex at or close to its plane.
pub enum CoplanarFaceExtrusion {
    Brep(Brep),
    ApexOnly,
}

impl Brep {
    /// Build an extrusion for a planar face and a coplanar or shallow apex.
    /// Coincident radial and source edges retain their distinct topology.
    /// Returns `None` for other source/apex configurations.
    pub fn try_extruded_face_to_coplanar_point(
        &self,
        apex: Point3,
        cap_base: bool,
        split_at_tangents: bool,
        tolerance: Tolerance,
    ) -> Result<Option<CoplanarFaceExtrusion>, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return Ok(None);
        };
        let Some(plane) = face.surface.plane(tolerance)? else {
            return Ok(None);
        };
        // SMLib's second stitching pass searches up to 40 model tolerances.
        // Assemble the topology explicitly within that distance so even very
        // shallow sweeps preserve vertices that are close and distinct.
        if plane.signed_distance_to(apex)?.abs() > 40.0 * tolerance.absolute() {
            return Ok(None);
        }
        let continuity = if split_at_tangents {
            viboceros_opennurbs::CurveContinuity::Gsmooth
        } else {
            viboceros_opennurbs::CurveContinuity::G1
        };
        let uses = self.edge_use_counts();
        if !uses.contains(&1) {
            return invalid("surface apex extrusion requires a boundary");
        }
        let mut splits = Vec::new();
        for (index, &count) in uses.iter().enumerate() {
            if count == 1 {
                let cuts = self.edges[index]
                    .curve
                    .discontinuity_parameters(continuity, 0.1_f64.to_radians())?;
                if !cuts.is_empty() {
                    splits.push((index, cuts));
                }
            }
        }
        let mut source = self.try_split_edges_at_parameters(&splits, tolerance)?;
        let base = source.faces.pop().expect("single source face");
        let uses = count_edge_uses(std::slice::from_ref(&base), source.edges.len());
        let mut faces = Vec::new();
        let mut remaining_controls = 500_000_usize;
        for boundary in &base.loops {
            let apex_index = source.vertices.len();
            source.vertices.push(BrepVertex::try_new(apex, 0.0)?);
            let mut radials = HashMap::new();
            for trim in &boundary.trims {
                let Some(index) = trim.edge.filter(|&edge| uses[edge] == 1) else {
                    continue;
                };
                let boundary_curve = &source.edges[index].curve;
                let [start, end] = source.edges[index].vertices;
                let domain = boundary_curve.domain();
                let endpoints = [
                    boundary_curve.evaluate(*domain.start())?,
                    boundary_curve.evaluate(*domain.end())?,
                ];
                // Rhino omits walls whose boundary endpoint is the apex,
                // including closed profiles targeted at their seam.
                if endpoints.contains(&apex)
                    || [start, end]
                        .iter()
                        .any(|&v| source.vertices[v].point == apex)
                {
                    continue;
                }
                remaining_controls = remaining_controls
                    .checked_sub(boundary_curve.control_points().len())
                    .ok_or(GeometryError::InvalidBrepTopology {
                        context: "surface apex wall control-net limit",
                    })?;
                if faces.len() >= 100_000 - usize::from(cap_base) {
                    return invalid("surface apex face count limit");
                }
                let surface = NurbsSurface::try_extruded_curve_to_point(boundary_curve, apex)?;
                let mut radial = |vertex: usize, point| -> Result<usize, GeometryError> {
                    let base = source.vertices[vertex].point;
                    let gap = base.distance_to(point)?;
                    match radials.entry(vertex) {
                        Entry::Occupied(edge) => {
                            let index: usize = *edge.get();
                            source.edges[index].tolerance = source.edges[index].tolerance.max(gap);
                            Ok(index)
                        }
                        Entry::Vacant(edge) => {
                            let index = source.edges.len();
                            source.edges.push(BrepEdge::try_new(
                                [vertex, apex_index],
                                NurbsCurve::try_new(1, vec![base, apex], vec![0.0, 0.0, 1.0, 1.0])?,
                                gap,
                            )?);
                            Ok(*edge.insert(index))
                        }
                    }
                };
                let from_start = radial(start, endpoints[0])?;
                let from_end = radial(end, endpoints[1])?;
                let wall = rectangular_surface_loop(
                    &surface,
                    [
                        RectangularTrimSpec::edge(
                            [start, apex_index],
                            from_start,
                            false,
                            BrepTrimType::Boundary,
                        ),
                        RectangularTrimSpec::singular(apex_index),
                        RectangularTrimSpec::edge(
                            [apex_index, end],
                            from_end,
                            true,
                            BrepTrimType::Boundary,
                        ),
                        RectangularTrimSpec::edge(
                            [end, start],
                            index,
                            true,
                            BrepTrimType::Boundary,
                        ),
                    ],
                )?;
                faces.push(BrepFace::try_new(
                    surface,
                    trim.reversed_3d ^ base.reversed,
                    vec![wall],
                )?);
            }
        }
        if cap_base {
            faces.insert(0, base);
        } else if faces.is_empty() {
            return Ok(Some(CoplanarFaceExtrusion::ApexOnly));
        }
        let subset = source.compact_face_topology(faces);
        Ok(Some(CoplanarFaceExtrusion::Brep(Self::try_new(
            subset.vertices,
            subset.edges,
            subset.faces,
            tolerance,
        )?)))
    }
}
