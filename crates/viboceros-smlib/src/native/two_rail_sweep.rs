//! Two-rail construction through the native rail-constrained skin producer.
use super::*;
use viboceros_geometry::{AffineTransform3, NurbsCurve2, Point2, SweepEndPoints};

mod refit;
mod transport;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TwoRailSweepOptions {
    pub ends: SweepEndPoints,
    pub maintain_height: bool,
    pub refit_rails: bool,
}

struct Section {
    stations: [f64; 2],
    curve: NurbsCurve,
    original: NurbsCurve,
}

struct PreparedRail {
    curve: NurbsCurve,
    original: NurbsCurve,
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
        self.original = super::trim_curve(&self.original, at(start)..=at(end))?;
        self.curve = super::trim_curve(&self.curve, start..=end)?.try_reparameterized(0. ..=1.)?;
        Ok(())
    }
}

unsafe extern "C" {
    fn vb_curves_two_rail_sweep(
        first: *const NativeCurve,
        second: *const NativeCurve,
        sections: *const *const NativeCurve,
        count: usize,
        parameters: *const f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

fn unsupported(message: &str) -> Error {
    Error::Kernel(format!("Two-rail sweep: {message}"))
}

fn prepare(
    curve: &NurbsCurve,
    tolerance: Tolerance,
    allow_rational: bool,
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
    if curve
        .evaluate(*curve.domain().start())?
        .distance_to(curve.evaluate(*curve.domain().end())?)?
        <= tolerance.absolute()
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

/// Construct an open two-rail sweep from 1–32 open polynomial sections.
/// With multiple sections, the terminal contacts trim the rails. Refit pairs
/// any remaining independent stations. The rail and profile images are
/// certified within the absolute tolerance.
pub fn two_rail_sweep_surface(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    two_rail_sweep_surface_with_point_ends(
        first,
        second,
        sections,
        SweepEndPoints::default(),
        tolerance,
    )
}

/// Add exact collapsed sections where both rails reach the selected points.
/// Interior point contacts trim private copies. The same continuous
/// certificates compare the output against the original retained intervals.
pub fn two_rail_sweep_surface_with_point_ends(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    two_rail_sweep_surface_with_options(
        first,
        second,
        sections,
        TwoRailSweepOptions {
            ends,
            ..Default::default()
        },
        tolerance,
    )
}

/// Sweep an open profile along two rails, optionally decoupling profile height
/// from rail width. Interpolate supplied profiles and retain both rail images.
pub fn two_rail_sweep_surface_with_options(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    options: TwoRailSweepOptions,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let TwoRailSweepOptions {
        ends,
        maintain_height,
        refit_rails,
    } = options;
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
    let mut prepared = [
        prepare_rail(first, sections, ends, tolerance)?,
        prepare_rail(second, sections, ends, tolerance)?,
    ];
    let origin = prepared[0].curve.evaluate(0.)?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let to_local = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let local_rails = [
        prepared[0].curve.transformed(to_local)?,
        prepared[1].curve.transformed(to_local)?,
    ];
    let mut rows = Vec::with_capacity(row_count);
    for section in sections {
        let mut curve = prepare(section, tolerance, false)?;
        let local = curve.transformed(to_local)?;
        let endpoints = [local.evaluate(0.)?, local.evaluate(1.)?];
        let mut matches = Vec::new();
        for reverse in [false, true] {
            if let (Some(a), Some(b)) = (
                contact(&local_rails[0], endpoints[usize::from(reverse)], tolerance)?,
                contact(&local_rails[1], endpoints[usize::from(!reverse)], tolerance)?,
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
    let partial = retain_sections(&mut prepared, &mut rows, ends.is_empty())?;
    let independent = rows
        .iter()
        .any(|r| (r.stations[0] - r.stations[1]).abs() > 1e-10);
    // Rhino refits automatically for shortened multi-profile sweeps and
    // whenever exact normalized rail clocks cannot pair the profile contacts.
    let refit_rails = refit_rails || independent || (partial && !single && ends.is_empty());
    if !refit_rails && prepared.iter().any(|rail| rail.curve.is_rational()) {
        return Err(unsupported("rational rails require refitting"));
    }
    let rails = [
        prepared[0].curve.transformed(to_local)?,
        prepared[1].curve.transformed(to_local)?,
    ];
    for row in &mut rows {
        row.curve = row.curve.transformed(to_local)?;
    }
    let affine_rails = rails
        .iter()
        .all(|rail| rail.degree() == 1 && rail.control_points().len() == 2);
    let interior_profiles = ends.is_empty() && rows.len() > 2;
    let rails = [&rails[0], &rails[1]];
    let (surface, stations, clock) = if refit_rails {
        let fitted = refit::sweep(rails, &rows, maintain_height, tolerance)?;
        (fitted.surface, fitted.stations, Some(fitted.clock))
    } else {
        let surface = if maintain_height || single || (affine_rails && interior_profiles) {
            transport::sweep(rails, &rows, maintain_height)?
        } else {
            skin(rails, &rows)?
        };
        (
            surface,
            rows.iter().map(|row| row.stations[0]).collect(),
            None,
        )
    };
    let mut surface =
        surface.transformed(AffineTransform3::from_translation(zero.vector_to(origin)?))?;
    if !ends.is_empty() && surface.degree_u() < 3 {
        surface = super::change_surface_degree(&surface, 3, surface.degree_v(), false)?;
    }
    if !ends.is_empty() {
        surface = exact_pole_rows(surface, ends, tolerance)?;
    }
    let u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let v = [*surface.domain_v().start(), *surface.domain_v().end()];
    let mut constraints = Vec::new();
    for (i, (rail, v)) in prepared
        .iter()
        .map(|rail| &rail.original)
        .zip(v)
        .enumerate()
    {
        if let Some(clock) = &clock {
            let edge = surface.isocurve_u(v)?;
            if !clock.certify_rail(i, rail, &edge, tolerance.absolute())? {
                return Err(unsupported(
                    "could not certify the refitted rail within tolerance",
                ));
            }
        } else {
            constraints.push((rail, [[u[0], v], [u[1], v]]));
        }
    }
    for (row, station) in rows.iter().zip(stations) {
        let s = u[0] + station * (u[1] - u[0]);
        constraints.push((&row.original, [[s, v[0]], [s, v[1]]]));
    }
    for (curve, endpoints) in constraints {
        let domain = curve.domain();
        let (a, b) = (*domain.start(), *domain.end());
        let uv = NurbsCurve2::try_new(
            1,
            vec![
                Point2::try_from(endpoints[0])?,
                Point2::try_from(endpoints[1])?,
            ],
            vec![a, a, b, b],
        )?;
        if surface
            .parameter_curve_deviation_bound(&uv, curve, tolerance.absolute())?
            .is_none()
        {
            return Err(unsupported(
                "native output could not certify a rail or section within tolerance",
            ));
        }
    }
    Ok(surface)
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

fn skin(rails: [&NurbsCurve; 2], rows: &[Section]) -> Result<NurbsSurface, Error> {
    if rails.iter().any(|rail| rail.degree() >= rows.len()) {
        return Err(unsupported(
            "requires at least maximum rail degree plus one sections",
        ));
    }
    let native = rails
        .into_iter()
        .chain(rows.iter().map(|row| &row.curve))
        .map(KernelCurve::from_nurbs_for_modeling)
        .collect::<Result<Vec<_>, Error>>()?;
    let handles = native[2..]
        .iter()
        .map(|c| c.handle.as_ptr().cast_const())
        .collect::<Vec<_>>();
    let parameters = rows.iter().map(|row| row.stations[0]).collect::<Vec<_>>();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curves_two_rail_sweep(
            native[0].handle.as_ptr(),
            native[1].handle.as_ptr(),
            handles.as_ptr(),
            handles.len(),
            parameters.as_ptr(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    brep::surface_from_native(pointer)
}

fn exact_pole_rows(
    surface: NurbsSurface,
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let u_count = surface.control_point_count_u();
    let mut controls = surface.control_points().to_vec();
    for row in controls.chunks_exact_mut(u_count) {
        for (i, pole) in [(0, ends.start), (u_count - 1, ends.end)] {
            if let Some(point) = pole {
                if row[i].point().distance_to(point)? > tolerance.absolute() {
                    return Err(unsupported("native output did not preserve the point end"));
                }
                // Restoring the world transform can round the point. Make the
                // whole row identical before certifying the surface and its rails.
                row[i] = WeightedPoint3::try_new(point, row[i].weight())?;
            }
        }
    }
    Ok(NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        u_count,
        surface.control_point_count_v(),
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )?)
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
        // With an interior pole, the remaining real terminal section chooses
        // which side of the crossing to keep. Reject picks that straddle it.
        let world = [curve.evaluate(endpoints[0])?, curve.evaluate(endpoints[1])?];
        let mut contacted = [false; 2];
        for section in sections {
            let section_domain = section.domain();
            for t in [*section_domain.start(), *section_domain.end()] {
                let point = section.evaluate(t)?;
                for i in 0..2 {
                    contacted[i] |= world[i].distance_to(point)? <= tolerance.absolute();
                }
            }
        }
        match contacted {
            [true, false] => Ok(endpoints[0]),
            [false, true] => Ok(endpoints[1]),
            _ => Err(unsupported(
                "an interior point needs a cross section at the remaining rail end",
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
    tolerance: Tolerance,
) -> Result<PreparedRail, Error> {
    if ends.is_empty() {
        return Ok(PreparedRail {
            curve: prepare(curve, tolerance, true)?,
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
    let prepared = super::knot_refinement::clamp_curve(&prepare(&original, tolerance, true)?)?;
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
