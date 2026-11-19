//! Keep the first chords of almost tangent outgoing trim branches comparable.
use super::*;

const MAX_BISECTIONS: usize = 12;

fn scaled_delta(from: Point2, to: Point2, width: [Real; 2]) -> [Real; 2] {
    [
        (to.x() - from.x()) / width[0],
        (to.y() - from.y()) / width[1],
    ]
}

struct Branch {
    origin: Point2,
    tangent: [Real; 2],
    parameter: Real,
    near_parameter: Real,
    length: Real,
}

impl Branch {
    fn new(
        curve: &NurbsCurve2,
        [parameter, near_parameter]: [Real; 2],
        sense: Real,
        widths: [Real; 2],
    ) -> Result<Self, GeometryError> {
        let (origin, slope) = match curve.evaluate_with_derivative(parameter) {
            Ok(value) => value,
            Err(_) => (curve.evaluate(parameter)?, [0.; 2]),
        };
        let tangent = [slope[0] * sense / widths[0], slope[1] * sense / widths[1]];
        let step = scaled_delta(origin, curve.evaluate(near_parameter)?, widths);
        Ok(Self {
            origin,
            tangent,
            parameter,
            near_parameter,
            length: step[0].hypot(step[1]),
        })
    }

    fn refine(
        &self,
        curve: &NurbsCurve2,
        other: &Self,
        widths: [Real; 2],
        output: &mut Vec<Real>,
    ) -> Result<(), GeometryError> {
        let mut parameter = self.near_parameter;
        let mut length = self.length;
        for _ in 0..MAX_BISECTIONS {
            if length <= 1.1 * other.length {
                break;
            }
            let middle = normalized_span_parameter([self.parameter, parameter], 0.5)?;
            if middle == self.parameter || middle == parameter {
                break;
            }
            let d = scaled_delta(self.origin, curve.evaluate(middle)?, widths);
            parameter = middle;
            length = d[0].hypot(d[1]);
            output.push(middle);
        }
        Ok(())
    }
}

pub(super) fn stations(
    boundary: &BrepLoop,
    resolution: usize,
    widths: [Real; 2],
) -> Result<Vec<Vec<Real>>, GeometryError> {
    if boundary.trims.len() < 2
        || resolution == 0
        || widths.iter().any(|x| !x.is_finite() || *x <= 0.)
    {
        return Ok(Vec::new());
    }
    let fraction = 1. / resolution as Real;
    let branches = boundary
        .trims
        .iter()
        .map(|trim| {
            let mut spans = trim.curve.spans();
            let first = spans.next().ok_or(GeometryError::InvalidKnotVector {
                context: "trim sampling requires a nonempty curve span",
            })?;
            let last = spans.last().unwrap_or(first);
            let first_sample = normalized_span_parameter([first.0, first.1], fraction)?;
            let last_sample = normalized_span_parameter([last.0, last.1], 1. - fraction)?;
            Ok([
                Branch::new(&trim.curve, [first.0, first_sample], 1., widths)?,
                Branch::new(&trim.curve, [last.1, last_sample], -1., widths)?,
            ])
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut extra = vec![Vec::new(); branches.len()];
    let epsilon = 128. * Real::EPSILON;
    for index in 0..branches.len() {
        let next = (index + 1) % branches.len();
        let a = &branches[index][1];
        let b = &branches[next][0];
        let gap = scaled_delta(a.origin, b.origin, widths);
        let den = a.tangent[0].hypot(a.tangent[1]) * b.tangent[0].hypot(b.tangent[1]);
        let dot = a.tangent[0].mul_add(b.tangent[0], a.tangent[1] * b.tangent[1]);
        if gap[0].hypot(gap[1]) > epsilon
            || a.length.min(b.length) <= epsilon
            || !den.is_finite()
            || den == 0.
            || dot < 0.98 * den
        {
            continue;
        }
        // At a cusp a coarse chord can cut through the neighboring curve's
        // samples even when the exact trims do not cross. Refine only the
        // longer branch and retain points on the actual parameter curve.
        if a.length > b.length {
            a.refine(&boundary.trims[index].curve, b, widths, &mut extra[index])?;
        } else {
            b.refine(&boundary.trims[next].curve, a, widths, &mut extra[next])?;
        }
    }
    for row in &mut extra {
        row.sort_by(Real::total_cmp);
        row.dedup();
    }
    Ok(extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_endpoint_sample_lengths_preserves_a_tangent_trim_cusp() {
        let point = |x, y| Point2::try_new(x, y).unwrap();
        let bezier = |points: [[Real; 2]; 3]| {
            NurbsCurve2::try_new(
                2,
                points.map(|[x, y]| point(x, y)).to_vec(),
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap()
        };
        let h = 1. / 16.;
        let curves = [
            bezier([[0., 0.], [0.5, 0.], [1., 1.]]),
            NurbsCurve2::try_line(point(1., 1.), point(1., 2.)).unwrap(),
            bezier([[1., 2.], [(1. + h) / 2., 2. * h], [h, 2. * h * h]]),
            bezier([[h, 2. * h * h], [h / 2., 0.], [0., 0.]]),
        ];
        let boundary = BrepLoop::try_new(
            BrepLoopType::Outer,
            curves
                .into_iter()
                .enumerate()
                .map(|(i, c)| {
                    BrepTrim::try_new(
                        [i, (i + 1) % 4],
                        Some(i),
                        false,
                        c,
                        BrepTrimType::Boundary,
                        SurfaceIso::NotIso,
                        [0.; 2],
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let coarse = sample_trim_loop(&boundary, 1).unwrap();
        assert!(
            triangulate_trim_region(&coarse, &[coarse.len()])
                .unwrap()
                .is_none()
        );
        let extra = stations(&boundary, 1, [1., 2.]).unwrap();
        assert!(extra[0].len() >= 4);
        assert!(extra[1..].iter().all(Vec::is_empty));
        let balanced = sample_trim_loop_with_stations(&boundary, 1, &extra).unwrap();
        assert!(
            triangulate_trim_region(&balanced, &[balanced.len()])
                .unwrap()
                .is_some()
        );
    }
}
