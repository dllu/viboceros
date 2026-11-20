//! Local section blending across straight rail segments with authored corner profiles.
use super::*;
use crate::{Brep, SurfaceSurfaceIntersectionProposal};

mod inferred;
mod seam;

#[derive(Clone, Copy, Debug, Default)]
pub struct SweepProfileMiterOptions {
    pub ends: SweepEndPoints,
    pub style: SweepFrameStyle,
    pub blend: SweepBlend,
    pub closed: bool,
    pub untrimmed: bool,
}

impl Sweep1 {
    /// Sweep a polygonal interval with shared cross sections at the active corners.
    /// Return None when the interval can use the ordinary smooth-rail path.
    pub fn try_profile_miters_with<E: From<GeometryError>>(
        rail: CurveRef<'_>,
        sections: &[SweepSection],
        options: SweepProfileMiterOptions,
        tolerance: Tolerance,
        prepare: &mut impl FnMut(&[NurbsCurve], usize, GeometryError) -> Result<Vec<NurbsCurve>, E>,
        operations: &mut SweepSurfaceOperations<'_, E>,
        intersect: &mut impl FnMut(
            &NurbsSurface,
            &NurbsSurface,
            Tolerance,
        ) -> Result<Vec<SurfaceSurfaceIntersectionProposal>, E>,
    ) -> Result<Option<Brep>, E> {
        let SweepProfileMiterOptions {
            ends,
            style,
            blend,
            closed,
            untrimmed,
        } = options;
        if !matches!(rail, CurveRef::Polyline(_))
            && !matches!(rail, CurveRef::NurbsCurve(curve) if curve.degree() == 1)
        {
            return Ok(None);
        }
        let pole_count = usize::from(ends.start.is_some()) + usize::from(ends.end.is_some());
        if sections.len() + pole_count < 2 {
            return Ok(None);
        }
        let source = rail.to_nurbs()?;
        let domain = source.domain();
        let mut kinks = source.full_multiplicity_kink_parameters(tolerance)?;
        if kinks.is_empty() || sections.is_empty() {
            return Ok(None);
        }
        let span = domain.end() - domain.start();
        let parameters = sections
            .iter()
            .map(|section| {
                let t = section.parameter;
                let endpoint = t.clamp(*domain.start(), *domain.end());
                let t = if (t - endpoint).abs() <= 8. * Real::EPSILON * span {
                    endpoint
                } else {
                    t
                };
                let t = rail.nurbs_parameter(t)?;
                Ok(snap_to_kink(t, &kinks, 64. * Real::EPSILON * span))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let first = parameters[0];
        let last = *parameters.last().unwrap();
        let start = ends.start.map_or(first, |_| *domain.start());
        let finish = if closed || ends.end.is_some() {
            *domain.end()
        } else {
            last
        };
        kinks.retain(|&t| start < t && t < finish);
        if kinks.is_empty() {
            return Ok(None);
        }
        if parameters.windows(2).any(|p| p[0] >= p[1])
            || (ends.start.is_some() && first <= *domain.start())
            || (ends.end.is_some() && last >= *domain.end())
        {
            return Err(invalid(
                "section rail parameters must be strictly increasing and inside point ends",
            )
            .into());
        }
        if closed
            && (!ends.is_empty()
                || !source.is_closed()?
                || first != *domain.start()
                || last >= finish)
        {
            return Err(invalid("a profiled miter loop requires ordered profiles from the rail seam without point ends").into());
        }
        if sections.len() + pole_count > MAX_SECTIONS {
            return Err(invalid("mitered sweep section budget exceeded").into());
        }
        let hinges = kinks
            .iter()
            .map(|t| parameters.binary_search_by(|p| p.total_cmp(t)).ok())
            .collect::<Vec<_>>();
        let inferred = hinges.iter().any(Option::is_none);
        if blend == SweepBlend::Global && (!inferred || closed || sections.len() + pole_count != 2)
        {
            return Err(invalid(
                "Global blending for polygonal miters requires two profiles bracketing the corners",
            )
            .into());
        }
        let curves = sections.iter().map(|s| s.curve.clone()).collect::<Vec<_>>();
        if curves
            .iter()
            .flat_map(NurbsCurve::control_points)
            .any(|c| c.weight() <= 0.)
        {
            return Err(invalid("sweep sections require positive rational weights").into());
        }
        let curves = crate::loft::prepare_section_regions(&curves, &mut |curve| {
            curve.clamped_to_active_domain()
        })?;
        let curves = prepare(
            &curves,
            MAX_SECTION_CONTROLS,
            invalid("section control budget exceeded"),
        )?;
        if curves.len() != sections.len()
            || curves.iter().any(|curve| {
                curve.degree() != curves[0].degree()
                    || curve.knots() != curves[0].knots()
                    || curve.control_points().len() > MAX_SECTION_CONTROLS
            })
        {
            return Err(invalid("section preparation produced inconsistent bases").into());
        }
        let mut sections = parameters
            .into_iter()
            .zip(curves)
            .map(|(parameter, curve)| SweepSection { parameter, curve })
            .collect::<Vec<_>>();
        let seam = if closed {
            let seam = seam::LoopSeam::try_new(&source, &sections[0].curve, style, tolerance)?;
            sections.push(SweepSection {
                parameter: finish,
                curve: seam.last.clone(),
            });
            Some(seam)
        } else {
            None
        };
        if inferred {
            let model = inferred::Model::new(
                &source,
                start..=finish,
                &kinks,
                &hinges,
                sections,
                options,
                tolerance,
            )?;
            if untrimmed {
                return Ok(Some(join_panels(&model.fit(operations, true)?, tolerance)?));
            }
            if !closed
                && ends.is_empty()
                && let Some(profile) = model.constant_profile()?
            {
                return Ok(Self::try_polyline_miters(
                    CurveRef::NurbsCurve(&source),
                    profile,
                    start..=finish,
                    style,
                    false,
                    tolerance,
                )?);
            }
            let panels = model.fit(operations, false)?;
            return inferred::trimmed::sew(panels, &hinges, seam.is_some(), tolerance, intersect)
                .map(Some);
        }
        let mut panels = Vec::with_capacity(kinks.len() + 3);
        let mut first = 0;
        let mut rail_start = start;
        let mut length = 0.;
        let mut count = 1;
        for (last, rail_end) in hinges
            .into_iter()
            .flatten()
            .zip(kinks)
            .chain([(sections.len() - 1, finish)])
        {
            let leg = source.try_trimmed(rail_start..=rail_end)?;
            let leg_ends = SweepEndPoints {
                start: ends.start.filter(|_| rail_start == start),
                end: ends.end.filter(|_| rail_end == finish),
            };
            let sweep = Self::try_new_with_point_ends_and_section_preparation(
                CurveRef::NurbsCurve(&leg),
                &sections[first..=last],
                leg_ends,
                style,
                blend,
                tolerance,
                &mut |curves, _, _| Ok::<_, E>(curves.to_vec()),
            )?;
            let panel = sweep.to_surface_with_operations(operations)?;
            count += panel.control_point_count_u() - 1;
            check_control_budget(count, panel.control_point_count_v())?;
            let end = length + (panel.domain_u().end() - panel.domain_u().start());
            panels.push(panel.try_reparameterized(length..=end, panel.domain_v())?);
            length = end;
            first = last;
            rail_start = rail_end;
        }
        if !untrimmed && let Some(seam) = seam {
            let [start, finish] = seam.extensions(&panels[0], panels.last().unwrap(), tolerance)?;
            count += start.control_point_count_u() + finish.control_point_count_u() - 2;
            check_control_budget(count, start.control_point_count_v())?;
            panels.insert(0, start);
            panels.push(finish);
        }
        Ok(Some(join_panels(&panels, tolerance)?))
    }
}

/// Use the diagonal of the combined one-sided section bounds.
fn extension_distance(a: &NurbsCurve, b: &NurbsCurve) -> Result<Real, GeometryError> {
    let bounds = a.control_point_bounds().union(b.control_point_bounds())?;
    bounds.min().distance_to(bounds.max())
}

// Profile contact can land a few ulps to either side of the true corner.
fn snap_to_kink(t: Real, kinks: &[Real], roundoff: Real) -> Real {
    let Err(next) = kinks.binary_search_by(|knot| knot.total_cmp(&t)) else {
        return t;
    };
    kinks
        .get(next)
        .into_iter()
        .chain(next.checked_sub(1).and_then(|i| kinks.get(i)))
        .copied()
        .min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs()))
        .filter(|&knot| (knot - t).abs() <= roundoff)
        .unwrap_or(t)
}

