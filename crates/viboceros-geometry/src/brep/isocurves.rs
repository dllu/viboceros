//! Trimmed isocurve extraction and display wires in a local UV frame.
use super::parameter_frame::ParameterFrame;
use super::*;

#[cfg(test)]
mod tests;

impl BrepFace {
    /// Returns the underlying U-isocurve portions inside the trim region.
    /// `v` is a native surface parameter. Output curve domains retain native
    /// U coordinates when their knots can be restored exactly; otherwise they
    /// retain a translated local domain rather than rounding the trim geometry.
    pub fn isocurve_u_segments(
        &self,
        v: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.isocurve_segments(0, v, tolerance)
    }

    /// Returns the underlying V-isocurve portions inside the trim region.
    /// `u` is native; curve domains follow [`Self::isocurve_u_segments`].
    pub fn isocurve_v_segments(
        &self,
        u: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.isocurve_segments(1, u, tolerance)
    }

    /// Extracts trimmed U-isocurves at all density-selected V stations,
    /// including the natural boundaries. Stations are generated in the local
    /// UV frame; output domains follow [`Self::isocurve_u_segments`].
    pub fn isocurve_u_segments_at_density(
        &self,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.isocurve_segments_at_density(0, wire_density, tolerance)
    }

    /// V-direction counterpart of [`Self::isocurve_u_segments_at_density`].
    pub fn isocurve_v_segments_at_density(
        &self,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.isocurve_segments_at_density(1, wire_density, tolerance)
    }

    fn isocurve_segments_at_density(
        &self,
        varying: usize,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let frame = self.local_parameter_frame()?;
        let parameters = if varying == 0 {
            frame.face.surface.wire_parameters_v(wire_density)?
        } else {
            frame.face.surface.wire_parameters_u(wire_density)?
        };
        let mut curves = Vec::new();
        for fixed in parameters {
            for curve in frame.isocurve_segments(varying, fixed, true, tolerance)? {
                push_brep_wire(&mut curves, curve)?;
            }
        }
        Ok(curves)
    }

    fn isocurve_segments(
        &self,
        varying: usize,
        fixed: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let domain = if varying == 0 {
            self.surface.domain_v()
        } else {
            self.surface.domain_u()
        };
        // Errors retain the caller's native coordinate and native domain.
        crate::parameter::checked_parameter(fixed, domain)?;
        let frame = self.local_parameter_frame()?;
        frame.isocurve_segments(varying, fixed - frame.origin[1 - varying], true, tolerance)
    }
}

impl ParameterFrame<'_> {
    fn isocurve_segments(
        &self,
        varying: usize,
        fixed: Real,
        restore_domain: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let curve = if varying == 0 {
            self.face.surface.isocurve_u(fixed)?
        } else {
            self.face.surface.isocurve_v(fixed)?
        };
        let intervals = trimmed_isocurve_intervals(&self.face, varying, fixed, tolerance)?;
        let curves = trim_isocurve_to_intervals(curve, intervals)?;
        if restore_domain {
            curves
                .into_iter()
                .map(|curve| self.restore_curve_parameter_origin(curve, varying))
                .collect()
        } else {
            Ok(curves)
        }
    }
}

impl Brep {
    /// Returns every topological edge once, followed by trimmed interior
    /// isocurves selected by the OpenNURBS wire-density rules. Display-only
    /// isocurves keep local domains so later sampling cannot round their
    /// parameters back onto a coarse large-origin UV grid.
    pub fn wireframe_curves(
        &self,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.faces[0].surface().wire_parameters_u(wire_density)?;
        if self.edges.len() > crate::MAX_SURFACE_WIRES {
            return Err(GeometryError::TooManySurfaceWires);
        }
        let mut curves = Vec::new();
        curves
            .try_reserve_exact(self.edges.len())
            .map_err(|_| GeometryError::TooManySurfaceWires)?;
        curves.extend(self.edges.iter().map(|edge| edge.curve().clone()));
        for face in &self.faces {
            let frame = face.local_parameter_frame()?;
            for (varying, parameters) in [
                (0, frame.face.surface.wire_parameters_v(wire_density)?),
                (1, frame.face.surface.wire_parameters_u(wire_density)?),
            ] {
                let interior_count = parameters.len().saturating_sub(2);
                for fixed in parameters.into_iter().skip(1).take(interior_count) {
                    for curve in frame.isocurve_segments(varying, fixed, false, tolerance)? {
                        push_brep_wire(&mut curves, curve)?;
                    }
                }
            }
        }
        Ok(curves)
    }
}
