//! Miter the translated copies of a cross section along straight rail segments.
use super::*;
use crate::Brep;

mod panel;
#[cfg(test)]
mod tests;

impl Sweep1 {
    /// Build a single-profile sweep over the active corners of a polygonal rail.
    /// Smooth intervals continue through the ordinary sweep constructor.
    pub fn try_polyline_miters(
        rail: CurveRef<'_>,
        profile: &SweepSection,
        domain: RangeInclusive<Real>,
        style: SweepFrameStyle,
        untrimmed: bool,
        tolerance: Tolerance,
    ) -> Result<Option<Brep>, GeometryError> {
        if !matches!(rail, CurveRef::Polyline(_))
            && !matches!(rail, CurveRef::NurbsCurve(curve) if curve.degree() == 1)
        {
            return Ok(None);
        }
        let source = rail.to_nurbs()?;
        let start = rail.nurbs_parameter(*domain.start())?;
        let end = rail.nurbs_parameter(*domain.end())?;
        let station = rail.nurbs_parameter(profile.parameter)?;
        if start >= end || station < start || station > end {
            return Ok(None);
        }
        let rail = CurveRef::NurbsCurve(&source);
        let stations = split_at_corners(&source, start, end, tolerance)?;
        let closed =
            rail.evaluate(start)?.distance_to(rail.evaluate(end)?)? <= tolerance.absolute();
        if stations.len() == 2 {
            return Ok(None);
        }

        let segment_count = stations.len() - 1;
        let pieces = profile_pieces(&profile.curve, tolerance)?;
        if pieces
            .iter()
            .any(|c| c.control_points().iter().any(|p| p.weight() <= 0.))
        {
            return Err(invalid(
                "mitered sweep sections require positive rational weights",
            ));
        }
        let profile_controls = pieces
            .iter()
            .map(|c| c.control_points().len())
            .sum::<usize>();
        if segment_count
            .saturating_mul(profile_controls)
            .saturating_mul(4)
            > MAX_SURFACE_CONTROLS
        {
            return Err(invalid("mitered sweep control budget exceeded"));
        }

        let mut queries = stations.clone();
        let anchor = queries
            .binary_search_by(|t| t.total_cmp(&station))
            .unwrap_or_else(|index| {
                queries.insert(index, station);
                index
            });
        let frames = match style {
            SweepFrameStyle::Freeform => {
                rail.rotation_minimizing_frames(&queries, None, Default::default())?
            }
            SweepFrameStyle::Roadlike(axis) => axis_frames(rail, &queries, |_| Ok(axis))?,
        };
        let reference = frames[anchor];
        let mut corners = Vec::with_capacity(segment_count);
        for &t in &stations[..segment_count] {
            let index = queries
                .binary_search_by(|value| value.total_cmp(&t))
                .unwrap();
            corners.push(frames[index]);
        }
        let finish = rail.evaluate(end)?;
        let closed_profile = profile.curve.is_closed()?;
        let stitch_rail_seam = closed && !untrimmed;
        let mut joins = Vec::new();
        let mut miter_joins = Vec::new();
        let mut panels: Vec<(Brep, [usize; 4])> = Vec::new();
        let mut edge_count = 0;
        let mut distance = 0.;
        let width = pieces.len();
        for i in 0..segment_count {
            let frame = corners[i];
            let (previous, next, end_point) = (
                i.checked_sub(1)
                    .map(|j| corners[j])
                    .or(stitch_rail_seam.then_some(corners[segment_count - 1])),
                corners
                    .get(i + 1)
                    .copied()
                    .or(stitch_rail_seam.then_some(corners[0])),
                corners.get(i + 1).map_or(finish, |next| next.origin()),
            );
            let tangent = frame.z_axis().as_vector();
            let start_normal = previous.map(|other| bisector(other, frame)).transpose()?;
            let end_normal = next.map(|other| bisector(frame, other)).transpose()?;
            let length = frame.origin().vector_to(end_point)?.dot(tangent)?;
            for (j, piece) in pieces.iter().enumerate() {
                let posed = pose_profile(piece, reference, frame, untrimmed, tolerance)?;
                let part = panel::build(
                    &posed,
                    tangent,
                    [frame.origin(), end_point],
                    [start_normal, end_normal],
                    distance,
                    length,
                    untrimmed,
                    tolerance,
                )?;
                let panel::Panel {
                    brep,
                    start,
                    end,
                    south,
                    north,
                } = part;
                let index = panels.len();
                let edges = [start, end, south, north].map(|edge| edge + edge_count);
                if i > 0 {
                    let previous: &[usize; 4] = &panels[index - width].1;
                    miter_joins.push((previous[1], edges[0], false));
                }
                if stitch_rail_seam && i + 1 == segment_count {
                    miter_joins.push((edges[1], panels[j].1[0], false));
                }
                if j > 0 {
                    joins.push((panels[index - 1].1[3], edges[2], false));
                }
                if closed_profile && width > 1 && j + 1 == width {
                    joins.push((edges[3], panels[index + 1 - width].1[2], false));
                }
                edge_count += brep.edges().len();
                panels.push((brep, edges));
            }
            distance += length;
        }
        let panels = panels.into_iter().map(|(brep, _)| brep).collect();
        let first_miter = joins.len();
        joins.extend(miter_joins);
        let result = Brep::try_combine(panels, tolerance)?.try_join_edge_pairs(
            &joins,
            tolerance.absolute(),
            tolerance,
        )?;
        if untrimmed {
            Ok(Some(result))
        } else {
            let result = split_miter_rims(result, &joins, first_miter, tolerance);
            Ok(Some(result.partition_sweep_extensions(tolerance)))
        }
    }
}

