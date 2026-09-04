//! Area and volume of exact B-rep faces, independent of display meshes.

use super::{
    Brep, BrepFace, PlanarSurfacePlane, centered_surface, face_covers_full_surface_domain,
    neumaier_add, normalized_span_parameter, planar_surface_plane, rectangular_face_trim_bounds,
};
use crate::{
    GeometryError, NurbsSurface, Real, Tolerance, Vector3, integration::integrate_adaptive,
    nurbs_surface::integrate_area_patch, require_finite, vector::product_three,
};

mod trimmed;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum Measure {
    Area,
    Volume,
}

struct PreparedFace<'a> {
    face: &'a BrepFace,
    surface: NurbsSurface,
    rectangular: bool,
    plane: Option<PlanarSurfacePlane>,
}

impl Brep {
    /// Computes surface area from exact NURBS and trim geometry.
    ///
    /// Rectangular domains are integrated per knot-span rectangle. Other
    /// planar faces use boundary integrals; curved faces use nested adaptive
    /// quadrature over their oriented UV boundaries, including holes. Control
    /// geometry is recentered to limit sensitivity to model translation.
    pub fn area(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        self.integrate_mass_property(Measure::Area, tolerance)
    }

    /// Computes oriented volume of a closed, consistently oriented B-rep.
    ///
    /// The divergence-theorem flux is integrated over exact rectangular or
    /// trimmed NURBS faces. The bounds center is subtracted before the scalar
    /// triple product; display tessellation is never used.
    pub fn signed_volume(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        if !self.is_solid() {
            return Err(GeometryError::OpenBrepVolume);
        }
        self.integrate_mass_property(Measure::Volume, tolerance)
    }

