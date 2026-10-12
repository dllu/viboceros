//! Shared tensor seam policy with external refinement and extraction producers.
use super::*;

#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Relocate selected closed seams in U then V, using whole-tensor geometry
    /// operations and the existing cyclic-control and homogeneous-append policy.
    /// Closure and periodic topology are determined from the complete control
    /// net, including constant rows. Unselected parameters are ignored.
    pub fn try_change_closed_seam_with_operations<E: From<GeometryError>>(
        &self,
        direction: SurfaceKnotDirection,
        parameter_u: Real,
        parameter_v: Real,
        refine: &mut impl FnMut(&Self, SurfaceKnotDirection, Real, usize) -> Result<Self, E>,
        split: &mut impl FnMut(&Self, SurfaceKnotDirection, Real, bool) -> Result<(Self, Self), E>,
    ) -> Result<Self, E> {
        let mut result = self.clone();
        for (axis, parameter, selected) in [
            (SurfaceKnotDirection::U, parameter_u, direction.includes_u()),
            (SurfaceKnotDirection::V, parameter_v, direction.includes_v()),
        ] {
            if !selected {
                continue;
            }
            let (closed, periodic) = match axis {
                SurfaceKnotDirection::U => (
                    result.homogeneous_control_curve_is_closed_u()?,
                    result.insertion_curve_is_periodic_u(),
                ),
                SurfaceKnotDirection::V => (
                    result.homogeneous_control_curve_is_closed_v()?,
                    result.insertion_curve_is_periodic_v(),
                ),
                SurfaceKnotDirection::Both => unreachable!(),
            };
            if !closed {
                return Err(GeometryError::SurfaceSeamDirectionMustBeClosed {
                    direction: if axis == SurfaceKnotDirection::U {
                        "U"
                    } else {
                        "V"
                    },
                }
                .into());
            }
            let first = result.seam_control_curve(axis, 0)?;
            let multiplicity = first.knot_multiplicity(parameter)?;
            let domain = first.domain();
            if parameter == *domain.start() || parameter == *domain.end() {
                result = result.map_seam_control_curves(axis, |curve| {
                    curve.try_change_closed_seam_with_periodic_topology(parameter, periodic)
                })?;
            } else if periodic && multiplicity <= 1 {
                let refined = refine(&result, axis, parameter, 1)?;
                result.check_directional_layout(&refined, axis)?;
                if refined.seam_control_curve(axis, 0)?.domain() != domain {
                    return Err(GeometryError::InvalidControlNet {
                        context: "seam refinement changed the active domain",
                    }
                    .into());
                }
                result = refined.map_seam_control_curves(axis, |curve| {
                    NurbsCurve::periodic_seam_from_refined(
                        curve.clone(),
                        parameter,
                        *domain.start(),
                        *domain.end(),
                    )
                })?;
            } else {
                let (left, right) = split(&result, axis, parameter, periodic)?;
                result.check_directional_layout(&left, axis)?;
                result.check_directional_layout(&right, axis)?;
                if left.seam_control_curve(axis, 0)?.domain() != (*domain.start()..=parameter)
                    || right.seam_control_curve(axis, 0)?.domain() != (parameter..=*domain.end())
                {
                    return Err(GeometryError::InvalidControlNet {
                        context: "seam split changed the requested domains",
                    }
                    .into());
                }
                let mut index = 0;
                result = left.map_seam_control_curves(axis, |curve| {
                    let second = right.seam_control_curve(axis, index)?;
                    index += 1;
                    NurbsCurve::closed_seam_from_split(
                        curve.clone(),
                        second,
                        *domain.start(),
                        *domain.end(),
                    )
                })?;
            }
        }
        Ok(result)
    }

    pub(super) fn check_directional_layout(
        &self,
        other: &Self,
        axis: SurfaceKnotDirection,
    ) -> Result<(), GeometryError> {
        let retained = match axis {
            SurfaceKnotDirection::U => {
                self.control_point_count_v == other.control_point_count_v
                    && self.knots_v == other.knots_v
            }
            SurfaceKnotDirection::V => {
                self.control_point_count_u == other.control_point_count_u
                    && self.knots_u == other.knots_u
            }
            SurfaceKnotDirection::Both => unreachable!(),
        };
        if !retained || self.degree_u != other.degree_u || self.degree_v != other.degree_v {
            return Err(GeometryError::InvalidControlNet {
                context: "surface operation changed degrees or the unselected basis",
            });
        }
        Ok(())
    }

    fn seam_control_curve(
        &self,
        axis: SurfaceKnotDirection,
        index: usize,
    ) -> Result<NurbsCurve, GeometryError> {
        match axis {
            SurfaceKnotDirection::U => NurbsCurve::try_new_rational(
                self.degree_u,
                self.control_points
                    [index * self.control_point_count_u..(index + 1) * self.control_point_count_u]
                    .to_vec(),
                self.knots_u.clone(),
            ),
            SurfaceKnotDirection::V => NurbsCurve::try_new_rational(
                self.degree_v,
                (0..self.control_point_count_v)
                    .map(|v| self.control_points[self.control_index(index, v)])
                    .collect(),
                self.knots_v.clone(),
            ),
            SurfaceKnotDirection::Both => unreachable!(),
        }
    }

    fn map_seam_control_curves(
        &self,
        axis: SurfaceKnotDirection,
        map: impl FnMut(&NurbsCurve) -> Result<NurbsCurve, GeometryError>,
    ) -> Result<Self, GeometryError> {
        match axis {
            SurfaceKnotDirection::U => self.map_u_control_curves(map),
            SurfaceKnotDirection::V => self.map_v_control_curves(map),
            SurfaceKnotDirection::Both => unreachable!(),
        }
    }

    pub(super) fn split_seam_control_curves(
        &self,
        axis: SurfaceKnotDirection,
        parameter: Real,
        periodic: bool,
    ) -> Result<(Self, Self), GeometryError> {
        let mut second = Vec::new();
        let left = self.map_seam_control_curves(axis, |curve| {
            let (left, right) = curve.try_split_with_periodic_topology(parameter, periodic)?;
            second.push(right);
            Ok(left)
        })?;
        let mut second = second.into_iter();
        let right = self.map_seam_control_curves(axis, |_| {
            Ok(second.next().expect("every control curve was split"))
        })?;
        Ok((left, right))
    }
}
