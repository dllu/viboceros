//! Exact affine support and retained boundary loops for planar faces.
use super::*;

impl Brep {
    /// Constructs one exact trimmed planar face from a closed NURBS boundary.
    ///
    /// The source curve remains the sole 3D boundary edge. Its exact rational
    /// projection becomes a counterclockwise p-curve on an affine plane, so
    /// concave and curved boundaries are retained without filling the plane's
    /// unused rectangular parameter domain.
    pub fn try_planar_face(
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_planar_face_with_holes(curve, &[], tolerance)
    }

    /// Constructs one exact trimmed planar face with zero or more inner loops.
    ///
    /// Every supplied NURBS curve is retained as a distinct shared-topology
    /// boundary edge. Inner curves are projected into the outer curve's exact
    /// affine plane and oriented clockwise in parameter space. Their standard
    /// sampled trim representation is validated as disjoint holes inside the
    /// counterclockwise outer loop before topology is committed.
    pub fn try_planar_face_with_holes(
        outer: &NurbsCurve,
        inner: &[NurbsCurve],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::planar_face_with_holes(outer, inner, tolerance, false)
    }

    /// Preserves independently admitted inner loops, including crossing siblings.
    /// Each complete hole must fit inside the outer loop. Model area subtracts
    /// every original loop; tessellation excludes their geometric union.
    pub fn try_planar_face_with_overlapping_holes(
        outer: &NurbsCurve,
        inner: &[NurbsCurve],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::planar_face_with_holes(outer, inner, tolerance, true)
    }

    fn planar_face_with_holes(
        outer: &NurbsCurve,
        inner: &[NurbsCurve],
        tolerance: Tolerance,
        overlapping: bool,
    ) -> Result<Self, GeometryError> {
        if !outer.is_closed()? || !outer.is_planar(tolerance)? {
            return Err(GeometryError::InvalidPlanarFaceBoundary);
        }

        let projection = project_planar_curve(outer, tolerance)
            .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
        let mut surface_bounds = projection.coordinate_bounds;
        let mut projected = Vec::with_capacity(inner.len() + 1);
        projected.push((
            outer,
            projection.curve,
            projection.maximum_residual,
            BrepLoopType::Outer,
        ));
        for curve in inner {
            if !curve.is_closed()? {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
            let (parameter_curve, maximum_residual) =
                project_curve_to_frame(curve, projection.frame, tolerance)
                    .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
            for control in parameter_curve.control_points() {
                let point = control.point();
                surface_bounds[0][0] = surface_bounds[0][0].min(point.x());
                surface_bounds[0][1] = surface_bounds[0][1].max(point.x());
                surface_bounds[1][0] = surface_bounds[1][0].min(point.y());
                surface_bounds[1][1] = surface_bounds[1][1].max(point.y());
            }
            projected.push((
                curve,
                parameter_curve,
                maximum_residual,
                BrepLoopType::Inner,
            ));
        }
        // Projected rational trims can evaluate a few ULPs beyond a control
        // coordinate extremum. Give the supporting surface a small parameter
        // margin so exact trim endpoints stay inside its active domain.
        for bounds in &mut surface_bounds {
            for _ in 0..8 {
                bounds[0] = bounds[0].next_down();
                bounds[1] = bounds[1].next_up();
            }
        }
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let surface = planar_cap_surface(projection.frame, zero, surface_bounds)?;
        let mut vertices = Vec::with_capacity(projected.len());
        let mut edges = Vec::with_capacity(projected.len());
        let mut loops = Vec::with_capacity(projected.len());
        for (index, (curve, parameter_curve, maximum_residual, loop_type)) in
            projected.into_iter().enumerate()
        {
            let (mut parameter_curve, mut curve_reversed) = oriented_cap_curve(parameter_curve)
                .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
            if loop_type == BrepLoopType::Inner {
                parameter_curve = parameter_curve.reversed()?;
                curve_reversed = !curve_reversed;
            }
            let seam = curve.evaluate(*curve.domain().start())?;
            let closure_tolerance = cap_closure_tolerance(
                seam,
                curve,
                &surface,
                &parameter_curve,
                curve_reversed,
                maximum_residual,
            )
            .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
            vertices.push(BrepVertex::try_new(seam, closure_tolerance)?);
            edges.push(BrepEdge::try_new(
                [index, index],
                curve.clone(),
                closure_tolerance,
            )?);
            loops.push(single_edge_loop(
                index,
                index,
                loop_type,
                parameter_curve,
                curve_reversed,
                BrepTrimType::Boundary,
                [closure_tolerance, closure_tolerance],
            )?);
        }
        if loops.len() > 1 {
            let sampled = loops
                .iter()
                .map(|face_loop| sample_trim_loop(face_loop, LOOP_SAMPLES_PER_SPAN))
                .collect::<Result<Vec<_>, _>>()?;
            if overlapping {
                for hole in &sampled[1..] {
                    let lengths = [sampled[0].len(), hole.len()];
                    let parameters = sampled[0].iter().chain(hole).copied().collect::<Vec<_>>();
                    if triangulate_trim_region(&parameters, &lengths)?.is_none() {
                        return Err(GeometryError::InvalidPlanarFaceBoundary);
                    }
                }
            } else {
                let lengths = sampled.iter().map(Vec::len).collect::<Vec<_>>();
                let parameters = sampled.into_iter().flatten().collect::<Vec<_>>();
                if triangulate_trim_region(&parameters, &lengths)?.is_none() {
                    return Err(GeometryError::InvalidPlanarFaceBoundary);
                }
            }
        }
        let faces = vec![BrepFace::try_new(surface, false, loops)?];
        Self::try_new(vertices, edges, faces, tolerance)
    }
}
