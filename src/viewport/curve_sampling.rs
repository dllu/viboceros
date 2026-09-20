//! Identical model-space segments for GPU drawing, click and window picking.

use super::*;

pub(super) trait ViewportCurve {
    fn visit_segments(&self, visit: impl FnMut(Point3, Point3));
}

impl ViewportCurve for NurbsCurve {
    fn visit_segments(&self, mut visit: impl FnMut(Point3, Point3)) {
        if let Ok(sampler) = self.parameter_sampler() {
            for span in sampler.spans() {
                visit_parametric_segments(CURVE_SAMPLES_PER_SPAN, |t| span.evaluate(t), &mut visit);
            }
        }
    }
}

impl ViewportCurve for CurveSegment3 {
    fn visit_segments(&self, mut visit: impl FnMut(Point3, Point3)) {
        match self {
            Self::NurbsCurve(curve) => curve.visit_segments(visit),
            Self::Line(line) => visit(line.start(), line.end()),
            Self::Polyline(curve) => {
                for pair in curve.vertices().windows(2) {
                    visit(pair[0], pair[1]);
                }
            }
            Self::Arc(arc) => visit_parametric_segments(
                circular_arc_samples(*arc),
                |t| arc.point_at(t),
                &mut visit,
            ),
        }
    }
}

fn visit_parametric_segments(
    count: usize,
    evaluate: impl Fn(Real) -> Result<Point3, GeometryError>,
    visit: &mut impl FnMut(Point3, Point3),
) {
    let mut previous = None;
    for sample in 0..=count {
        let point = evaluate(sample as Real / count as Real).ok();
        if let (Some(start), Some(end)) = (previous, point) {
            visit(start, end);
        }
        // An unevaluable point breaks the polyline; never bridge across it.
        previous = point;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_sample_breaks_the_polyline() {
        let mut segments = Vec::new();
        visit_parametric_segments(
            4,
            |t| {
                if t == 0.5 {
                    Err(GeometryError::ZeroWeightAtParameter)
                } else {
                    Point3::try_new(t, 0., 0.)
                }
            },
            &mut |a, b| segments.push([a.x(), b.x()]),
        );
        assert_eq!(segments, [[0., 0.25], [0.75, 1.]]);
    }

    #[test]
    fn analytic_leaf_sampling_ignores_coarse_native_domains() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let line =
            viboceros_geometry::LineSegment::try_new(p(0., 0.), p(2., 1.), Tolerance::DEFAULT)
                .unwrap();
        let arc = CircularArc3::try_from_circle_sweep(
            Circle3::try_new(
                p(0., 0.),
                2.,
                Vector3::try_new(0., 0., 1.)
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
            std::f64::consts::FRAC_PI_2,
        )
        .unwrap();
        let origin = 2.0_f64.powi(52);
        let leaves = [
            (
                CurveSegment3::Line(line),
                CurveSegment3::Line(line.try_reparameterized(origin..=origin + 2.).unwrap()),
            ),
            (
                CurveSegment3::Arc(arc),
                CurveSegment3::Arc(arc.try_reparameterized(origin..=origin + 2.).unwrap()),
            ),
        ];
        for (original, shifted) in leaves {
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            original.visit_segments(|a, b| expected.push([a, b]));
            shifted.visit_segments(|a, b| actual.push([a, b]));
            assert!(!actual.is_empty());
            assert_eq!(actual, expected);
        }
        let polyline =
            Polyline3::try_new(vec![p(0., 0.), p(1., 2.), p(4., 3.)], Tolerance::DEFAULT).unwrap();
        let mut actual = Vec::new();
        CurveSegment3::Polyline(polyline.clone()).visit_segments(|a, b| actual.push([a, b]));
        assert_eq!(
            actual,
            polyline
                .vertices()
                .windows(2)
                .map(|p| [p[0], p[1]])
                .collect::<Vec<_>>()
        );
    }
}
