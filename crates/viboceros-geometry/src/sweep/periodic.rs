//! Periodic cubic interpolation of the closed arc-length section model.
use super::*;
use crate::curve::ArcLengthSampler;
use crate::nurbs::periodic_greville_parameters;
use crate::spline_collocation::Break;

const DEGREE: usize = 3;

struct Stations<'a> {
    sweep: &'a Sweep1,
    sampler: ArcLengthSampler<'a>,
    profiles: Vec<Real>,
}

impl Stations<'_> {
    fn sections(&self, stations: &[Real]) -> Result<Vec<NurbsCurve>, GeometryError> {
        let parameters = stations
            .iter()
            .map(|&distance| {
                match self
                    .profiles
                    .binary_search_by(|site| site.total_cmp(&distance))
                {
                    Ok(i) => Ok(self.sweep.sections[i].parameter),
                    Err(_) => self.sampler.parameter_at_distance(distance),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.sweep.sections_at_stations(
            &parameters,
            stations,
            &self.profiles,
            SweepBlend::Local,
            BlendCoordinates::Homogeneous,
        )
    }
}

pub(super) fn fit<E: From<GeometryError>>(
    sweep: &Sweep1,
    solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    let rail = sweep.rail.as_ref();
    let mut sampler = ArcLengthSampler::try_new(rail, sweep.tolerance)?;
    sampler.prepare_budgeted_repeated_sampling(32)?;
    let length = sampler.total_length();
    let profiles = sweep
        .sections
        .iter()
        .map(|section| sampler.distance_at_parameter(section.parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let margin = 64. * Real::EPSILON * length;
    if profiles.windows(2).any(|p| p[1] - p[0] <= margin) {
        return Err(invalid("closed sweep needs distinct profile stations").into());
    }
    // Rhino's cyclic refit uses the local profile blend for both settings.
    let multiplicity = 2;
    let mut breaks = profiles
        .iter()
        .map(|&parameter| Break {
            parameter,
            multiplicity,
        })
        .collect::<Vec<_>>();
    let source = rail.to_nurbs()?;
    for (t, _) in source.interior_knot_groups() {
        let distance = sampler.distance_at_parameter(rail.parameter_from_nurbs(t)?)?;
        let i = profiles.partition_point(|&p| p < distance);
        let adjacent = [i.checked_sub(1), Some(i)];
        if !adjacent
            .into_iter()
            .flatten()
            .filter_map(|i| profiles.get(i))
            .any(|p| (p - distance).abs() <= margin)
        {
            breaks.push(Break {
                parameter: distance,
                multiplicity: 2,
            });
        }
    }
    breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    breaks.dedup_by(|a, b| (a.parameter - b.parameter).abs() <= margin);
    let initial = breaks
        .windows(2)
        .map(|pair| Break {
            parameter: pair[0].parameter.midpoint(pair[1].parameter),
            multiplicity: 1,
        })
        .collect::<Vec<_>>();
    breaks.extend(initial);
    breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    let stations = Stations {
        sweep,
        sampler,
        profiles,
    };
    loop {
        let unique = breaks[..breaks.len() - 1]
            .iter()
            .map(|b| b.multiplicity)
            .sum::<usize>();
        if unique <= DEGREE
            || unique + DEGREE > MAX_AXIS_CONTROLS
            || (unique + DEGREE) * sweep.local[0].len() > MAX_SURFACE_CONTROLS
        {
            return Err(invalid("closed sweep refit exceeded its control budget").into());
        }
        let knots = periodic_knots(&breaks, length, unique);
        let (mut sites, shift) =
            periodic_greville_parameters(DEGREE, unique + DEGREE, unique, &knots)?;
        pin_profiles(
            &mut sites,
            &stations.profiles[..stations.profiles.len() - 1],
        )?;
        let sections = stations.sections(&sites)?;
        sites.push(length);
        let surface = interpolate(knots, &sites, &sections, shift, solve)?;
        if surface.control_points().iter().any(|p| p.weight() <= 0.) {
            return Err(invalid("closed sweep refit produced nonpositive weights").into());
        }
        let mut audit = Vec::with_capacity((breaks.len() - 1) * 4);
        for pair in breaks.windows(2) {
            let (a, b) = (pair[0].parameter, pair[1].parameter);
            for fraction in [0., 0.211_324_865_4, 0.5, 0.788_675_134_6] {
                audit.push(a + fraction * (b - a));
            }
        }
        let targets = stations.sections(&audit)?;
        let mut added = Vec::new();
        for (span, pair) in breaks.windows(2).enumerate() {
            let mut refine = false;
            for (t, target) in audit[4 * span..4 * span + 4]
                .iter()
                .zip(&targets[4 * span..4 * span + 4])
            {
                if super::fit::section_error(&surface.isocurve_v(*t)?, target)?
                    > sweep.tolerance.absolute() * 0.8
                {
                    refine = true;
                    break;
                }
            }
            if refine {
                let mid = pair[0].parameter.midpoint(pair[1].parameter);
                if !(pair[0].parameter < mid && mid < pair[1].parameter) {
                    return Err(invalid("closed sweep fit exhausted parameter resolution").into());
                }
                added.push(Break {
                    parameter: mid,
                    multiplicity: 1,
                });
            }
        }
        if added.is_empty() {
            return Ok(surface);
        }
        breaks.extend(added);
        breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    }
}

fn periodic_knots(breaks: &[Break], length: Real, unique: usize) -> Vec<Real> {
    let cycle = breaks[..breaks.len() - 1]
        .iter()
        .flat_map(|b| std::iter::repeat_n(b.parameter, b.multiplicity))
        .collect::<Vec<_>>();
    cycle[unique - DEGREE..]
        .iter()
        .map(|t| t - length)
        .chain(cycle.iter().copied())
        .chain(cycle[..=DEGREE].iter().map(|t| t + length))
        .collect()
}

fn pin_profiles(sites: &mut [Real], profiles: &[Real]) -> Result<(), GeometryError> {
    let mut next = 0;
    for &profile in profiles {
        if next == sites.len() {
            return Err(invalid("closed sweep could not pin all profiles"));
        }
        let upper =
            (next + sites[next..].partition_point(|&site| site < profile)).min(sites.len() - 1);
        let lower = upper.saturating_sub(1).max(next);
        let i = if (sites[lower] - profile).abs() < (sites[upper] - profile).abs() {
            lower
        } else {
            upper
        };
        sites[i] = profile;
        next = i + 1;
    }
    if sites.windows(2).any(|p| p[0] >= p[1]) {
        return Err(invalid("closed sweep could not pin distinct profile sites"));
    }
    Ok(())
}

fn interpolate<E: From<GeometryError>>(
    knots: Vec<Real>,
    parameters: &[Real],
    sections: &[NurbsCurve],
    shift: usize,
    solve: &mut impl FnMut(&crate::NurbsInterpolationSystem<'_>, usize) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    let first = &sections[0];
    let width = first.control_points().len();
    let origin = first.control_points()[0].point().to_array();
    let columns = width * 4;
    let mut targets = Vec::with_capacity(columns * sections.len());
    for control in sections.iter().flat_map(NurbsCurve::control_points) {
        targets.extend(
            control
                .point()
                .to_array()
                .into_iter()
                .zip(origin)
                .map(|(a, b)| (a - b) * control.weight()),
        );
        targets.push(control.weight());
    }
    let system = crate::NurbsInterpolationSystem {
        degree: DEGREE,
        knots: &knots,
        parameters,
        periodic: true,
        columns,
        targets: &targets,
    };
    let solution =
        crate::surface_rebuild::checked_solve(&system, &mut |input| solve(input, shift))?;
    let unique = sections.len();
    let count = unique + DEGREE;
    let mut controls = Vec::with_capacity(count * width);
    for v in 0..width {
        for u in 0..count {
            let start = (u % unique) * columns + v * 4;
            let row = &solution[start..start + 4];
            let point = Point3::try_from(std::array::from_fn(|i| origin[i] + row[i] / row[3]))?;
            controls.push(WeightedPoint3::try_new(point, row[3])?);
        }
    }
    Ok(NurbsSurface::try_new_rational(
        DEGREE,
        first.degree(),
        count,
        width,
        controls,
        knots,
        first.knots().to_vec(),
    )?)
}
