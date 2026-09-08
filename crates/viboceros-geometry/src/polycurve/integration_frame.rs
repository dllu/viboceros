//! Temporary parameter conditioning without merging or changing leaf geometry.

use super::PolyCurve3;
use crate::{CurveSegment3, GeometryError, parameter::map_parameter};
use std::borrow::Cow;

impl PolyCurve3 {
    /// Normalizes the outer domain and NURBS/polyline leaf domains. Native
    /// analytic segments keep their parameterization. Stored geometry is never
    /// modified, and unrepresentable distinct intervals remain errors.
    pub(crate) fn for_integration(&self) -> Result<Cow<'_, Self>, GeometryError> {
        let needs_copy = self.domain() != (0.0..=1.0)
            || self.segments().iter().any(|segment| match segment {
                CurveSegment3::NurbsCurve(c) => c.domain() != (0.0..=1.0),
                CurveSegment3::Polyline(c) => c.domain() != (0.0..=1.0),
                _ => false,
            });
        if !needs_copy {
            return Ok(Cow::Borrowed(self));
        }
        // Validate the smaller outer frame before copying potentially large
        // leaf geometry. Failure here is loss of numerical resolution, not
        // an invalid source polycurve.
        let parameters = self
            .parameters
            .iter()
            .map(|&parameter| map_parameter(parameter, self.domain(), 0.0..=1.0))
            .collect::<Result<Vec<_>, _>>()?;
        if parameters.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        let segments = self
            .segments()
            .iter()
            .map(|segment| {
                Ok(match segment {
                    CurveSegment3::NurbsCurve(c) => {
                        CurveSegment3::NurbsCurve(c.for_integration()?.into_owned())
                    }
                    CurveSegment3::Polyline(c) => {
                        CurveSegment3::Polyline(c.for_integration()?.into_owned())
                    }
                    _ => segment.clone(),
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(Cow::Owned(Self::try_with_segment_domains(
            segments, parameters,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsCurve, Point3};

    #[test]
    fn collapsed_outer_intervals_report_numerical_failure_without_editing_source() {
        use crate::LineSegment;
        let points =
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]].map(|p| Point3::try_from(p).unwrap());
        let segments = points
            .windows(2)
            .map(|p| {
                CurveSegment3::Line(
                    LineSegment::try_new(p[0], p[1], crate::Tolerance::DEFAULT).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let source =
            PolyCurve3::try_with_segment_domains(segments, vec![-f64::from_bits(1), 0., f64::MAX])
                .unwrap();
        let original = source.clone();
        assert!(matches!(
            source.for_integration(),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
        assert_eq!(source, original);
    }

    #[test]
    fn collapsed_leaf_intervals_are_rejected_even_when_mapped_knots_are_valid() {
        let leaf = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
                Point3::try_new(1., 1., 0.).unwrap(),
                Point3::try_new(2., 1., 0.).unwrap(),
            ],
            vec![-1., -1., 0., f64::from_bits(1), f64::MAX, f64::MAX],
        )
        .unwrap();
        let mapped = leaf.try_reparameterized(0.0..=1.0).unwrap();
        assert_eq!(mapped.knots()[2], mapped.knots()[3]);
        let source = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::NurbsCurve(leaf)],
            vec![0., 1.],
        )
        .unwrap();
        let original = source.clone();
        assert!(matches!(
            source.for_integration(),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
        assert_eq!(source, original);
    }

    #[test]
    fn integration_copy_preserves_controls_weights_breaks_and_source() {
        let leaf = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5, 1., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
        .try_reparameterized(1.0..=f64::from_bits(1_f64.to_bits() + 1))
        .unwrap();
        let source = PolyCurve3::try_with_segment_domains(
            vec![CurveSegment3::NurbsCurve(leaf.clone())],
            vec![-10., 10.],
        )
        .unwrap();
        let original = source.clone();
        let normalized = source.for_integration().unwrap();
        assert!(matches!(normalized, Cow::Owned(_)));
        assert_eq!(normalized.parameters, vec![0., 1.]);
        let CurveSegment3::NurbsCurve(curve) = &normalized.segments()[0] else {
            panic!("leaf class changed")
        };
        assert_eq!(curve.control_points(), leaf.control_points());
        assert_eq!(curve.knots(), &[0., 0., 0., 1., 1., 1.]);
        assert_eq!(source, original);
        assert!(matches!(
            normalized.for_integration().unwrap(),
            Cow::Borrowed(_)
        ));
    }
}
