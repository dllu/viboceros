//! Natural borders, topological edges, and density-selected surface wires.
use super::*;

#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Extracts every non-degenerate natural border as exact NURBS curves.
    ///
    /// Each inner vector is one connected border. A rectangular open patch
    /// therefore has four perimeter-ordered curves, a cylinder has two
    /// one-curve circular borders, and a surface closed in both directions has
    /// none. Singular collapsed sides, such as a cone apex or sphere pole, are
    /// omitted because they are points rather than curve borders. Output
    /// domains use the same local frame as [`Self::natural_edge_curves`].
    pub fn natural_boundary_curve_loops(
        &self,
    ) -> Result<Vec<Vec<crate::NurbsCurve>>, GeometryError> {
        self.local_parameter_frame(std::iter::empty())?
            .surface
            .natural_boundary_curve_loops_in_frame()
    }

    fn natural_boundary_curve_loops_in_frame(&self) -> Result<Vec<Vec<NurbsCurve>>, GeometryError> {
        let closed_u = self.is_closed_u()?;
        let closed_v = self.is_closed_v()?;
        if closed_u && closed_v {
            return Ok(Vec::new());
        }

        let u_domain = self.domain_u();
        let v_domain = self.domain_v();
        if !closed_u && !closed_v {
            let candidates = [
                self.isocurve_u(*v_domain.start())?,
                self.isocurve_v(*u_domain.end())?,
                self.isocurve_u(*v_domain.end())?.reversed()?,
                self.isocurve_v(*u_domain.start())?.reversed()?,
            ];
            let perimeter = candidates
                .into_iter()
                .filter(curve_has_extent)
                .collect::<Vec<_>>();
            return Ok((!perimeter.is_empty())
                .then_some(perimeter)
                .into_iter()
                .collect());
        }

        let candidates = if closed_u {
            vec![
                self.isocurve_u(*v_domain.start())?,
                self.isocurve_u(*v_domain.end())?.reversed()?,
            ]
        } else {
            vec![
                self.isocurve_v(*u_domain.end())?,
                self.isocurve_v(*u_domain.start())?.reversed()?,
            ]
        };
        Ok(candidates
            .into_iter()
            .filter(curve_has_extent)
            .map(|curve| vec![curve])
            .collect())
    }

    /// Non-degenerate U-isocurves at every density-selected V station,
    /// including natural boundaries. Stations and output curve domains use a
    /// lossless local parameter frame, as in [`Self::wireframe_curves`].
    pub fn isocurves_u_at_density(
        &self,
        wire_density: i32,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.isocurves_at_density(0, wire_density)
    }

    /// V-direction counterpart of [`Self::isocurves_u_at_density`].
    pub fn isocurves_v_at_density(
        &self,
        wire_density: i32,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        self.isocurves_at_density(1, wire_density)
    }

    fn isocurves_at_density(
        &self,
        varying: usize,
        wire_density: i32,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let frame = self.local_parameter_frame(std::iter::empty())?;
        let surface = &frame.surface;
        let parameters = if varying == 0 {
            surface.wire_parameters_v(wire_density)?
        } else {
            surface.wire_parameters_u(wire_density)?
        };
        let mut curves = Vec::new();
        for fixed in parameters {
            let curve = if varying == 0 {
                surface.isocurve_u(fixed)?
            } else {
                surface.isocurve_v(fixed)?
            };
            push_surface_wire(&mut curves, curve)?;
        }
        Ok(curves)
    }

    /// Returns all U parameters used by an OpenNURBS-compatible wireframe.
    /// Natural boundaries are included even when they form a closed seam.
    pub fn wire_parameters_u(&self, wire_density: i32) -> Result<Vec<Real>, GeometryError> {
        surface_wire_parameters(self.spans_u(), wire_density)
    }

    /// Returns all V parameters used by an OpenNURBS-compatible wireframe.
    /// Natural boundaries are included even when they form a closed seam.
    pub fn wire_parameters_v(&self, wire_density: i32) -> Result<Vec<Real>, GeometryError> {
        surface_wire_parameters(self.spans_v(), wire_density)
    }

    /// Returns the standalone surface's exact topological edges in
    /// OpenNURBS order.
    ///
    /// Closed directions contribute one seam rather than two coincident
    /// sides, while collapsed singular sides are omitted. Consequently an
    /// open patch has four edges, a cylinder has two rims and one seam, a
    /// sphere has one seam, and a torus has two seams.
    /// Curves use losslessly translated local parameter domains, avoiding
    /// coarse native parameter grids when sampling their model-space geometry.
    pub fn natural_edge_curves(&self) -> Result<Vec<crate::NurbsCurve>, GeometryError> {
        self.local_parameter_frame(std::iter::empty())?
            .surface
            .natural_edge_curves_in_frame()
    }

    fn natural_edge_curves_in_frame(&self) -> Result<Vec<crate::NurbsCurve>, GeometryError> {
        let closed_u = self.is_closed_u()?;
        let closed_v = self.is_closed_v()?;
        let u_start = *self.domain_u().start();
        let u_end = *self.domain_u().end();
        let v_start = *self.domain_v().start();
        let v_end = *self.domain_v().end();
        let mut curves = Vec::new();

        match (closed_u, closed_v) {
            (false, false) => {
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_end)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_end)?.reversed()?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?.reversed()?)?;
            }
            (true, false) => {
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_end)?.reversed()?)?;
            }
            (false, true) => {
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_v(u_end)?.reversed()?)?;
            }
            (true, true) => {
                push_surface_wire(&mut curves, self.isocurve_v(u_start)?)?;
                push_surface_wire(&mut curves, self.isocurve_u(v_start)?)?;
            }
        }

        Ok(curves)
    }

    /// Returns the exact topological boundaries, seams, and interior
    /// isoparametric curves displayed for this standalone surface.
    /// Generated stations and curve domains stay in the lossless local frame.
    pub fn wireframe_curves(
        &self,
        wire_density: i32,
    ) -> Result<Vec<crate::NurbsCurve>, GeometryError> {
        let frame = self.local_parameter_frame(std::iter::empty())?;
        let surface = &frame.surface;
        let parameters_u = surface.wire_parameters_u(wire_density)?;
        let parameters_v = surface.wire_parameters_v(wire_density)?;
        let mut curves = surface.natural_edge_curves_in_frame()?;

        for v in interior_wire_parameters(&parameters_v) {
            push_surface_wire(&mut curves, surface.isocurve_u(v)?)?;
        }
        for u in interior_wire_parameters(&parameters_u) {
            push_surface_wire(&mut curves, surface.isocurve_v(u)?)?;
        }
        Ok(curves)
    }
}

