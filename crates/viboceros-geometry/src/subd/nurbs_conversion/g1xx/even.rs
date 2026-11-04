//! Quartic edge compatibility without the odd-valence quadratic boundary rule.
use super::*;

struct Fan {
    ring: Vec<Corner>,
    origin: [f64; 3],
    first: Vec<[f64; 3]>,
    second: Vec<[f64; 3]>,
    opposite_third: Vec<[f64; 3]>,
    second_delta: Vec<[f64; 3]>,
    cosine: f64,
}

fn prepare(patches: &[SubDNurbsPatch], ring: &[Corner]) -> Fan {
    let get = |c: Corner, outgoing: bool, along| {
        let surface = &patches[c.patch].surface;
        surface.control_points()[edge_index(surface, c.local, outgoing, along, 0)]
            .point()
            .to_array()
    };
    let origin = get(ring[0], true, 0);
    // The incoming use in the preceding face owns each shared boundary stencil.
    let relative = |k, opposite| {
        ring.iter()
            .enumerate()
            .map(|(i, &c)| {
                let prev = ring[(i + ring.len() - 1) % ring.len()];
                let (owner, outgoing) = if opposite { (c, true) } else { (prev, false) };
                let p = get(owner, outgoing, k);
                std::array::from_fn(|a| p[a] - origin[a])
            })
            .collect::<Vec<[f64; 3]>>()
    };
    let old_first = relative(1, false);
    let old_second = relative(2, false);
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
        second_delta: second
            .iter()
            .zip(old_second)
            .map(|(new, old)| std::array::from_fn(|a| new[a] - old[a]))
            .collect(),
        first,
        second,
        opposite_third: relative(3, true),
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
    // Enforce the quartic transverse sum after actual degree elevation. The
    // shared-boundary simplification loses terms when local depths differ.
    let transverse = fan
        .ring
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let prev = fan.ring[(i + fan.ring.len() - 1) % fan.ring.len()];
            let a = quartic_transverse(&patches[c.patch].surface, c.local, true, fan.origin);
            let b = quartic_transverse(&patches[prev.patch].surface, prev.local, false, fan.origin);
            std::array::from_fn::<_, 3, _>(|axis| {
                0.5 * (fan.first[i][axis]
                    + fan.second[i][axis]
                    + fan.cosine / 8. * (fan.opposite_third[i][axis] - fan.second[i][axis])
                    - a[axis]
                    - b[axis])
            })
        })
        .collect::<Vec<_>>();
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
            shift(outgoing, 2, 1, transverse[j])?;
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

fn quartic_transverse(
    surface: &NurbsSurface,
    corner: usize,
    outgoing: bool,
    origin: [f64; 3],
) -> [f64; 3] {
    let get = |along, across| {
        let p = surface.control_points()[edge_index(surface, corner, outgoing, along, across)]
            .point()
            .to_array();
        std::array::from_fn::<_, 3, _>(|a| p[a] - origin[a])
    };
    if surface.degree_u() == 4 {
        return get(2, 1);
    }
    // Tensor degree elevation at a doubled first cubic knot. Cubic charts keep
    // their degree, so obtain only this virtual quartic control in constant work.
    let mut result = [0.; 3];
    for (along, across, weight) in [(1, 0, 0.125), (2, 0, 0.125), (1, 1, 0.375), (2, 1, 0.375)] {
        let p = get(along, across);
        for a in 0..3 {
            result[a] += weight * p[a];
        }
    }
    result
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
            for (along, target) in [(1, fan.first[j]), (2, fan.second[j])] {
                let at = edge_index(surface, c.local, outgoing, along, 0);
                controls[at] =
                    Point3::try_from(std::array::from_fn(|a| fan.origin[a] + target[a]))?;
            }
            let at = edge_index(surface, c.local, outgoing, 2, 1);
            let old = controls[at].to_array();
            controls[at] =
                Point3::try_from(std::array::from_fn(|a| old[a] + fan.second_delta[j][a]))?;
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
