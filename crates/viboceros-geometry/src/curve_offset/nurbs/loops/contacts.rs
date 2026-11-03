//! Bezier hull sweep for distinct visits of a closed polynomial offset.
use super::*;

pub(super) fn find(
    curve: &NurbsCurve,
    source: &NurbsCurve,
    tolerance: Tolerance,
    operations: Option<CurveOffsetOperations>,
    mut include: impl FnMut(Real) -> Result<bool, GeometryError>,
) -> Result<Vec<[Real; 2]>, GeometryError> {
    let mut pending = curve
        .try_bezier_spans()?
        .into_iter()
        .rev()
        .map(|c| (c, 0))
        .collect::<Vec<_>>();
    let mut pieces = Vec::new();
    while let Some((piece, depth)) = pending.pop() {
        if pieces.len() + pending.len() >= MAX_REGION_PIECES {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        if forward::backward(&piece, source)? {
            continue;
        }
        let c = piece.control_points();
        let chord = c[0].point().vector_to(c.last().unwrap().point())?;
        let monotone = chord.length()? > 0.
            && c.windows(2).all(|p| {
                p[0].point()
                    .vector_to(p[1].point())
                    .and_then(|v| v.dot(chord))
                    .is_ok_and(|v| v >= 0.)
            });
        if monotone {
            pieces.push(piece);
            continue;
        }
        let bounds = piece.control_point_bounds();
        // The search can omit a sub-tolerance stationary neighborhood. It is
        // never omitted from the actual locus; surviving geometry is validated.
        if bounds.min().distance_to(bounds.max())? <= tolerance.absolute() * 0.25 {
            continue;
        }
        if depth >= 48 {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        let domain = piece.domain();
        let t = domain.start().midpoint(*domain.end());
        let (a, b) = piece.try_split(t)?;
        pending.push((b, depth + 1));
        pending.push((a, depth + 1));
    }
    pieces.sort_by(|a, b| a.domain().start().total_cmp(b.domain().start()));
    let boxes = pieces
        .iter()
        .map(NurbsCurve::control_point_bounds)
        .collect::<Vec<_>>();
    let full = curve.control_point_bounds();
    let ext = full.min().vector_to(full.max())?.to_array();
    let axis = (0..3).max_by(|&a, &b| ext[a].total_cmp(&ext[b])).unwrap();
    let mut order = (0..pieces.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        boxes[a].min().to_array()[axis].total_cmp(&boxes[b].min().to_array()[axis])
    });
    let mut pairs = 0usize;
    let mut hits = Vec::new();
    for (position, &i) in order.iter().enumerate() {
        for &j in &order[position + 1..] {
            if boxes[j].min().to_array()[axis]
                > boxes[i].max().to_array()[axis] + tolerance.absolute()
            {
                break;
            }
            if (0..3).any(|k| {
                boxes[j].min().to_array()[k] > boxes[i].max().to_array()[k] + tolerance.absolute()
                    || boxes[i].min().to_array()[k]
                        > boxes[j].max().to_array()[k] + tolerance.absolute()
            }) {
                continue;
            }
            pairs += 1;
            if pairs > 1_000_000 {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
            let (left, right) = (i.min(j), i.max(j));
            let (a, b) = (&pieces[left], &pieces[right]);
            if right == left + 1
                && a.domain().end() == b.domain().start()
                && junction::separated(a, b)?
            {
                continue;
            }
            if left == 0 && right + 1 == pieces.len() && junction::separated(b, a)? {
                continue;
            }
            let junction = if right == left + 1 {
                let p = a.evaluate(*a.domain().end())?;
                let q = b.evaluate(*b.domain().start())?;
                (p.distance_to(q)? <= tolerance.absolute()).then_some(p.midpoint(q)?)
            } else {
                None
            };
            let seam = (*a.domain().start() == 0. && *b.domain().end() == 1.)
                .then(|| a.evaluate(0.))
                .transpose()?;
            let near_shared = |p: Point3| {
                [junction, seam].into_iter().flatten().any(|q| {
                    p.distance_to(q)
                        .is_ok_and(|d| d <= 2. * tolerance.absolute())
                })
            };
            for event in offset_intersections(a, b, tolerance, operations)? {
                match event {
                    CurveCurveIntersectionEvent::Point(hit) => {
                        if near_shared(hit.point()) {
                            continue;
                        }
                        let mut pair = [hit.first_parameter(), hit.second_parameter()];
                        pair.sort_by(Real::total_cmp);
                        if !include(pair[0])? || !include(pair[1])? {
                            continue;
                        }
                        for t in &mut pair {
                            if *t == 1. {
                                *t = 0.;
                            }
                        }
                        pair.sort_by(Real::total_cmp);
                        if pair[0] == pair[1] {
                            continue;
                        }
                        if !hits.iter().any(|p: &[Real; 2]| {
                            (p[0] - pair[0]).abs() < 1e-8 && (p[1] - pair[1]).abs() < 1e-8
                        }) {
                            hits.push(pair);
                        }
                        if hits.len() > 1024 {
                            return Err(GeometryError::NurbsOffsetFitLimit);
                        }
                    }
                    CurveCurveIntersectionEvent::Overlap(hit) => {
                        if !include(hit.start().first_parameter())?
                            || !include(hit.start().second_parameter())?
                            || !include(hit.end().first_parameter())?
                            || !include(hit.end().second_parameter())?
                        {
                            continue;
                        }
                        if !near_shared(hit.start().point()) || !near_shared(hit.end().point()) {
                            return Err(GeometryError::SelfIntersectingOffsetRegion);
                        }
                    }
                }
            }
        }
    }
    hits.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    Ok(hits)
}
