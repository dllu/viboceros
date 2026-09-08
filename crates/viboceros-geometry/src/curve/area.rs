//! Planar enclosed-area dispatch using analytic formulas or exact boundaries.

use crate::{Brep, CurveRef, GeometryError, Real, Tolerance};

impl CurveRef<'_> {
    /// Measures enclosed planar area without using display tessellation.
    /// General boundaries must satisfy the planar-face builder's closure,
    /// planarity, and trim-validity checks. Reversal does not change area.
    pub fn planar_area(self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        match self {
            Self::Circle(curve) => curve.area(),
            Self::Ellipse(curve) => curve.area(),
            Self::Polyline(curve) => curve.planar_area(tolerance),
            Self::Line(_) => Err(GeometryError::InvalidPlanarFaceBoundary),
            Self::NurbsCurve(curve) => {
                let normalized = curve.for_integration()?;
                Brep::try_planar_face(&normalized, tolerance)?.area(tolerance)
            }
            Self::PolyCurve(curve) => {
                // Normalize before promotion: mapping a multi-span leaf into
                // a poorly resolved outer domain can collapse knots, even
                // though the composite retains that leaf's full geometry.
                let normalized = curve.try_reparameterized(0.0..=1.0)?;
                CurveRef::NurbsCurve(&normalized.to_nurbs()?).planar_area(tolerance)
            }
            Self::Arc(_) => CurveRef::NurbsCurve(&self.to_nurbs()?).planar_area(tolerance),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Circle3, CurveSegment3, LineSegment, NurbsCurve, Point3, PolyCurve3, Polyline3, UnitVector3,
    };

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn closed_cubic_area_survives_extreme_affine_parameter_domains() {
        let curve = NurbsCurve::try_new(
            3,
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(0., 1., 0.),
                point(0., 0., 0.),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        for domain in [
            0.0..=1.0,
            1.0..=f64::from_bits(1_f64.to_bits() + 1),
            0.0..=f64::from_bits(1),
            -f64::MAX..=f64::MAX,
        ] {
            let mapped = curve.try_reparameterized(domain.clone()).unwrap();
            for mapped in [mapped.clone(), mapped.reversed().unwrap()] {
                let area = CurveRef::NurbsCurve(&mapped)
                    .planar_area(Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("{domain:?}: {error}"));
                // x=3t(1-t)^2, y=3t^2(1-t); half integral(x dy-y dx)=3/20.
                assert!((area - 0.15).abs() < 1e-12, "{domain:?}: {area}");
            }
        }
    }

    #[test]
    fn rational_circle_area_matches_analytic_area_in_both_directions() {
        let circle = Circle3::try_new(
            point(1e6, -2e6, 3e6),
            2.,
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let curve = circle.to_nurbs().unwrap();
        for domain in [
            0.0..=1.0,
            0.0..=1e-200,
            -f64::MAX..=f64::MAX,
            1.0..=f64::from_bits(1_f64.to_bits() + 16),
        ] {
            let curve = curve.try_reparameterized(domain).unwrap();
            for curve in [curve.clone(), curve.reversed().unwrap()] {
                let actual = CurveRef::NurbsCurve(&curve)
                    .planar_area(Tolerance::DEFAULT)
                    .unwrap();
                assert!((actual - 4. * std::f64::consts::PI).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn mixed_polycurve_uses_curved_boundary_not_its_control_polygon() {
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0., 0., 0.), point(0.5, 1., 0.), point(1., 0., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let line =
            LineSegment::try_new(point(1., 0., 0.), point(0., 0., 0.), Tolerance::DEFAULT).unwrap();
        let polycurve = PolyCurve3::try_new(vec![
            CurveSegment3::NurbsCurve(curve),
            CurveSegment3::Line(line),
        ])
        .unwrap();
        for domain in [
            0.0..=2.0,
            1.0..=f64::from_bits(1_f64.to_bits() + 2),
            0.0..=f64::from_bits(2),
            -f64::MAX / 2.0..=f64::MAX / 2.0,
        ] {
            let mapped = polycurve.try_reparameterized(domain).unwrap();
            for curve in [mapped.clone(), mapped.reversed().unwrap()] {
                let area = CurveRef::PolyCurve(&curve)
                    .planar_area(Tolerance::DEFAULT)
                    .unwrap();
                // x=t, y=2*t*(1-t): integral y dx = 1/3, not triangle area 1/2.
                assert!((area - 1. / 3.).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn composite_area_is_independent_of_outer_parameter_resolution() {
        let circle = Circle3::try_new(
            point(0., 0., 0.),
            2.,
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for domain in [
            [0., 1.],
            [1., f64::from_bits(1_f64.to_bits() + 1)],
            [0., f64::from_bits(1)],
            [-f64::MAX / 2.0, f64::MAX / 2.0],
        ] {
            // The composite keeps the leaf's four rational spans independently
            // of an outer domain that may have no representable interior value.
            let curve = PolyCurve3::try_with_segment_domains(
                vec![CurveSegment3::NurbsCurve(circle.to_nurbs().unwrap())],
                domain.to_vec(),
            )
            .unwrap();
            let original = curve.clone();
            let area = CurveRef::PolyCurve(&curve)
                .planar_area(Tolerance::DEFAULT)
                .unwrap_or_else(|error| panic!("{domain:?}: {error}"));
            assert!((area - 4. * std::f64::consts::PI).abs() < 1e-10);
            assert_eq!(curve, original);
        }
    }

    #[test]
    fn open_and_nonplanar_nurbs_boundaries_are_rejected() {
        for points in [
            vec![point(0., 0., 0.), point(1., 0., 0.), point(1., 1., 0.)],
            vec![
                point(0., 0., 0.),
                point(1., 0., 0.),
                point(1., 1., 1.),
                point(0., 1., 0.),
                point(0., 0., 0.),
            ],
        ] {
            let curve = Polyline3::try_new(points, Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap();
            assert!(
                CurveRef::NurbsCurve(&curve)
                    .planar_area(Tolerance::DEFAULT)
                    .is_err()
            );
        }
    }
}
