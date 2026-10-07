//! Bounded numerical closest-parameter fitting for off-surface source curves.
use super::*;

pub(super) fn fit(
    chart: &Chart<'_>,
    source: &NurbsCurve,
    tolerance: Tolerance,
    numerical: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let sampler = source.parameter_sampler()?;
    let node = |t| {
        let (u, v) = chart
            .surface
            .closest_parameters(sampler.evaluate(t)?, numerical)?;
        chart.point(Point2::try_new(u, v)?)
    };
    let mut segments = Vec::new();
    let mut cuts = source
        .knots()
        .iter()
        .filter(|t| source.domain().contains(t))
        .map(|t| {
            remap_scalar(
                *t,
                [*source.domain().start(), *source.domain().end()],
                [0., 1.],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    cuts.sort_by(Real::total_cmp);
    cuts.dedup();
    let mut pending = cuts
        .windows(2)
        .rev()
        .map(|x| (x[0], x[1], 0_usize))
        .collect::<Vec<_>>();
    while let Some((start, end, depth)) = pending.pop() {
        let fraction = |t: Real| start.mul_add(1. - t, end * t);
        let points = [
            node(start)?,
            node(fraction(1. / 3.))?,
            node(fraction(2. / 3.))?,
            node(end)?,
        ];
        let blend = |weights: [Real; 4]| {
            let a = points[0].to_array();
            Point3::try_from(std::array::from_fn(|axis| {
                points
                    .iter()
                    .zip(weights)
                    .skip(1)
                    .fold(a[axis], |sum, (p, w)| {
                        w.mul_add(p.to_array()[axis] - a[axis], sum)
                    })
            }))
        };
        let controls = [
            points[0],
            blend([-5. / 6., 3., -1.5, 1. / 3.])?,
            blend([1. / 3., -1.5, 3., -5. / 6.])?,
            points[3],
        ];
        let candidate =
            NurbsCurve::try_new(3, controls.to_vec(), vec![0., 0., 0., 0., 1., 1., 1., 1.])?;
        let mut fits = true;
        for i in 1..32 {
            let t = i as Real / 32.;
            if candidate.evaluate(t)?.distance_to(node(fraction(t))?)? > tolerance.absolute() * 0.25
            {
                fits = false;
                break;
            }
        }
        if fits {
            segments.push((start, end, controls));
        } else {
            if depth == 24 || segments.len() + pending.len() > 16_384 {
                return Err(GeometryError::SurfacePullbackDidNotConverge {
                    tolerance: tolerance.absolute(),
                });
            }
            let middle = (start + end) * 0.5;
            pending.push((middle, end, depth + 1));
            pending.push((start, middle, depth + 1));
        }
    }
    let mut controls = Vec::new();
    let mut knots = Vec::new();
    for (start, _, points) in &segments {
        knots.extend(std::iter::repeat_n(source.parameter_at(*start)?, 4));
        controls.extend(points);
    }
    knots.extend(std::iter::repeat_n(
        source.parameter_at(segments.last().unwrap().1)?,
        4,
    ));
    NurbsCurve::try_new(3, controls, knots)
}