fn curve_has_extent(curve: &crate::NurbsCurve) -> bool {
    let first = curve.control_points()[0].point();
    curve
        .control_points()
        .iter()
        .any(|control| control.point() != first)
}

fn surface_wire_parameters(
    spans: impl Iterator<Item = (Real, Real)>,
    wire_density: i32,
) -> Result<Vec<Real>, GeometryError> {
    if !(crate::MIN_SURFACE_WIRE_DENSITY..=crate::MAX_SURFACE_WIRE_DENSITY).contains(&wire_density)
    {
        return Err(GeometryError::InvalidSurfaceWireDensity(wire_density));
    }

    // Rhino/OpenNURBS density -1 draws only natural boundaries, 0 adds knot
    // wires, 1 adds one midpoint only when there are no interior knots, and
    // N >= 2 adds N-1 evenly spaced wires inside every nonempty knot span.
    let spans = spans.collect::<Vec<_>>();
    let first = spans
        .first()
        .copied()
        .expect("a validated NURBS direction has a nonempty span");
    let last = spans
        .last()
        .copied()
        .expect("a validated NURBS direction has a nonempty span");
    if wire_density < 0 {
        return Ok(vec![first.0, last.1]);
    }
    let extra_per_span = match wire_density {
        1 if spans.len() == 1 => 1,
        2.. => (wire_density - 1) as usize,
        _ => 0,
    };
    let parameter_count = spans
        .len()
        .checked_mul(extra_per_span + 1)
        .and_then(|count| count.checked_add(1))
        .filter(|&count| count <= crate::MAX_SURFACE_WIRES)
        .ok_or(GeometryError::TooManySurfaceWires)?;
    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(parameter_count)
        .map_err(|_| GeometryError::TooManySurfaceWires)?;
    parameters.push(first.0);
    for (start, end) in spans {
        for division in 1..=extra_per_span {
            let fraction = division as Real / (extra_per_span + 1) as Real;
            let parameter = start.mul_add(1.0 - fraction, end * fraction);
            if parameter > start && parameter < end {
                parameters.push(parameter);
            }
        }
        parameters.push(end);
    }
    Ok(parameters)
}

fn interior_wire_parameters(parameters: &[Real]) -> impl Iterator<Item = Real> + '_ {
    parameters
        .iter()
        .copied()
        .skip(1)
        .take(parameters.len().saturating_sub(2))
}

fn push_surface_wire(
    curves: &mut Vec<crate::NurbsCurve>,
    curve: crate::NurbsCurve,
) -> Result<(), GeometryError> {
    if !curve_has_extent(&curve) {
        return Ok(());
    }
    if curves.len() == crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }
    curves.push(curve);
    Ok(())
}
