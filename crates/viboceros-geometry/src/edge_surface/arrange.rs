use super::*;

pub(super) fn curves(
    source: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let n = source.len();
    let ends = source
        .iter()
        .map(|c| {
            Ok([
                c.evaluate(*c.domain().start())?,
                c.evaluate(*c.domain().end())?,
            ])
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    if n == 2 {
        let direct = ends[0][0].distance_to(ends[1][0])? + ends[0][1].distance_to(ends[1][1])?;
        let reverse = ends[0][0].distance_to(ends[1][1])? + ends[0][1].distance_to(ends[1][0])?;
        return Ok(vec![
            source[0].clone(),
            if reverse < direct {
                source[1].reversed()?
            } else {
                source[1].clone()
            },
        ]);
    }
    let mut permutations = Vec::new();
    enumerate(&mut vec![0], n, &mut permutations);
    let mut best = None;
    for order in permutations {
        // Preserve the first input direction; every cycle also has a reversed
        // traversal, so this does not remove a geometric ordering candidate.
        for mask in (0..1_usize << n).step_by(2) {
            let mut gaps = Vec::new();
            for i in 0..n {
                let a = order[i];
                let b = order[(i + 1) % n];
                gaps.push(ends[a][1 - (mask >> a & 1)].distance_to(ends[b][mask >> b & 1])?);
            }
            let mut sorted = gaps.clone();
            sorted.sort_by(Real::total_cmp);
            let score = sorted.iter().fold(0.0_f64, |a, b| a.hypot(*b));
            if best.as_ref().is_none_or(|(s, _, _, _)| score < *s) {
                best = Some((score, order.clone(), mask, gaps));
            }
        }
    }
    let (_, mut order, mask, gaps) = best.unwrap();
    let (largest, gap) = gaps
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    if *gap > tolerance.absolute() {
        order.rotate_left((largest + 1) % n);
    }
    order
        .into_iter()
        .map(|i| {
            if mask >> i & 1 == 1 {
                source[i].reversed()
            } else {
                Ok(source[i].clone())
            }
        })
        .collect()
}

fn enumerate(order: &mut Vec<usize>, n: usize, output: &mut Vec<Vec<usize>>) {
    if order.len() == n {
        output.push(order.clone());
        return;
    }
    for i in 1..n {
        if !order.contains(&i) {
            order.push(i);
            enumerate(order, n, output);
            order.pop();
        }
    }
}

pub(super) fn close_corners(curves: &mut [NurbsCurve]) -> Result<(), GeometryError> {
    let n = curves.len();
    let corners = (0..n)
        .map(|i| {
            let a = curves[(i + n - 1) % n]
                .control_points()
                .last()
                .unwrap()
                .point()
                .to_array();
            let b = curves[i].control_points()[0].point().to_array();
            Point3::try_from(std::array::from_fn(|j| a[j] * 0.5 + b[j] * 0.5))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (i, curve) in curves.iter_mut().enumerate() {
        let start = curve.control_points()[0]
            .point()
            .vector_to(corners[i])?
            .to_array();
        let end = curve
            .control_points()
            .last()
            .unwrap()
            .point()
            .vector_to(corners[(i + 1) % n])?
            .to_array();
        let controls = curve
            .control_points()
            .iter()
            .zip(basis::greville(curve)?)
            .map(|(c, t)| {
                let p = c.point().to_array();
                WeightedPoint3::try_new(
                    Point3::try_from(std::array::from_fn(|j| {
                        p[j] + ((1.0 - t) * start[j] + t * end[j]) / c.weight()
                    }))?,
                    c.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        *curve = NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())?;
    }
    Ok(())
}
