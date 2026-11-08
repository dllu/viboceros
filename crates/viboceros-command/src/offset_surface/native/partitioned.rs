//! Fit smooth rational charts before assembling their certified common edges.
use super::*;
use viboceros_geometry::{CurveContinuity, NurbsSurface, join_breps};

const MAX_STATIONS: usize = 1_000_000;

pub(super) fn offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    if options.solid {
        return Ok(None);
    }
    let (surface, reversed) = match geometry {
        Geometry::NurbsSurface(s) => (s, false),
        Geometry::Brep(b) if b.faces().len() == 1 && b.faces()[0].is_untrimmed(tolerance)? => {
            (b.faces()[0].surface(), b.faces()[0].is_reversed())
        }
        _ => return Ok(None),
    };
    if !surface.is_rational() || surface.is_closed_u()? || surface.is_closed_v()? {
        return Ok(None);
    }
    let breaks = surface.discontinuity_parameters(CurveContinuity::G2, tolerance.angular())?;
    if breaks.iter().all(Vec::is_empty)
        || surface
            .discontinuity_parameters(CurveContinuity::G1, tolerance.angular())?
            .iter()
            .any(|axis| !axis.is_empty())
    {
        return Ok(None);
    }
    let partition = Brep::try_surface_partitioned_at_discontinuities(
        surface,
        &[(CurveContinuity::G2, tolerance.angular())],
        tolerance,
    )?;
    let requested = options.effective_tolerance(tolerance);
    let approximation = (requested * 0.25).min(tolerance.absolute().min(1e-8));
    let mut stations = MAX_STATIONS;
    let mut skins = Vec::new();
    for side in if options.both_sides {
        &[1., -1.][..]
    } else {
        &[1.][..]
    } {
        let distance = options.distance * side * if reversed { -1. } else { 1. };
        let mut pieces = Vec::with_capacity(partition.faces().len());
        for face in partition.faces() {
            let source = face.surface();
            let fitted = viboceros_smlib::offset_surface(source, distance, approximation)?;
            check_fit(source, &fitted, distance, requested * 0.25, &mut stations)?;
            pieces.push(Brep::try_surface_face(fitted, tolerance)?);
        }
        let mut joined = join_breps(
            &pieces.iter().collect::<Vec<_>>(),
            tolerance.absolute(),
            tolerance,
        )?;
        if joined.len() != 1 || joined[0].brep.faces().len() != partition.faces().len() {
            return Err(failure("Offset patches did not form one certified shell"));
        }
        let mut skin = joined.remove(0).brep;
        if reversed {
            skin.reverse_orientation();
        }
        skins.push(skin);
    }
    Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
        skins, tolerance,
    )?)))
}

fn failure(message: &str) -> CommandError {
    viboceros_smlib::Error::Kernel(message.into()).into()
}

/// This is a finite independent fit admission check, not a continuous distance
/// certificate. Include native knot boundaries and midpoints: a successful
/// native status can still miss a fine request after reaching its fit limit.
fn check_fit(
    source: &NurbsSurface,
    fitted: &NurbsSurface,
    distance: f64,
    limit: f64,
    remaining: &mut usize,
) -> Result<(), CommandError> {
    let axes = [
        (source.domain_u(), source.knots_u(), fitted.knots_u()),
        (source.domain_v(), source.knots_v(), fitted.knots_v()),
    ]
    .map(|(domain, source, fitted)| {
        let mut sites = source
            .iter()
            .chain(fitted)
            .copied()
            .filter(|t| domain.contains(t))
            .collect::<Vec<_>>();
        sites.extend(
            (0..=16).map(|i| domain.start() + (domain.end() - domain.start()) * i as f64 / 16.),
        );
        sites.sort_by(f64::total_cmp);
        sites.dedup();
        let middle = sites
            .windows(2)
            .map(|w| w[0].midpoint(w[1]))
            .collect::<Vec<_>>();
        sites.extend(middle);
        sites
    });
    let left = axes[0]
        .len()
        .checked_mul(axes[1].len())
        .and_then(|n| remaining.checked_sub(n))
        .ok_or_else(|| failure("Offset fit exceeds its station budget"))?;
    *remaining = left;
    for &u in &axes[0] {
        for &v in &axes[1] {
            let ideal = source
                .evaluate(u, v)?
                .translated(source.normal_at(u, v)?.as_vector().scaled(distance)?)?;
            if ideal
                .distance_bound_to(fitted.evaluate(u, v)?, limit)?
                .is_none()
            {
                return Err(failure(
                    "Offset fit misses the requested normal-offset tolerance",
                ));
            }
        }
    }
    Ok(())
}