fn split_miter_rims(
    brep: Brep,
    joins: &[(usize, usize, bool)],
    first_miter: usize,
    tolerance: Tolerance,
) -> Brep {
    // The join retains edge order, omitting the second edge of each pair.
    let mut removed = joins.iter().map(|&(_, edge, _)| edge).collect::<Vec<_>>();
    removed.sort_unstable();
    let mut splits = Vec::new();
    for &(edge, _, _) in &joins[first_miter..] {
        let index = edge - removed.partition_point(|&old| old < edge);
        let parameters = brep.edges()[index]
            .curve()
            .interior_knot_groups()
            .into_iter()
            .map(|(knot, _)| knot)
            .collect::<Vec<_>>();
        if !parameters.is_empty() {
            splits.push((index, parameters));
        }
    }
    if splits.is_empty() {
        brep
    } else {
        brep.try_split_edges_at_parameters(&splits, tolerance)
            .unwrap_or(brep)
    }
}

/// Raised ruled panel between two sections in the same rational basis.
pub(super) fn surface_between(
    [start, end]: &[NurbsCurve; 2],
    u: [Real; 2],
) -> Result<NurbsSurface, GeometryError> {
    let mut controls = Vec::with_capacity(4 * start.control_points().len());
    for (a, b) in start.control_points().iter().zip(end.control_points()) {
        controls.push(*a);
        for fraction in [1. / 3., 2. / 3.] {
            let p = a
                .point()
                .translated(a.point().vector_to(b.point())?.scaled(fraction)?)?;
            controls.push(WeightedPoint3::try_new(p, a.weight())?);
        }
        controls.push(*b);
    }
    NurbsSurface::try_new_rational(
        3,
        start.degree(),
        4,
        start.control_points().len(),
        controls,
        u.into_iter().flat_map(|t| [t; 4]).collect(),
        start.knots().to_vec(),
    )
}

fn split_at_corners(
    curve: &NurbsCurve,
    start: Real,
    end: Real,
    tolerance: Tolerance,
) -> Result<Vec<Real>, GeometryError> {
    let mut stations = vec![start];
    for (knot, multiplicity) in curve.interior_knot_groups() {
        if start < knot
            && knot < end
            && multiplicity == curve.degree()
            && curve.kink_angle_at(knot)? > tolerance.angular()
        {
            stations.push(knot);
        }
    }
    stations.push(end);
    Ok(stations)
}

fn profile_pieces(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let domain = curve.domain();
    split_at_corners(curve, *domain.start(), *domain.end(), tolerance)?
        .windows(2)
        .map(|pair| {
            curve
                .try_trimmed(pair[0]..=pair[1])?
                .clamped_to_active_domain()
        })
        .collect()
}

fn bisector(incoming: Frame3, outgoing: Frame3) -> Result<UnitVector3, GeometryError> {
    let a = incoming.z_axis().as_vector();
    let b = outgoing.z_axis().as_vector();
    let sum = Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())?;
    if sum.length()? <= 64. * Real::EPSILON {
        return Err(invalid("a reversing rail has no miter plane"));
    }
    sum.normalized_nonzero()
}

fn pose_profile(
    curve: &NurbsCurve,
    authored: Frame3,
    frame: Frame3,
    untrimmed: bool,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let controls = curve
        .control_points()
        .iter()
        .map(|control| {
            let displacement = authored.origin().vector_to(control.point())?;
            let [x, y, z] = authored
                .axes()
                .map(|axis| displacement.dot(axis.as_vector()));
            let z = z?;
            if untrimmed && z.abs() > tolerance.absolute() {
                return Err(invalid(
                    "untrimmed polyline miters require a perpendicular profile",
                ));
            }
            WeightedPoint3::try_new(frame.point_at([x?, y?, z])?, control.weight())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NurbsCurve::try_new_rational(
        curve.degree(),
        controls,
        curve.knots().to_vec(),
    )?)
}
