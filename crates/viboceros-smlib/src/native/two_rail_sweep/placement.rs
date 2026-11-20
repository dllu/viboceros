//! Orient and trim both rails, then validate profile and slash contacts.
use super::{MAX_TWO_RAIL_SLASHES, Section, TwoRailSweepOptions, unsupported};
use crate::native::{
    Error,
    knot_refinement::{clamp_curve, trim_curve},
};
use viboceros_geometry::{
    AffineTransform3, NurbsCurve, Point3, SweepEndPoints, Tolerance, WeightedPoint3,
};

mod closed;
mod loop_contacts;

pub(super) struct PreparedRail {
    pub(super) curve: NurbsCurve,
    pub(super) original: NurbsCurve,
}

impl PreparedRail {
    fn reverse(&mut self, rows: &mut [Section], axis: usize) -> Result<(), Error> {
        self.curve = self.curve.reversed()?.try_reparameterized(0. ..=1.)?;
        self.original = self.original.reversed()?;
        for row in rows {
            row.stations[axis] = 1. - row.stations[axis];
        }
        Ok(())
    }

    fn trim(&mut self, [start, end]: [f64; 2]) -> Result<(), Error> {
        if [start, end] == [0., 1.] {
            return Ok(());
        }
        let domain = self.original.domain();
        let at = |t: f64| {
            if t == 0. {
                *domain.start()
            } else if t == 1. {
                *domain.end()
            } else {
                (1. - t).mul_add(*domain.start(), t * domain.end())
            }
        };
        self.original = trim_curve(&self.original, at(start)..=at(end))?;
        self.curve = trim_curve(&self.curve, start..=end)?.try_reparameterized(0. ..=1.)?;
        Ok(())
    }
}

pub(super) struct Placement {
    pub(super) rails: [PreparedRail; 2],
    pub(super) rows: Vec<Section>,
    pub(super) paired_stations: Vec<[f64; 2]>,
    pub(super) origin: Point3,
    pub(super) to_local: AffineTransform3,
    pub(super) refit_rails: bool,
    pub(super) closed_rails: bool,
    pub(super) closed_loop: bool,
    pub(super) closed_profiles: bool,
}

