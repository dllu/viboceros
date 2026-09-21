use super::*;

pub(super) fn split(
    source: &NurbsSurface,
    axis: usize,
    cut: Real,
    budget: &mut Budget,
) -> Result<[NurbsSurface; 2], GeometryError> {
    let knots = [source.knots_u(), source.knots_v()];
    let degree = [source.degree_u(), source.degree_v()];
    let count = [
        source.control_point_count_u(),
        source.control_point_count_v(),
    ];
    let domain = [source.domain_u(), source.domain_v()];
    if !cut.is_finite() || cut <= *domain[axis].start() || cut >= *domain[axis].end() {
        return invalid("face partition knot must be finite and interior");
    }
    budget.charge(knots[axis].len())?;
    let a = knots[axis].partition_point(|&k| k < cut);
    let b = knots[axis].partition_point(|&k| k <= cut);
    if b - a != degree[axis] {
        return invalid("face partition requires a continuous full-degree knot");
    }
    let mut results = Vec::with_capacity(2);
    for side in 0..2 {
        let mut counts = count;
        counts[axis] = if side == 0 { a } else { count[axis] - a + 1 };
        budget.charge(counts[0].saturating_mul(counts[1]))?;
        let offset = if side == 0 { 0 } else { a - 1 };
        let mut controls = Vec::with_capacity(counts[0] * counts[1]);
        for v in 0..counts[1] {
            for u in 0..counts[0] {
                controls.push(
                    source
                        .control_point(
                            u + if axis == 0 { offset } else { 0 },
                            v + if axis == 1 { offset } else { 0 },
                        )
                        .unwrap(),
                );
            }
        }
        let mut new_knots = [knots[0].to_vec(), knots[1].to_vec()];
        new_knots[axis] = if side == 0 {
            [knots[axis][..a].to_vec(), vec![cut; degree[axis] + 1]].concat()
        } else {
            [vec![cut; degree[axis] + 1], knots[axis][b..].to_vec()].concat()
        };
        results.push(NurbsSurface::try_new_rational(
            degree[0],
            degree[1],
            counts[0],
            counts[1],
            controls,
            new_knots[0].clone(),
            new_knots[1].clone(),
        )?);
    }
    Ok(results.try_into().unwrap())
}
