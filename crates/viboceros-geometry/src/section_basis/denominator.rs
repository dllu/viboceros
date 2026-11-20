//! Exact Cartesian interpolation of rational sections in one common gauge.
use crate::{GeometryError, NurbsCurve, Point3, WeightedPoint3, bezier::polynomial};

/// Equalize the weight functions of already matched section curves without
/// changing their parameterized images. Keeping a common denominator avoids
/// introducing creases when otherwise smooth rational curves are blended.
/// Leave the supplied curves in their original gauge if the product would
/// exceed the caller's control limit or the supported expansion degree.
pub fn try_share_denominator(
    curves: &mut [NurbsCurve],
    maximum: usize,
) -> Result<(), GeometryError> {
    let Some(first) = curves.first() else {
        return Ok(());
    };
    if curves
        .iter()
        .any(|curve| curve.degree() != first.degree() || curve.knots() != first.knots())
    {
        return Err(GeometryError::InvalidControlNet {
            context: "common denominator requires matching section bases",
        });
    }
    let mut groups: Vec<Vec<f64>> = Vec::new();
    let mut representatives = Vec::new();
    let mut classes = Vec::with_capacity(curves.len());
    for (index, curve) in curves.iter().enumerate() {
        let weights = curve
            .control_points()
            .iter()
            .map(|point| point.weight())
            .collect::<Vec<_>>();
        if weights.iter().any(|&w| w <= 0.) {
            return Ok(());
        }
        let class = groups
            .iter()
            .position(|group| *group == weights)
            .unwrap_or_else(|| {
                groups.push(weights);
                representatives.push(index);
                groups.len() - 1
            });
        classes.push(class);
    }
    let degree = first.degree().saturating_mul(groups.len());
    let span_count = first.spans().count();
    if groups.len() <= 1 || degree > 32 || degree.saturating_mul(span_count) >= maximum {
        return Ok(());
    }
    let domain = first.domain();
    if first.knots().chunk_by(|a, b| a == b).any(|group| {
        group.len() > first.degree() && group[0] > *domain.start() && group[0] < *domain.end()
    }) {
        return Err(GeometryError::InvalidControlNet {
            context: "common denominator requires continuous sections",
        });
    }
    let spans = curves
        .iter()
        .map(NurbsCurve::try_bezier_spans)
        .collect::<Result<Vec<_>, _>>()?;
    let mut controls = vec![Vec::with_capacity(1 + degree * span_count); curves.len()];
    let mut knots = vec![*first.domain().start(); degree + 1];
    for span in 0..span_count {
        let factors = representatives
            .iter()
            .map(|&i| {
                spans[i][span]
                    .control_points()
                    .iter()
                    .map(|point| point.weight())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let denominator = multiply(factors.iter())?;
        let quotients = (0..factors.len())
            .map(|group| {
                multiply(
                    factors
                        .iter()
                        .enumerate()
                        .filter_map(|(i, factor)| (i != group).then_some(factor)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (index, sections) in spans.iter().enumerate() {
            let profile = sections[span].control_points();
            let mut numerators = Vec::with_capacity(3);
            for axis in 0..3 {
                let homogeneous = profile
                    .iter()
                    .map(|p| p.point().to_array()[axis] * p.weight())
                    .collect::<Vec<_>>();
                numerators.push(polynomial::product(
                    &homogeneous,
                    &quotients[classes[index]],
                    &mut |_| Ok(()),
                )?);
            }
            for (i, &weight) in denominator.iter().enumerate().skip(usize::from(span > 0)) {
                let point =
                    Point3::try_from(std::array::from_fn(|axis| numerators[axis][i] / weight))?;
                controls[index].push(WeightedPoint3::try_new(point, weight)?);
            }
        }
        knots.extend(std::iter::repeat_n(*spans[0][span].domain().end(), degree));
    }
    knots.push(*first.domain().end());
    let expanded = controls
        .into_iter()
        .map(|controls| NurbsCurve::try_new_rational(degree, controls, knots.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    curves.clone_from_slice(&expanded);
    Ok(())
}

fn multiply<'a>(
    factors: impl IntoIterator<Item = &'a Vec<f64>>,
) -> Result<Vec<f64>, GeometryError> {
    factors.into_iter().try_fold(vec![1.], |product, next| {
        polynomial::product(&product, next, &mut |_| Ok(()))
    })
}
