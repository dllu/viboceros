//! Sew neighboring sweep supports where their intersection crosses both rims.
use super::*;
use crate::{NurbsCurve2, Point2, SurfaceExtensionEdge, SurfaceSurfaceIntersectionProposal};

mod strip;
mod trace;

struct Joint {
    first: usize,
    second: usize,
    authored: bool,
}

pub(in crate::sweep::profile_miters) fn sew<E: From<GeometryError>>(
    panels: Vec<NurbsSurface>,
    hinges: &[Option<usize>],
    closed: bool,
    tolerance: Tolerance,
    intersect: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Tolerance,
    ) -> Result<Vec<SurfaceSurfaceIntersectionProposal>, E>,
) -> Result<Brep, E> {
    let joints = hinges
        .iter()
        .enumerate()
        .map(|(i, hinge)| Joint {
            first: i,
            second: i + 1,
            authored: hinge.is_some(),
        })
        .chain(closed.then_some(Joint {
            first: panels.len() - 1,
            second: 0,
            authored: false,
        }))
        .collect::<Vec<_>>();
    let domains = panels
        .iter()
        .map(NurbsSurface::domain_u)
        .collect::<Vec<_>>();
    let mut lengths = vec![[0.; 2]; panels.len()];
    for joint in joints.iter().filter(|joint| !joint.authored) {
        let a = panels[joint.first].isocurve_v(*domains[joint.first].end())?;
        let b = panels[joint.second].isocurve_v(*domains[joint.second].start())?;
        let distance = extension_distance(&a, &b)?;
        lengths[joint.first][1] = distance;
        lengths[joint.second][0] = distance;
    }
    let mut extended = Vec::with_capacity(panels.len());
    for (mut panel, distances) in panels.into_iter().zip(lengths) {
        for (edge, distance) in [SurfaceExtensionEdge::West, SurfaceExtensionEdge::East]
            .into_iter()
            .zip(distances)
        {
            if distance > tolerance.absolute() {
                panel = panel.try_extended_linearly_by_length(edge, distance)?;
            }
        }
        extended.push(panel);
    }
    let final_controls = extended
        .iter()
        .map(|s| s.control_point_count_u())
        .sum::<usize>();
    check_control_budget(final_controls, extended[0].control_point_count_v())?;
    let mut cuts = (0..extended.len())
        .map(|_| [None, None])
        .collect::<Vec<_>>();
    let mut connected = Vec::new();
    for joint in joints {
        if joint.authored {
            connected.push((joint.first, joint.second));
            continue;
        }
        let first = &extended[joint.first];
        let second = &extended[joint.second];
        let u = [*domains[joint.first].end(), *domains[joint.second].start()];
        let a = first.isocurve_v(u[0])?;
        let b = second.isocurve_v(u[1])?;
        let pair = if matching_sections(&a, &b, tolerance)? {
            Some([
                strip::Cut::at_u(first, u[0], a.clone())?,
                strip::Cut::at_u(second, u[1], a)?,
            ])
        } else {
            trace::complete_trace(
                [first, second],
                intersect(first, second, tolerance)?,
                tolerance,
            )?
        };
        if let Some([east, west]) = pair
            && strip::ordered(first, cuts[joint.first][0].as_ref(), Some(&east))
            && strip::ordered(second, Some(&west), cuts[joint.second][1].as_ref())
        {
            cuts[joint.first][1] = Some(east);
            cuts[joint.second][0] = Some(west);
            connected.push((joint.first, joint.second));
        }
    }
    let mut parts = Vec::with_capacity(extended.len());
    let mut sides = Vec::with_capacity(extended.len());
    let mut offset = 0;
    for ((panel, corner_cuts), original) in extended.into_iter().zip(cuts).zip(domains) {
        let part = strip::build(panel, corner_cuts, original, tolerance)?;
        sides.push(
            part.sides
                .map(|edges| edges.into_iter().map(|i| i + offset).collect::<Vec<_>>()),
        );
        offset += part.brep.edges().len();
        parts.push(part.brep);
    }
    let mut pairs = Vec::new();
    for (a, b) in connected {
        let east = &sides[a][1];
        let west = &sides[b][0];
        if east.len() != west.len() {
            return Err(invalid("miter boundaries have different profile spans").into());
        }
        pairs.extend(east.iter().zip(west).map(|(&a, &b)| (a, b, false)));
    }
    let combined = Brep::try_combine(parts, tolerance)?;
    Ok(combined
        .try_join_edge_pairs(&pairs, tolerance.absolute(), tolerance)?
        .partition_sweep_miter_faces(tolerance))
}
