//! Cubic and quartic local G1xx processing from oriented smooth face fans.
use super::*;
use std::collections::BTreeMap;
mod even;
mod stencil;

#[derive(Clone, Copy)]
struct Corner {
    patch: usize,
    local: usize,
    incoming: u32,
}

fn failure(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}

fn index(surface: &NurbsSurface, corner: usize, i: usize, j: usize) -> usize {
    let u = if corner == 1 || corner == 2 {
        surface.control_point_count_u() - 1 - i
    } else {
        i
    };
    let v = if corner >= 2 {
        surface.control_point_count_v() - 1 - j
    } else {
        j
    };
    v * surface.control_point_count_u() + u
}

fn edge_index(
    surface: &NurbsSurface,
    corner: usize,
    outgoing: bool,
    along: usize,
    across: usize,
) -> usize {
    if corner.is_multiple_of(2) == outgoing {
        index(surface, corner, along, across)
    } else {
        index(surface, corner, across, along)
    }
}

pub(super) fn apply(source: &SubD, patches: &mut [SubDNurbsPatch]) -> Result<(), GeometryError> {
    let valence = super::continuity::valences(source);
    let enabled = super::soft::continuity_enabled(source)?;
    let selected = source
        .vertex_tags()
        .iter()
        .zip(&valence)
        .enumerate()
        .map(|(i, (&tag, &n))| tag == 1 && n != 4 && enabled[i])
        .collect::<Vec<_>>();
    if !selected.iter().any(|&v| v) {
        return Ok(());
    }
    let mut fans = BTreeMap::<u32, BTreeMap<u32, Corner>>::new();
    let edge_key = |a: u32, b: u32| (a.min(b), a.max(b));
    let creases = source
        .edges()
        .iter()
        .zip(source.edge_tags())
        .filter_map(|(&[a, b], &tag)| (tag == 2).then_some(edge_key(a, b)))
        .collect::<std::collections::BTreeSet<_>>();
    let mut point_count = 0usize;
    let mut quartic = Vec::with_capacity(patches.len());
    for (patch_index, patch) in patches.iter_mut().enumerate() {
        let face = &source.faces()[patch.face];
        let mut ends = [false; 4]; // U start, U end, V start, V end
        let mut degree = None;
        for local in 0..if patch.corner.is_some() { 1 } else { 4 } {
            let source_corner = patch.corner.unwrap_or(local);
            let vertex = face[source_corner];
            let tag = source.vertex_tags()[vertex as usize];
            let n = valence[vertex as usize];
            let outgoing = face[(source_corner + 1) % face.len()];
            let incoming = face[(source_corner + face.len() - 1) % face.len()];
            // Native chart selection counts a dart only on the side where
            // its incoming face edge is smooth. Its controls stay unchanged.
            if degree.is_none()
                && (tag == 1 || tag == 4)
                && n != 4
                && !creases.contains(&edge_key(vertex, incoming))
            {
                degree = Some(n.is_multiple_of(2));
            }
            if !selected[vertex as usize] {
                continue;
            }
            ends[usize::from(local == 1 || local == 2)] = true;
            ends[2 + usize::from(local >= 2)] = true;
            if fans
                .entry(vertex)
                .or_default()
                .insert(
                    outgoing,
                    Corner {
                        patch: patch_index,
                        local,
                        incoming,
                    },
                )
                .is_some()
            {
                return Err(failure("G1xx requires distinct manifold face-edge uses"));
            }
        }
        quartic.push(degree.unwrap_or(false));
        let nu =
            patch.surface.control_point_count_u() + usize::from(ends[0]) + usize::from(ends[1]);
        let nv =
            patch.surface.control_point_count_v() + usize::from(ends[2]) + usize::from(ends[3]);
        point_count += nu * nv;
        if point_count > 4_000_000 {
            return Err(failure("G1xx control point budget exceeded"));
        }
        for (end, enabled) in ends.into_iter().enumerate() {
            if !enabled {
                continue;
            }
            let knots = if end < 2 {
                patch.surface.knots_u()
            } else {
                patch.surface.knots_v()
            };
            let parameter = if end % 2 == 0 {
                knots[4]
            } else {
                knots[knots.len() - 5]
            };
            patch.surface = if end < 2 {
                patch.surface.try_insert_knot_u(parameter, 2)?
            } else {
                patch.surface.try_insert_knot_v(parameter, 2)?
            };
        }
    }
    let mut even_fans = Vec::new();
    for fan in fans.into_values() {
        let mut ring = Vec::with_capacity(fan.len());
        let first = *fan.first_key_value().unwrap().0;
        let mut edge = first;
        for _ in 0..fan.len() {
            let corner = *fan
                .get(&edge)
                .ok_or_else(|| failure("G1xx requires an oriented smooth face fan"))?;
            ring.push(corner);
            edge = corner.incoming;
            if edge == first {
                break;
            }
        }
        if edge != first || ring.len() != fan.len() || ring.len() < 3 {
            return Err(failure("G1xx requires one closed smooth face fan"));
        }
        if ring.len().is_multiple_of(2) {
            even_fans.push(ring);
        } else {
            improve(patches, &ring)?;
        }
    }
    even::apply(patches, &even_fans, &quartic)
}

fn improve(patches: &mut [SubDNurbsPatch], ring: &[Corner]) -> Result<(), GeometryError> {
    let edge_get = |c: Corner, outgoing: bool, along, across| {
        let surface = &patches[c.patch].surface;
        surface.control_points()[edge_index(surface, c.local, outgoing, along, across)]
            .point()
            .to_array()
    };
    let p = edge_get(ring[0], true, 0, 0);
    let mut opposite_third = Vec::with_capacity(ring.len());
    let mut second_owner = Vec::with_capacity(ring.len());
    let old = ring
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let previous = ring[(i + ring.len() - 1) % ring.len()];
            // The preceding face's incoming use owns the shared stencil.
            // Finite soft-crease charts can have unequal boundary controls.
            opposite_third.push(edge_get(c, true, 3, 0));
            second_owner.push(edge_get(previous, false, 2, 0));
            [
                edge_get(previous, false, 1, 0),
                edge_get(previous, false, 3, 0),
                edge_get(c, true, 1, 1),
            ]
        })
        .collect::<Vec<_>>();
    let updated = stencil::controls(p, &old, &opposite_third);
    for (i, &corner) in ring.iter().enumerate() {
        let next = (i + 1) % ring.len();
        let surface = &patches[corner.patch].surface;
        let mut controls = surface
            .control_points()
            .iter()
            .map(|c| c.point())
            .collect::<Vec<_>>();
        for (outgoing, edge) in [(true, i), (false, next)] {
            let at = edge_index(surface, corner.local, outgoing, 1, 0);
            controls[at] = Point3::try_from(updated[edge][0])?;
            let at = edge_index(surface, corner.local, outgoing, 2, 0);
            // The transverse shift is the owner's displacement on both faces.
            let delta =
                std::array::from_fn::<_, 3, _>(|a| updated[edge][1][a] - second_owner[edge][a]);
            controls[at] = Point3::try_from(updated[edge][1])?;
            let at = edge_index(surface, corner.local, outgoing, 2, 1);
            let old_point = controls[at].to_array();
            controls[at] = Point3::try_from(std::array::from_fn(|a| old_point[a] + delta[a]))?;
        }
        controls[index(surface, corner.local, 1, 1)] = Point3::try_from(updated[i][2])?;
        patches[corner.patch].surface = NurbsSurface::try_new(
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
