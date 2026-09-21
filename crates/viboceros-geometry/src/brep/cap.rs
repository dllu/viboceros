//! Planar hole capping with exact source edges and projected rational trims.
use super::*;
mod boundary;
#[cfg(test)]
mod tests;

const MAX_CAP_LOOPS: usize = 1024;
const MAX_CAP_CONTROLS: usize = 1_000_000;

struct WorkBudget(usize);
impl WorkBudget {
    fn charge(&mut self, count: usize) -> Result<(), GeometryError> {
        self.0 = self
            .0
            .checked_sub(count)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "planar capping exceeds its boundary/control budget",
            })?;
        Ok(())
    }
}

struct ProjectedLoop {
    boundary: BrepLoop,
    reversed: bool,
    bounds: [[Real; 2]; 2],
}

struct CapBoundary {
    edges: Vec<(usize, bool)>,
    frame: Frame3,
    area: Real,
    projected: ProjectedLoop,
}

impl Brep {
    /// Adds planar faces to closed, consistently oriented naked-edge cycles.
    /// Source vertices, spatial edges and underlying surfaces are retained.
    /// Coplanar nested cycles form holes, not overlapping filled disks. Planarity
    /// is conservatively checked on control points; containment uses sampled
    /// trim-region validation, not certified continuous curve intersections.
    /// Ambiguous boundaries and entirely planar B-reps are left alone.
    /// Limits: 1024 cycles and one million charged control/sample work units.
    /// Returns `None` when no boundary can be capped; inputs are never mutated.
    pub fn try_cap_planar_holes(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let cycles = boundary::cycles(self);
        if cycles.is_empty() {
            return Ok(None);
        }
        if cycles.len() > MAX_CAP_LOOPS {
            return cap_limit();
        }
        let mut boundaries = Vec::new();
        let mut budget = WorkBudget(MAX_CAP_CONTROLS);
        for edges in cycles {
            let points = edges
                .iter()
                .flat_map(|&(i, _)| self.edges[i].curve.control_points())
                .map(|p| p.point())
                .take(budget.0 + 1)
                .collect::<Vec<_>>();
            budget.charge(points.len())?;
            let Some(frame) = boundary::frame(&points, tolerance)? else {
                continue;
            };
            if boundary::is_planar(self, frame, tolerance, &mut budget)? {
                continue;
            }
            let Some(projected) =
                self.cap_loop(&edges, frame, BrepLoopType::Outer, tolerance, &mut budget)?
            else {
                continue;
            };
            let area = log_sampled_area(&projected.boundary, &mut budget)?;
            boundaries.push(CapBoundary {
                edges,
                frame,
                area,
                projected,
            });
        }
        if boundaries.is_empty() {
            return Ok(None);
        }

        // Strict geometric containment, independent of native curve domains.
        // Area only orders possible parents; complete projected loops must pass
        // the trim-region validation before a containment relation is accepted.
        let mut order = (0..boundaries.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| {
            boundaries[a]
                .area
                .total_cmp(&boundaries[b].area)
                .then(a.cmp(&b))
        });
        let mut parents = vec![None; boundaries.len()];
        for child in 0..boundaries.len() {
            for &parent in &order {
                if boundaries[parent].area <= boundaries[child].area {
                    continue;
                }
                let outer = &boundaries[parent];
                let Some(inner) = self.cap_loop(
                    &boundaries[child].edges,
                    outer.frame,
                    BrepLoopType::Inner,
                    tolerance,
                    &mut budget,
                )?
                else {
                    continue;
                };
                if valid_region(&[&outer.projected.boundary, &inner.boundary], &mut budget)? {
                    parents[child] = Some(parent);
                    break;
                }
            }
        }
        let depth = |mut index: usize| {
            let mut count = 0usize;
            while let Some(parent) = parents[index] {
                count += 1;
                index = parent;
            }
            count
        };
        let mut added = Vec::new();
        for (index, boundary) in boundaries.iter().enumerate() {
            if !depth(index).is_multiple_of(2) {
                continue;
            }
            let ProjectedLoop {
                boundary: outer,
                reversed,
                bounds,
            } = &boundary.projected;
            let mut loops = vec![outer.clone()];
            for (child, parent) in parents.iter().enumerate() {
                if *parent != Some(index) {
                    continue;
                }
                let inner = self
                    .cap_loop(
                        &boundaries[child].edges,
                        boundary.frame,
                        BrepLoopType::Inner,
                        tolerance,
                        &mut budget,
                    )?
                    .expect("accepted coplanar child");
                if inner.reversed != *reversed {
                    return invalid("cap boundaries have inconsistent shell orientation");
                }
                loops.push(inner.boundary);
            }
            if !valid_region(&loops.iter().collect::<Vec<_>>(), &mut budget)? {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
            let surface =
                planar_cap_surface(boundary.frame, Vector3::try_new(0., 0., 0.)?, *bounds)?;
            added.push(BrepFace::try_new(surface, *reversed, loops)?);
        }
        if added.is_empty() {
            return Ok(None);
        }
        let mut faces = self.faces.clone();
        let mut capped_edges = vec![false; self.edges.len()];
        for face in &added {
            for trim in face.loops.iter().flat_map(|l| &l.trims) {
                capped_edges[trim.edge.expect("cap trim")] = true;
            }
        }
        for face in &mut faces {
            for trim in face.loops.iter_mut().flat_map(|l| &mut l.trims) {
                if trim.edge.is_some_and(|e| capped_edges[e]) {
                    trim.trim_type = BrepTrimType::Mated;
                }
            }
        }
        faces.extend(added);
        Self::try_new(self.vertices.clone(), self.edges.clone(), faces, tolerance).map(Some)
    }

    fn cap_loop(
        &self,
        edges: &[(usize, bool)],
        frame: Frame3,
        kind: BrepLoopType,
        tolerance: Tolerance,
        budget: &mut WorkBudget,
    ) -> Result<Option<ProjectedLoop>, GeometryError> {
        let mut trims = Vec::new();
        let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
        for &(index, reversed) in edges {
            let edge = &self.edges[index];
            budget.charge(edge.curve.control_points().len())?;
            let sign = edge.curve.control_points()[0].weight().is_sign_positive();
            if edge
                .curve
                .control_points()
                .iter()
                .any(|p| p.weight().is_sign_positive() != sign)
            {
                return invalid("planar capping requires same-sign rational weights");
            }
            let projected = project_curve_to_frame(&edge.curve, frame, tolerance);
            let (curve, _) = match projected {
                Err(GeometryError::InvalidPlanarFaceBoundary) => return Ok(None),
                result => result?,
            };
            for p in curve.control_points() {
                for (axis, coordinate) in [p.point().x(), p.point().y()].into_iter().enumerate() {
                    bounds[axis][0] = bounds[axis][0].min(coordinate);
                    bounds[axis][1] = bounds[axis][1].max(coordinate);
                }
            }
            trims.push(BrepTrim::try_new(
                oriented_edge_vertices(edge, reversed),
                Some(index),
                reversed,
                if reversed { curve.reversed()? } else { curve },
                BrepTrimType::Mated,
                SurfaceIso::NotIso,
                [0.; 2],
            )?);
        }
        let mut face_loop = BrepLoop::try_new(kind, trims)?;
        charge_samples(&face_loop, budget)?;
        let area = sampled_loop_signed_area(&face_loop)?;
        if area == 0.0 {
            return Ok(None);
        }
        let reverse = (area < 0.0) == (kind == BrepLoopType::Outer);
        if reverse {
            face_loop.trims.reverse();
            for trim in &mut face_loop.trims {
                trim.vertices.reverse();
                trim.reversed_3d = !trim.reversed_3d;
                trim.curve = trim.curve.reversed()?;
            }
        }
        Ok(Some(ProjectedLoop {
            boundary: face_loop,
            reversed: reverse,
            bounds,
        }))
    }
}

fn cap_limit<T>() -> Result<T, GeometryError> {
    invalid("planar capping exceeds its boundary/control budget")
}

fn valid_region(loops: &[&BrepLoop], budget: &mut WorkBudget) -> Result<bool, GeometryError> {
    // Bound sampling before allocation, including repeated containment trials.
    for boundary in loops {
        charge_samples(boundary, budget)?;
    }
    let sampled = loops
        .iter()
        .map(|l| sample_cap_loop(l))
        .collect::<Result<Vec<_>, _>>()?;
    let lengths = sampled.iter().map(Vec::len).collect::<Vec<_>>();
    let points = sampled.into_iter().flatten().collect::<Vec<_>>();
    Ok(triangulate_trim_region(&points, &lengths)?.is_some())
}

fn log_sampled_area(boundary: &BrepLoop, budget: &mut WorkBudget) -> Result<Real, GeometryError> {
    charge_samples(boundary, budget)?;
    let points = sample_cap_loop(boundary)?;
    let normalization = TrimParameterNormalization::try_from_points(&points)?
        .ok_or(GeometryError::InvalidPlanarFaceBoundary)?;
    let normalized = points
        .into_iter()
        .map(|p| normalization.normalize(p))
        .collect::<Result<Vec<_>, _>>()?;
    let mut sum = 0.;
    let mut correction = 0.;
    for (i, a) in normalized.iter().enumerate() {
        let b = normalized[(i + 1) % normalized.len()];
        neumaier_add(&mut sum, &mut correction, a[0].mul_add(b[1], -a[1] * b[0]));
    }
    let area = (sum + correction).abs();
    if area == 0. {
        return Err(GeometryError::InvalidPlanarFaceBoundary);
    }
    // The existing signed-area helper is normalized separately per loop: its
    // magnitude alone cannot order differently-sized, geometrically similar loops.
    Ok(area.ln() + normalization.log_area_scale())
}

fn charge_samples(boundary: &BrepLoop, budget: &mut WorkBudget) -> Result<(), GeometryError> {
    for trim in &boundary.trims {
        budget.charge(
            trim.curve
                .control_points()
                .len()
                .saturating_mul(LOOP_SAMPLES_PER_SPAN)
                .saturating_add(1),
        )?;
    }
    Ok(())
}

fn sample_cap_loop(boundary: &BrepLoop) -> Result<Vec<Point2>, GeometryError> {
    let mut points = Vec::new();
    for trim in &boundary.trims {
        // A same-sign rational degree-one span is exactly a line segment.
        // Artificial interior samples can form roundoff-thin triangles after
        // rotated-plane projection, incorrectly rejecting an ordinary rectangle.
        let count = if trim.curve.degree() == 1 {
            1
        } else {
            LOOP_SAMPLES_PER_SPAN
        };
        for (start, end) in trim.curve.spans() {
            if points.is_empty() {
                points.push(trim.curve.evaluate(start)?);
            }
            for sample in 1..=count {
                points.push(trim.curve.evaluate(normalized_span_parameter(
                    [start, end],
                    sample as Real / count as Real,
                )?)?);
            }
        }
    }
    if points.len() > 1 {
        points.pop();
    }
    Ok(points)
}
