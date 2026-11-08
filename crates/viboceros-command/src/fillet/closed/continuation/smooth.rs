//! Expand polynomial endpoint charts until unseen branches cannot beat the best.
use super::*;

pub(super) fn solve(
    original: [&Curve3; 2],
    seed: [Real; 2],
    i: usize,
    radius: Real,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Option<Candidate>, CommandError> {
    let source = original[i];
    let domain = source.as_ref().domain();
    let nurbs = source.as_ref().to_nurbs()?;
    let mut curves = original.map(Clone::clone);
    let mut best = None;
    let mut step = *domain.end() - *domain.start();
    let boundary = original[1 - i].as_ref().tight_bounds(tolerance).ok();
    let fixed = Tolerance::try_new(
        tolerance.absolute(),
        Real::MIN_POSITIVE,
        tolerance.angular(),
    )?;
    for _ in 0..40 {
        curves[i] = Curve3::NurbsCurve(crate::extension_backend::curve_domain(
            &nurbs,
            *domain.start() - step..=*domain.end() + step,
        )?);
        let possible = boundary
            .zip(curves[i].as_ref().tight_bounds(fixed).ok())
            .is_none_or(|(a, b)| {
                let (amin, amax, bmin, bmax) = (
                    a.min().to_array(),
                    a.max().to_array(),
                    b.min().to_array(),
                    b.max().to_array(),
                );
                let padding = 2. * radius + tolerance.absolute();
                (0..3).all(|k| amin[k] <= bmax[k] + padding && bmin[k] <= amax[k] + padding)
            });
        if possible {
            for contact in
                search::solutions([&curves[0], &curves[1]], seed, i, radius, plane, tolerance)?
            {
                let cost = search::score([&curves[0], &curves[1]], seed, &contact, tolerance)?;
                if best.as_ref().is_none_or(|(old, _)| cost < *old) {
                    let mut candidate = curves.clone();
                    let t = contact.parameters[i];
                    candidate[i] = crate::curve_segments::trimmed(
                        &curves[i],
                        domain.start().min(t)..=domain.end().max(t),
                    )?;
                    best = Some((
                        cost,
                        Candidate {
                            curves: candidate,
                            contacts: contact,
                            reset_arc_domains: [false; 2],
                        },
                    ));
                }
            }
        }
        if let Some((cost, _)) = &best {
            let d = curves[i].as_ref().domain();
            let margin = 4. * tolerance.absolute().max(tolerance.relative() * cost);
            if search::distance(&curves[i], seed[i], *d.start(), tolerance)? > cost + margin
                && search::distance(&curves[i], seed[i], *d.end(), tolerance)? > cost + margin
            {
                break;
            }
        }
        if let Some(bounds) = boundary
            && let Curve3::NurbsCurve(curve) = &curves[i]
            && tail::excludes(curve, true, bounds, 2. * radius + tolerance.absolute())
            && tail::excludes(curve, false, bounds, 2. * radius + tolerance.absolute())
        {
            break;
        }
        step *= 2.;
    }
    Ok(best.map(|(_, candidate)| candidate))
}