pub(super) fn prepare_placement(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    options: TwoRailSweepOptions<'_>,
    tolerance: Tolerance,
) -> Result<Placement, Error> {
    let ends = options.ends;
    let closed_rails = match [first.is_closed()?, second.is_closed()?] {
        [true, true] if ends.is_empty() => true,
        [false, false] if !options.closed => false,
        _ => {
            return Err(unsupported(
                "a closed sweep needs two closed rails without point ends",
            ));
        }
    };
    if options.slashes.len() > MAX_TWO_RAIL_SLASHES {
        return Err(unsupported("slash count exceeds the limit"));
    }
    let point_count = usize::from(ends.start.is_some()) + usize::from(ends.end.is_some());
    let row_count = sections.len().saturating_add(point_count);
    if sections.is_empty()
        || row_count > 32
        || first.control_points().len()
            + second.control_points().len()
            + sections
                .iter()
                .map(|c| c.control_points().len())
                .sum::<usize>()
            + 2 * point_count
            > 1024
    {
        return Err(unsupported(
            "requires 1–32 total sections including a real profile, and at most 1024 total controls",
        ));
    }
    let closed_profiles = sections[0].is_closed()?;
    for section in &sections[1..] {
        if section.is_closed()? != closed_profiles {
            return Err(unsupported("select either open or closed profiles"));
        }
    }
    let mut prepared = [
        prepare_rail(first, sections, ends, closed_rails, tolerance)?,
        prepare_rail(second, sections, ends, closed_rails, tolerance)?,
    ];
    let origin = prepared[0].curve.evaluate(0.)?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let to_local = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let local_rails = [
        prepared[0].curve.transformed(to_local)?,
        prepared[1].curve.transformed(to_local)?,
    ];
    let mut rows = Vec::with_capacity(row_count);
    let locate = if closed_rails {
        closed::contact
    } else {
        contact
    };
    for section in sections {
        let mut curve = prepare(section, tolerance, closed_profiles, closed_profiles)?;
        let local = curve.transformed(to_local)?;
        if closed_profiles {
            rows.push(Section {
                stations: loop_contacts::stations(&local_rails, &local, tolerance)?,
                curve,
                original: section.clone(),
            });
            continue;
        }
        let endpoints = [local.evaluate(0.)?, local.evaluate(1.)?];
        let mut matches = Vec::new();
        for reverse in [false, true] {
            if let (Some(a), Some(b)) = (
                locate(&local_rails[0], endpoints[usize::from(reverse)], tolerance)?,
                locate(&local_rails[1], endpoints[usize::from(!reverse)], tolerance)?,
            ) {
                matches.push((a, b, reverse));
            }
        }
        let [(a, b, reverse)] = matches.as_slice() else {
            return Err(unsupported(
                "section endpoints must unambiguously contact the two rails",
            ));
        };
        let original = if *reverse {
            curve = curve.reversed()?;
            section.reversed()?
        } else {
            section.clone()
        };
        rows.push(Section {
            stations: [*a, *b],
            curve,
            original,
        });
    }
    for (parameter, point) in [(0., ends.start), (1., ends.end)] {
        if let Some(point) = point {
            let curve = NurbsCurve::try_new(1, vec![point; 2], vec![0., 0., 1., 1.])?;
            rows.push(Section {
                stations: [parameter; 2],
                curve: curve.clone(),
                original: curve,
            });
        }
    }
    let single = rows.len() == 1;
    let closed_loop = closed_rails && (options.closed || single);
    let (partial, refit_closed) = if closed_rails {
        (
            false,
            closed::retain(&mut prepared, &mut rows, closed_loop)?,
        )
    } else {
        (
            retain_sections(&mut prepared, &mut rows, ends.is_empty())?,
            false,
        )
    };
    let independent = rows
        .iter()
        .any(|r| (r.stations[0] - r.stations[1]).abs() > 1e-10);
    // Rhino refits automatically for shortened multi-profile sweeps and
    // whenever exact normalized rail clocks cannot pair the profile contacts.
    let refit_rails = options.refit_rails
        || !options.slashes.is_empty()
        || independent
        || refit_closed
        || (partial && !single && ends.is_empty());
    if !closed_profiles
        && !closed_rails
        && !refit_rails
        && prepared.iter().any(|rail| rail.curve.is_rational())
    {
        return Err(unsupported("rational rails require refitting"));
    }
    let paired_stations = if refit_rails || closed_rails || closed_profiles {
        collect_paired_stations(&prepared, &rows, options.slashes, tolerance)?
    } else {
        Vec::new()
    };
    Ok(Placement {
        rails: prepared,
        rows,
        paired_stations,
        origin,
        to_local,
        refit_rails,
        closed_rails,
        closed_loop,
        closed_profiles,
    })
}

fn collect_paired_stations(
    rails: &[PreparedRail; 2],
    rows: &[Section],
    slashes: &[[Point3; 2]],
    tolerance: Tolerance,
) -> Result<Vec<[f64; 2]>, Error> {
    let mut stations = rows.iter().map(|row| row.stations).collect::<Vec<_>>();
    for points in slashes {
        let locate = |i: usize| {
            contact(&rails[i].curve, points[i], tolerance)?
                .filter(|t| *t > 0. && *t < 1.)
                .ok_or_else(|| unsupported("slash points must lie inside the retained rails"))
        };
        stations.push([locate(0)?, locate(1)?]);
    }
    stations.sort_by(|a, b| a[0].total_cmp(&b[0]));
    if stations[0] != [0.; 2] {
        stations.insert(0, [0.; 2]);
    }
    if stations.last() != Some(&[1.; 2]) {
        stations.push([1.; 2]);
    }
    if stations
        .windows(2)
        .any(|pair| (0..2).any(|i| pair[1][i] - pair[0][i] <= 1e-10))
    {
        return Err(unsupported(
            "slashes must progress in the same order as the profiles on both rails",
        ));
    }
    Ok(stations)
}

