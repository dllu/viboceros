//! Assemble coplanar ruled walls by source incidence, even when edges overlap.
use super::incidence::count_edge_uses;
use super::*;
use crate::Plane;
use std::collections::{HashMap, hash_map::Entry};

mod planar_boundary;

/// Output of extruding one planar face to an apex at or close to its plane.
pub enum CoplanarFaceExtrusion {
    Brep(Brep),
    ApexOnly,
}

impl NurbsCurve {
    /// Boundary pieces used by surface-to-point extrusion. Retains the
    /// original seam as an endpoint and splits only at selected geometric
    /// continuity breaks.
    pub fn surface_apex_wall_segments(
        &self,
        split_at_tangents: bool,
    ) -> Result<Vec<Self>, GeometryError> {
        let breaks = self.surface_apex_wall_breaks(split_at_tangents)?;
        let domain = self.domain();
        let mut stops = Vec::with_capacity(breaks.len() + 2);
        stops.push(*domain.start());
        stops.extend(breaks);
        stops.push(*domain.end());
        let mut parts = Vec::with_capacity(stops.len() - 1);
        super::command_apex::partition(self.clone(), &stops, &Self::try_trimmed, &mut parts)?;
        Ok(parts)
    }

    fn surface_apex_wall_breaks(
        &self,
        split_at_tangents: bool,
    ) -> Result<Vec<Real>, GeometryError> {
        if self.spans().nth(1).is_none() {
            return Ok(Vec::new());
        }
        let continuity = if split_at_tangents {
            viboceros_opennurbs::CurveContinuity::Gsmooth
        } else {
            viboceros_opennurbs::CurveContinuity::G1
        };
        self.discontinuity_parameters(continuity, 0.1_f64.to_radians())
    }
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
        if self.shallow_apex_plane(apex, tolerance)?.is_none() {
            return Ok(None);
        }
        let mut source = self.with_apex_wall_splits(split_at_tangents, tolerance)?;
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

    fn shallow_apex_plane(
        &self,
        apex: Point3,
        tolerance: Tolerance,
    ) -> Result<Option<(Plane, Real)>, GeometryError> {
        let [face] = self.faces.as_slice() else {
            return Ok(None);
        };
        let Some(plane) = face.surface.plane(tolerance)? else {
            return Ok(None);
        };
        let height = plane.signed_distance_to(apex)?;
        // SMLib's second stitching pass searches up to 40 model tolerances.
        // Explicit incidence preserves distinct vertices within that distance.
        Ok((height.abs() <= 40.0 * tolerance.absolute()).then_some((plane, height)))
    }

    fn with_apex_wall_splits(
        &self,
        split_at_tangents: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let uses = self.edge_use_counts();
        if !uses.contains(&1) {
            return invalid("surface apex extrusion requires a boundary");
        }
        let mut splits = Vec::new();
        for (index, &count) in uses.iter().enumerate() {
            if count == 1 {
                let cuts = self.edges[index]
                    .curve
                    .surface_apex_wall_breaks(split_at_tangents)?;
                if !cuts.is_empty() {
                    splits.push((index, cuts));
                }
            }
        }
        self.try_split_edges_at_parameters(&splits, tolerance)
    }
}
