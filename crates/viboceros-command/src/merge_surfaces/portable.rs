//! Exact degree elevation/refinement followed by homogeneous concatenation.
use super::*;
use viboceros_geometry::{NurbsSurface, SurfaceKnotDirection, WeightedPoint3};

pub(super) fn join(
    a: &NurbsSurface,
    b: &NurbsSurface,
    direction: SurfaceKnotDirection,
    options: Options,
) -> Result<NurbsSurface, CommandError> {
    if direction == SurfaceKnotDirection::V {
        return Ok(join(
            &a.try_swapped_uv()?,
            &b.try_swapped_uv()?,
            SurfaceKnotDirection::U,
            options,
        )?
        .try_swapped_uv()?);
    }
    let (p, q) = (
        a.degree_u().max(b.degree_u()),
        a.degree_v().max(b.degree_v()),
    );
    let mut a = a
        .try_trimmed(a.domain_u(), a.domain_v())?
        .try_change_degree(p, q, false)?;
    let mut b = b
        .try_trimmed(b.domain_u(), b.domain_v())?
        .try_change_degree(p, q, false)?;
    b = b.try_reparameterized(b.domain_u(), a.domain_v())?;
    let mut groups = [a.knots_v(), b.knots_v()]
        .into_iter()
        .flat_map(|k| k.chunk_by(|a, b| a == b).map(|g| (g[0], g.len())))
        .collect::<Vec<_>>();
    groups.sort_by(|a, b| a.0.total_cmp(&b.0));
    let union = groups
        .chunk_by(|a, b| a.0 == b.0)
        .map(|g| (g[0].0, g.iter().map(|v| v.1).max().unwrap()))
        .collect::<Vec<_>>();
    let count_v = union.iter().map(|g| g.1).sum::<usize>() - q - 1;
    let count_u = a.control_point_count_u() + b.control_point_count_u() - 1;
    let controls = count_u
        .checked_mul(count_v)
        .filter(|&n| n <= 1_000_000)
        .ok_or(CommandError::Usage("MergeSrf control budget exceeded"))?;
    let missing = union
        .iter()
        .map(|&(t, n)| {
            [a.knots_v(), b.knots_v()]
                .into_iter()
                .map(|k| n - multiplicity(k, t))
                .sum::<usize>()
        })
        .sum::<usize>();
    if controls.checked_mul(missing).is_none_or(|n| n > 4_000_000) {
        return Err(CommandError::Usage(
            "MergeSrf refinement work budget exceeded",
        ));
    }
    for (t, n) in union {
        if t <= *a.domain_v().start() || t >= *a.domain_v().end() {
            continue;
        }
        if multiplicity(a.knots_v(), t) < n {
            a = a.try_insert_knot_v(t, n)?;
        }
        if multiplicity(b.knots_v(), t) < n {
            b = b.try_insert_knot_v(t, n)?;
        }
    }
    if a.knots_v() != b.knots_v() {
        return Err(CommandError::Usage(
            "MergeSrf incompatible transverse basis",
        ));
    }
    let seam = *a.domain_u().end();
    let shift = seam - b.domain_u().start();
    let mut knots = a.knots_u()[..a.knots_u().len() - 1].to_vec();
    knots.extend(b.knots_u()[p + 1..].iter().map(|k| k + shift));
    let mut net = Vec::with_capacity(controls);
    let na = a.control_point_count_u();
    let nb = b.control_point_count_u();
    for v in 0..count_v {
        net.extend_from_slice(&a.control_points()[v * na..v * na + na - 1]);
        let x = a.control_points()[v * na + na - 1];
        let y = b.control_points()[v * nb];
        let weight = 0.5 * x.weight() + 0.5 * y.weight();
        let xa = x.point().to_array();
        let ya = y.point().to_array();
        let point = std::array::from_fn::<_, 3, _>(|i| {
            (0.5 * x.weight() * xa[i] + 0.5 * y.weight() * ya[i]) / weight
        });
        net.push(WeightedPoint3::try_new(
            Point3::try_new(point[0], point[1], point[2])?,
            weight,
        )?);
        net.extend_from_slice(&b.control_points()[v * nb + 1..(v + 1) * nb]);
    }
    if options.smooth && p >= 2 && options.roundness > 0. {
        let start = knots.partition_point(|&k| k < seam);
        let end = knots.partition_point(|&k| k <= seam);
        knots[start] += (knots[start - 1] - seam) * (options.roundness * 0.5);
        knots[end - 1] += (knots[end] - seam) * (options.roundness * 0.5);
    }
    Ok(NurbsSurface::try_new_rational(
        p,
        q,
        count_u,
        count_v,
        net,
        knots,
        a.knots_v().to_vec(),
    )?)
}

fn multiplicity(knots: &[Real], parameter: Real) -> usize {
    knots.partition_point(|&k| k <= parameter) - knots.partition_point(|&k| k < parameter)
}