fn retain_sections(
    rails: &mut [PreparedRail; 2],
    rows: &mut [Section],
    orient_rails: bool,
) -> Result<bool, Error> {
    if let [row] = rows {
        let [a, b] = row.stations;
        let reverse = match (a, b) {
            (0., 0.) | (1., 1.) => false,
            (0., 1.) | (1., 0.) => true,
            _ => {
                rails[0]
                    .curve
                    .derivative_at(a)?
                    .dot(rails[1].curve.derivative_at(b)?)?
                    < 0.
            }
        };
        if reverse {
            rails[1].reverse(rows, 1)?;
        }
        let [a, b] = rows[0].stations;
        let toward_start = a == 1. || b == 1.;
        let span = |t| if toward_start { [0., t] } else { [t, 1.] };
        rails[0].trim(span(a))?;
        rails[1].trim(span(b))?;
        rows[0].stations = [f64::from(toward_start); 2];
        return Ok(span(a) != [0., 1.] || span(b) != [0., 1.]);
    }

    let a = rows[0].stations;
    let b = rows.last().unwrap().stations;
    let on_ends = (0..2).all(|i| a[i].min(b[i]) == 0. && a[i].max(b[i]) == 1.);
    if orient_rails && !on_ends && rows.windows(2).all(|r| r[0].stations[0] > r[1].stations[0]) {
        // For a deliberately reversed set of partial profiles, begin the
        // shortened sweep at the first profile picked.
        rails[0].reverse(rows, 0)?;
    }
    rows.sort_by(|a, b| a.stations[0].total_cmp(&b.stations[0]));
    if orient_rails && rows[0].stations[1] > rows.last().unwrap().stations[1] {
        rails[1].reverse(rows, 1)?;
    }
    if rows
        .windows(2)
        .any(|pair| (0..2).any(|axis| pair[1].stations[axis] - pair[0].stations[axis] <= 1e-10))
    {
        return Err(unsupported(
            "requires distinct, consistently ordered contacts on both rails",
        ));
    }
    let start = rows[0].stations;
    let end = rows.last().unwrap().stations;
    for axis in 0..2 {
        rails[axis].trim([start[axis], end[axis]])?;
        for row in rows.iter_mut() {
            row.stations[axis] = (row.stations[axis] - start[axis]) / (end[axis] - start[axis]);
        }
    }
    Ok(start != [0.; 2] || end != [1.; 2])
}