    fn integrate_mass_property(
        &self,
        measure: Measure,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let bounds = self.bounds();
        let reference = bounds.center()?;
        let scale = bounds
            .min()
            .distance_to(bounds.max())?
            .max(tolerance.absolute());
        let area_tolerance = mass_tolerance(tolerance.absolute(), scale, 1.0)?;
        let absolute_tolerance = match measure {
            Measure::Area => area_tolerance,
            Measure::Volume => mass_tolerance(tolerance.absolute(), scale, scale)?,
        };
        let prepared = self
            .faces
            .iter()
            .map(|face| {
                let mut surface = centered_surface(&face.surface, reference)?;
                let mut rectangular = face_covers_full_surface_domain(face, tolerance)?;
                if !rectangular && let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)?
                {
                    surface = surface
                        .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
                    rectangular = true;
                }
                let plane = if rectangular {
                    None
                } else {
                    planar_surface_plane(&surface, tolerance)?
                };
                Ok(PreparedFace {
                    face,
                    surface,
                    rectangular,
                    plane,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let piece_count = prepared.iter().try_fold(0_usize, |total, prepared| {
            let count = if prepared.rectangular {
                prepared
                    .surface
                    .spans_u()
                    .count()
                    .checked_mul(prepared.surface.spans_v().count())
                    .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?
            } else {
                1
            };
            total
                .checked_add(count)
                .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
        })?;
        let piece_tolerance = (absolute_tolerance / piece_count as Real).max(Real::MIN_POSITIVE);
        let piece_area_tolerance = (area_tolerance / piece_count as Real).max(Real::MIN_POSITIVE);
        let mut sum = 0.0;
        let mut correction = 0.0;
        for PreparedFace {
            face,
            surface,
            rectangular,
            plane,
        } in prepared
        {
            if rectangular {
                for u in surface.spans_u() {
                    for v in surface.spans_v() {
                        let value = match measure {
                            Measure::Area => integrate_area_patch(
                                &surface,
                                [u.0, u.1],
                                [v.0, v.1],
                                piece_tolerance,
                                tolerance.relative(),
                            )?,
                            Measure::Volume => integrate_volume_patch(
                                &surface,
                                face.reversed,
                                [u.0, u.1],
                                [v.0, v.1],
                                piece_tolerance,
                                tolerance.relative(),
                            )?,
                        };
                        neumaier_add(&mut sum, &mut correction, value);
                    }
                }
            } else {
                let value = if let Some(plane) = plane {
                    match measure {
                        Measure::Area => {
                            0.5 * integrate_planar_trimmed_face_doubled_area(
                                face,
                                &surface,
                                plane,
                                piece_area_tolerance,
                                tolerance.relative(),
                            )?
                            .abs()
                        }
                        Measure::Volume => integrate_planar_trimmed_face_volume(
                            face,
                            &surface,
                            plane,
                            piece_area_tolerance,
                            tolerance.relative(),
                        )?,
                    }
                } else {
                    trimmed::integrate(
                        face,
                        &surface,
                        measure,
                        piece_tolerance,
                        tolerance.relative(),
                    )?
                };
                neumaier_add(&mut sum, &mut correction, value);
            }
        }
        let value = sum + correction;
        require_finite([value], "B-rep mass property")?;
        Ok(value)
    }
}

fn mass_tolerance(
    absolute: Real,
    first_scale: Real,
    second_scale: Real,
) -> Result<Real, GeometryError> {
    match product_three(
        absolute,
        first_scale,
        second_scale,
        "B-rep mass property tolerance",
    ) {
        Ok(value) => Ok(value),
        Err(GeometryError::NonFinite { .. }) => Ok(Real::MAX),
        Err(error) => Err(error),
    }
}

fn integrate_planar_trimmed_face_volume(
    face: &BrepFace,
    surface: &NurbsSurface,
    plane: PlanarSurfacePlane,
    absolute_area_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let doubled_area = integrate_planar_trimmed_face_doubled_area(
        face,
        surface,
        plane,
        absolute_area_tolerance,
        relative_tolerance,
    )?;
    let plane_position = Vector3::try_new(plane.point.x(), plane.point.y(), plane.point.z())?;
    let plane_distance = plane_position.dot(plane.normal.as_vector())?;
    let magnitude = product_three(
        plane_distance.abs(),
        doubled_area.abs(),
        1.0 / 6.0,
        "planar B-rep face volume",
    )?;
    let orientation = if face.reversed { -1.0 } else { 1.0 };
    Ok(orientation * plane_distance.signum() * doubled_area.signum() * magnitude)
}

fn integrate_planar_trimmed_face_doubled_area(
    face: &BrepFace,
    surface: &NurbsSurface,
    plane: PlanarSurfacePlane,
    absolute_area_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let span_count = face
        .loops
        .iter()
        .flat_map(|face_loop| &face_loop.trims)
        .map(|trim| trim.curve.spans().count())
        .try_fold(0_usize, |total, count| {
            total
                .checked_add(count)
                .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
        })?;
    if span_count == 0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let span_tolerance = (absolute_area_tolerance / span_count as Real).max(Real::MIN_POSITIVE);
    let mut sum = 0.0;
    let mut correction = 0.0;
    for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
        for (start, end) in trim.curve.spans() {
            let doubled_area = integrate_adaptive(
                start,
                end,
                span_tolerance,
                relative_tolerance,
                |parameter| {
                    let (surface_parameter, parameter_derivative) =
                        trim.curve.evaluate_with_derivative(parameter)?;
                    let (point, derivative_u, derivative_v) = surface
                        .evaluate_with_derivatives(surface_parameter.x(), surface_parameter.y())?;
                    let derivative = Vector3::try_new(
                        derivative_u.x().mul_add(
                            parameter_derivative[0],
                            derivative_v.x() * parameter_derivative[1],
                        ),
                        derivative_u.y().mul_add(
                            parameter_derivative[0],
                            derivative_v.y() * parameter_derivative[1],
                        ),
                        derivative_u.z().mul_add(
                            parameter_derivative[0],
                            derivative_v.z() * parameter_derivative[1],
                        ),
                    )?;
                    let position = Vector3::try_new(point.x(), point.y(), point.z())?;
                    position.cross(derivative)?.dot(plane.normal.as_vector())
                },
            )?;
            neumaier_add(&mut sum, &mut correction, doubled_area);
        }
    }
    let doubled_area = sum + correction;
    require_finite([doubled_area], "planar B-rep doubled face area")?;
    Ok(doubled_area)
}

fn integrate_volume_patch(
    surface: &NurbsSurface,
    reversed: bool,
    u: [Real; 2],
    v: [Real; 2],
    absolute_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let half_u = u[1] * 0.5 - u[0] * 0.5;
    let half_v = v[1] * 0.5 - v[0] * 0.5;
    require_finite([half_u, half_v], "B-rep volume parameter span")?;
    if half_u <= 0.0 || half_v <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let inner_tolerance = (absolute_tolerance * 0.25).max(Real::MIN_POSITIVE);
    integrate_adaptive(
        0.0,
        1.0,
        absolute_tolerance,
        relative_tolerance,
        |normalized_u| {
            let parameter_u = normalized_span_parameter(u, normalized_u)?;
            integrate_adaptive(
                0.0,
                1.0,
                inner_tolerance,
                relative_tolerance,
                |normalized_v| {
                    let parameter_v = normalized_span_parameter(v, normalized_v)?;
                    let (point, derivative_u, derivative_v) =
                        surface.evaluate_with_derivatives(parameter_u, parameter_v)?;
                    let position = Vector3::try_new(point.x(), point.y(), point.z())?;
                    let normalized_u = derivative_u.scaled(half_u)?;
                    let normalized_v = derivative_v.scaled(half_v)?;
                    let triple = position.dot(normalized_u.cross(normalized_v)?)?;
                    let magnitude =
                        product_three(triple.abs(), 4.0, 1.0 / 3.0, "B-rep volume integrand")?;
                    let orientation = if reversed { -1.0 } else { 1.0 };
                    Ok(orientation * triple.signum() * magnitude)
                },
            )
        },
    )
}