fn join_panels(panels: &[NurbsSurface], tolerance: Tolerance) -> Result<Brep, GeometryError> {
    let first = &panels[0];
    let p = first.degree_u();
    let width = first.control_point_count_v();
    let count = 1 + panels
        .iter()
        .map(|panel| panel.control_point_count_u() - 1)
        .sum::<usize>();
    check_control_budget(count, width)?;
    let mut knots = Vec::with_capacity(count + p + 1);
    for (i, panel) in panels.iter().enumerate() {
        if panel.degree_u() != p
            || panel.degree_v() != first.degree_v()
            || panel.knots_v() != first.knots_v()
            || panel.control_point_count_v() != width
        {
            return Err(invalid(
                "mitered sweep panels lost their common section basis",
            ));
        }
        // A shared endpoint control and a degree-multiple knot preserve both
        // sides of the C0 joint in one surface before building the BRep grid.
        let start = if i == 0 { 0 } else { p + 1 };
        let end = panel.knots_u().len() - usize::from(i + 1 < panels.len());
        knots.extend_from_slice(&panel.knots_u()[start..end]);
    }
    let mut controls = Vec::with_capacity(count * width);
    for v in 0..width {
        for (i, panel) in panels.iter().enumerate() {
            let n = panel.control_point_count_u();
            let row = &panel.control_points()[v * n..(v + 1) * n];
            if i > 0 && controls.last() != Some(&row[0]) {
                return Err(invalid(
                    "mitered sweep panels lost the shared corner section",
                ));
            }
            controls.extend_from_slice(&row[usize::from(i > 0)..]);
        }
    }
    let surface = NurbsSurface::try_new_rational(
        p,
        first.degree_v(),
        count,
        width,
        controls,
        knots,
        first.knots_v().to_vec(),
    )?;
    let [_, v] = surface.sampled_kink_parameters(tolerance.angular())?;
    let cuts = panels[1..]
        .iter()
        .map(|panel| *panel.domain_u().start())
        .collect::<Vec<_>>();
    Brep::try_surface_grid(&surface, &cuts, &v, tolerance)
}

fn check_control_budget(u: usize, v: usize) -> Result<(), GeometryError> {
    if u > MAX_AXIS_CONTROLS || u * v > MAX_SURFACE_CONTROLS {
        return Err(invalid("mitered sweep control budget exceeded"));
    }
    Ok(())
}