/// Locate the supplied point ends on one rail, in its original parameter chart.
pub fn two_rail_point_stations(
    curve: &NurbsCurve,
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<[Option<f64>; 2], Error> {
    let locate = |point: Option<Point3>| -> Result<_, Error> {
        point
            .map(|point| {
                contact(curve, point, tolerance)?
                    .ok_or_else(|| unsupported("point ends must lie on both rails"))
            })
            .transpose()
    };
    let stations = [locate(ends.start)?, locate(ends.end)?];
    if let [Some(start), Some(end)] = stations
        && start == end
    {
        return Err(unsupported("point ends must occupy distinct stations"));
    }
    Ok(stations)
}

fn rail_interval(
    curve: &NurbsCurve,
    sections: &[NurbsCurve],
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<[f64; 2], Error> {
    let [start, end] = two_rail_point_stations(curve, ends, tolerance)?;
    let domain = curve.domain();
    let endpoints = [*domain.start(), *domain.end()];
    let opposite = |station| -> Result<f64, Error> {
        if station == endpoints[0] {
            return Ok(endpoints[1]);
        }
        if station == endpoints[1] {
            return Ok(endpoints[0]);
        }
        // Retain the side of the crossing containing the real profiles.
        // The terminal profile can itself be inside that side of the rail.
        let mut occupied = [false; 2];
        for section in sections {
            if section.is_closed()? {
                let center = section.control_point_bounds().center()?;
                let location = loop_contacts::station(curve, section, center, tolerance)?;
                occupied[0] |= location < station;
                occupied[1] |= location > station;
                continue;
            }
            let section_domain = section.domain();
            for t in [*section_domain.start(), *section_domain.end()] {
                if let Some(location) = contact(curve, section.evaluate(t)?, tolerance)? {
                    occupied[0] |= location < station;
                    occupied[1] |= location > station;
                }
            }
        }
        match occupied {
            [true, false] => Ok(endpoints[0]),
            [false, true] => Ok(endpoints[1]),
            _ => Err(unsupported(
                "cross sections must lie on one side of an interior point end",
            )),
        }
    };
    match (start, end) {
        (Some(start), Some(end)) => Ok([start, end]),
        (Some(start), None) => Ok([start, opposite(start)?]),
        (None, Some(end)) => Ok([opposite(end)?, end]),
        (None, None) => Ok(endpoints),
    }
}

fn prepare_rail(
    curve: &NurbsCurve,
    sections: &[NurbsCurve],
    ends: SweepEndPoints,
    closed: bool,
    tolerance: Tolerance,
) -> Result<PreparedRail, Error> {
    if ends.is_empty() {
        return Ok(PreparedRail {
            curve: prepare(curve, tolerance, true, closed)?,
            original: curve.clone(),
        });
    }
    let [start, end] = rail_interval(curve, sections, ends, tolerance)?;
    let range = start.min(end)..=start.max(end);
    let mut original = if range == curve.domain() {
        curve.clone()
    } else {
        curve.try_trimmed(range)?
    };
    if end < start {
        original = original.reversed()?;
    }
    let prepared = clamp_curve(&prepare(&original, tolerance, true, false)?)?;
    let mut points = prepared.control_points().to_vec();
    if let Some(point) = ends.start {
        points[0] = WeightedPoint3::try_new(point, points[0].weight())?;
    }
    if let Some(point) = ends.end {
        let end = points.last_mut().unwrap();
        *end = WeightedPoint3::try_new(point, end.weight())?;
    }
    Ok(PreparedRail {
        curve: NurbsCurve::try_new_rational(prepared.degree(), points, prepared.knots().to_vec())?,
        original,
    })
}

fn prepare(
    curve: &NurbsCurve,
    tolerance: Tolerance,
    allow_rational: bool,
    allow_closed: bool,
) -> Result<NurbsCurve, Error> {
    if curve.degree() > 16 || curve.control_points().len() > 128 {
        return Err(unsupported(
            "requires degree at most 16 and at most 128 controls per input curve",
        ));
    }
    if !allow_rational && curve.is_rational() {
        return Err(unsupported(
            "requires polynomial input curves outside refitting",
        ));
    }
    if (!allow_closed
        && curve
            .evaluate(*curve.domain().start())?
            .distance_to(curve.evaluate(*curve.domain().end())?)?
            <= tolerance.absolute())
        || curve.knots()[curve.degree() + 1..curve.control_points().len()]
            .windows(curve.degree() + 1)
            .any(|k| k.iter().all(|v| *v == k[0]))
    {
        return Err(unsupported(
            "requires open, continuous, noncollapsed curves",
        ));
    }
    if curve.is_rational() {
        return Ok(curve.try_reparameterized(0. ..=1.)?);
    }
    Ok(NurbsCurve::try_new(
        curve.degree(),
        curve.control_points().iter().map(|p| p.point()).collect(),
        curve.knots().to_vec(),
    )?
    .try_reparameterized(0. ..=1.)?)
}

fn contact(rail: &NurbsCurve, point: Point3, tolerance: Tolerance) -> Result<Option<f64>, Error> {
    let domain = rail.domain();
    for t in [*domain.start(), *domain.end()] {
        if rail.evaluate(t)?.distance_to(point)? <= tolerance.absolute() {
            return Ok(Some(t));
        }
    }
    let t = rail.closest_parameter(point, tolerance)?;
    Ok((rail.evaluate(t)?.distance_to(point)? <= tolerance.absolute()).then_some(t))
}
