//! Quartic edge compatibility without the odd-valence quadratic boundary rule.
use super::*;

struct Fan {
    ring: Vec<Corner>,
    origin: [f64; 3],
    first: Vec<[f64; 3]>,
    second: Vec<[f64; 3]>,
    third: Vec<[f64; 3]>,
    first_delta: Vec<[f64; 3]>,
    second_delta: Vec<[f64; 3]>,
    cosine: f64,
}

fn prepare(patches: &[SubDNurbsPatch], ring: &[Corner]) -> Fan {
    let get = |c: Corner, along| {
        let surface = &patches[c.patch].surface;
        surface.control_points()[edge_index(surface, c.local, true, along, 0)]
            .point()
            .to_array()
    };
    let origin = get(ring[0], 0);
    let relative = |k| {
        ring.iter()
            .map(|&c| {
                let p = get(c, k);
                std::array::from_fn(|a| p[a] - origin[a])
            })
            .collect::<Vec<[f64; 3]>>()
    };
    let old_first = relative(1);
    let old_second = relative(2);
    let first = stencil::first_harmonic(&old_first);
    let mut alternating = [0.; 3];
    for (i, p) in old_second.iter().enumerate() {
        let sign = if i.is_multiple_of(2) { 1. } else { -1. };
        for a in 0..3 {
            alternating[a] += sign * p[a] / ring.len() as f64;
        }
    }
    let second = old_second
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let sign = if i.is_multiple_of(2) { 1. } else { -1. };
            std::array::from_fn(|a| p[a] - sign * alternating[a])
        })
        .collect::<Vec<[f64; 3]>>();
    Fan {
        ring: ring.to_vec(),
        origin,
        first_delta: first
            .iter()
            .zip(old_first)
            .map(|(new, old)| std::array::from_fn(|a| new[a] - old[a]))
            .collect(),
        second_delta: second
            .iter()
            .zip(old_second)
            .map(|(new, old)| std::array::from_fn(|a| new[a] - old[a]))
            .collect(),
        first,
        second,
        third: relative(3),
        cosine: (std::f64::consts::TAU / ring.len() as f64).cos(),
    }
}

pub(super) fn apply(
    patches: &mut [SubDNurbsPatch],
    rings: &[Vec<Corner>],
    elevate: &[bool],
) -> Result<(), GeometryError> {
    if rings.is_empty() {
        return Ok(());
    }
    let fans = rings
        .iter()
        .map(|ring| prepare(patches, ring))
        .collect::<Vec<_>>();
    let mut point_count = 0usize;
    for (patch, &enabled) in patches.iter().zip(elevate) {
        let surface = &patch.surface;
        let extra = |knots: &[f64]| {
            if enabled {
                knots.windows(2).filter(|w| w[0] < w[1]).count()
            } else {
                0
            }
        };
        point_count += (surface.control_point_count_u() + extra(surface.knots_u()))
            * (surface.control_point_count_v() + extra(surface.knots_v()));
        if point_count > 4_000_000 {
            return Err(failure("G1xx quartic control point budget exceeded"));
        }
    }
    for fan in &fans {
        cubic_boundaries(patches, fan)?;
    }
    for (patch, &enabled) in patches.iter_mut().zip(elevate) {
        if enabled {
            patch.surface = patch.surface.try_change_degree(4, 4, false)?;
        }
    }
    for fan in fans {
        improve(patches, &fan)?;
    }
    Ok(())
}

fn improve(patches: &mut [SubDNurbsPatch], fan: &Fan) -> Result<(), GeometryError> {
    let rhs = fan
        .first
        .iter()
        .zip(&fan.second)
        .map(|(first, second)| {
            std::array::from_fn(|a| 1.5 * first[a] + 0.75 * fan.cosine * (second[a] - first[a]))
        })
        .collect::<Vec<_>>();
    let twists = stencil::solve(&rhs);
    for (i, &c) in fan.ring.iter().enumerate() {
        if patches[c.patch].surface.degree_u() != 4 {
            continue;
        }
        let surface = &patches[c.patch].surface;
        let mut controls = surface
            .control_points()
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>();
        let mut shift = |outgoing, along, across, delta: [f64; 3]| -> Result<(), GeometryError> {
            let at = edge_index(surface, c.local, outgoing, along, across);
            let old = controls[at].to_array();
            controls[at] = Point3::try_from(std::array::from_fn(|a| old[a] + delta[a]))?;
            Ok(())
        };
        for (outgoing, j) in [(true, i), (false, (i + 1) % fan.ring.len())] {
            let delta = std::array::from_fn(|a| {
                0.375 * fan.first_delta[j][a]
                    + fan.cosine / 16. * (fan.third[j][a] - fan.second[j][a])
            });
            shift(outgoing, 2, 1, delta)?;
        }
        controls[index(surface, c.local, 1, 1)] =
            Point3::try_from(std::array::from_fn(|a| fan.origin[a] + twists[i][a]))?;
        patches[c.patch].surface = NurbsSurface::try_new(
            4,
            4,
            surface.control_point_count_u(),
            surface.control_point_count_v(),
            controls,
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )?;
    }
    Ok(())
}

// The first processed extraordinary corner selects the native patch degree.
// Even twists retain G0 values on cubic patches; boundaries update everywhere.
fn cubic_boundaries(patches: &mut [SubDNurbsPatch], fan: &Fan) -> Result<(), GeometryError> {
    for (i, &c) in fan.ring.iter().enumerate() {
        let surface = &patches[c.patch].surface;
        let mut controls = surface
            .control_points()
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>();
        for (outgoing, j) in [(true, i), (false, (i + 1) % fan.ring.len())] {
            for (along, across, delta) in [
                (1, 0, fan.first_delta[j]),
                (2, 0, fan.second_delta[j]),
                (2, 1, fan.second_delta[j]),
            ] {
                let at = edge_index(surface, c.local, outgoing, along, across);
                let old = controls[at].to_array();
                controls[at] = Point3::try_from(std::array::from_fn(|a| old[a] + delta[a]))?;
            }
        }
        patches[c.patch].surface = NurbsSurface::try_new(
            3,
            3,
            surface.control_point_count_u(),
            surface.control_point_count_v(),
            controls,
            surface.knots_u().to_vec(),
            surface.knots_v().to_vec(),
        )?;
    }
    Ok(())
}
