//! Validated directional and rectangular surface extraction policy.
use super::*;

#[cfg(test)]
mod tests;

impl NurbsSurface {
    pub(super) fn extraction_domain(
        &self,
        direction: SurfaceKnotDirection,
    ) -> Result<RangeInclusive<Real>, GeometryError> {
        match direction {
            SurfaceKnotDirection::U => Ok(self.domain_u()),
            SurfaceKnotDirection::V => Ok(self.domain_v()),
            SurfaceKnotDirection::Both => Err(GeometryError::InvalidControlNet {
                context: "directional extraction requires one surface axis",
            }),
        }
    }

    /// Preserve directional trimming policy while delegating control-curve
    /// extraction, including the exact polynomial-line path, to the caller.
    pub fn try_trimmed_direction_with_nurbs<E: From<GeometryError>>(
        &self,
        direction: SurfaceKnotDirection,
        interval: RangeInclusive<Real>,
        trim: &mut impl FnMut(&NurbsCurve, RangeInclusive<Real>) -> Result<NurbsCurve, E>,
    ) -> Result<Self, E> {
        self.try_trimmed_direction_with_extractor(
            direction,
            interval,
            &mut |surface, direction, range| match direction {
                SurfaceKnotDirection::U => {
                    surface.map_u_control_curves(|curve| trim(curve, range.clone()))
                }
                SurfaceKnotDirection::V => {
                    surface.map_v_control_curves(|curve| trim(curve, range.clone()))
                }
                SurfaceKnotDirection::Both => unreachable!("direction was validated"),
            },
        )
    }

    /// Restrict one axis through a whole-tensor extractor, retaining an exact
    /// whole-domain no-op and validating the returned degree and other basis.
    pub fn try_trimmed_direction_with_extractor<E: From<GeometryError>>(
        &self,
        direction: SurfaceKnotDirection,
        interval: RangeInclusive<Real>,
        trim: &mut impl FnMut(&Self, SurfaceKnotDirection, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Self, E> {
        let domain = self.extraction_domain(direction)?;
        crate::validate_curve_trim_interval(&interval, domain.clone())?;
        if interval == domain {
            return Ok(self.clone());
        }
        let output = trim(self, direction, interval.clone())?;
        self.check_directional_layout(&output, direction)?;
        if output.extraction_domain(direction)? != interval {
            return Err(GeometryError::InvalidControlNet {
                context: "surface extraction changed the requested interval",
            }
            .into());
        }
        Ok(output)
    }

    /// Extract a rectangular subdomain in U then V, rejecting invalid bounds
    /// before either backend callback and preserving each whole-axis no-op.
    pub fn try_trimmed_with_extractor<E: From<GeometryError>>(
        &self,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        trim: &mut impl FnMut(&Self, SurfaceKnotDirection, RangeInclusive<Real>) -> Result<Self, E>,
    ) -> Result<Self, E> {
        crate::validate_curve_trim_interval(&u, self.domain_u())?;
        crate::validate_curve_trim_interval(&v, self.domain_v())?;
        self.try_trimmed_direction_with_extractor(SurfaceKnotDirection::U, u, trim)?
            .try_trimmed_direction_with_extractor(SurfaceKnotDirection::V, v, trim)
    }

    /// Split at one interior parameter, retaining the unselected basis and
    /// checking both returned intervals before exposing either result.
    pub fn try_split_direction_with_extractor<E: From<GeometryError>>(
        &self,
        direction: SurfaceKnotDirection,
        parameter: Real,
        split: &mut impl FnMut(&Self, SurfaceKnotDirection, Real) -> Result<(Self, Self), E>,
    ) -> Result<(Self, Self), E> {
        let domain = self.extraction_domain(direction)?;
        crate::require_finite([parameter], "NURBS curve split parameter")?;
        if parameter <= *domain.start() || parameter >= *domain.end() {
            return Err(GeometryError::InvalidCurveSplitParameter.into());
        }
        let (left, right) = split(self, direction, parameter)?;
        self.check_directional_layout(&left, direction)?;
        self.check_directional_layout(&right, direction)?;
        if left.extraction_domain(direction)? != (*domain.start()..=parameter)
            || right.extraction_domain(direction)? != (parameter..=*domain.end())
        {
            return Err(GeometryError::InvalidControlNet {
                context: "surface split changed the requested intervals",
            }
            .into());
        }
        Ok((left, right))
    }

    pub(super) fn split_direction_control_curves(
        &self,
        direction: SurfaceKnotDirection,
        parameter: Real,
    ) -> Result<(Self, Self), GeometryError> {
        let mut rights = Vec::new();
        let mut split = |curve: &NurbsCurve| {
            let (left, right) = curve.try_split(parameter)?;
            rights.push(right);
            Ok::<_, GeometryError>(left)
        };
        let left = match direction {
            SurfaceKnotDirection::U => self.map_u_control_curves(&mut split)?,
            SurfaceKnotDirection::V => self.map_v_control_curves(&mut split)?,
            SurfaceKnotDirection::Both => unreachable!("direction was validated"),
        };
        let mut rights = rights.into_iter();
        let mut right = |_: &NurbsCurve| {
            Ok::<_, GeometryError>(rights.next().expect("each control curve was split once"))
        };
        let right = match direction {
            SurfaceKnotDirection::U => self.map_u_control_curves(&mut right)?,
            SurfaceKnotDirection::V => self.map_v_control_curves(&mut right)?,
            SurfaceKnotDirection::Both => unreachable!("direction was validated"),
        };
        Ok((left, right))
    }
}
