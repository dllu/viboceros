//! Two-rail construction through the native rail-constrained skin producer.
use super::*;
use viboceros_geometry::{AffineTransform3, NurbsCurve2, Point2, SweepEndPoints};

mod matching;
pub use matching::{TwoRailEdgeSupport, TwoRailSweepMatching};
mod placement;
mod refit;
mod transport;
pub use placement::two_rail_point_stations;
use placement::{Placement, prepare_placement};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TwoRailSweepOptions<'a> {
    pub ends: SweepEndPoints,
    pub maintain_height: bool,
    pub refit_rails: bool,
    /// Return from the last profile to the first when both rails are closed.
    pub closed: bool,
    /// One point on each rail, in rail selection order.
    pub slashes: &'a [[Point3; 2]],
}

pub const MAX_TWO_RAIL_SLASHES: usize = 32;

#[derive(Clone)]
struct Section {
    stations: [f64; 2],
    curve: NurbsCurve,
    original: NurbsCurve,
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

/// Sweep 1–32 sections between two rails. Multiple sections
/// bound the retained portion by default. One section on closed rails sweeps
/// the entire loop. The rail and section images are certified within tolerance.
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

/// Return the retained rails for slash picking and validate any existing
/// pairs using the same contact checks as the sweep builder.
pub fn two_rail_slash_paths(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    mut options: TwoRailSweepOptions<'_>,
    tolerance: Tolerance,
) -> Result<[NurbsCurve; 2], Error> {
    options.refit_rails = true;
    let placement = prepare_placement(first, second, sections, options, false, tolerance)?;
    Ok(placement.rails.map(|rail| rail.curve))
}

/// Sweep open profiles along two rails, honoring optional slash alignments.
pub fn two_rail_sweep_surface_with_options(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    options: TwoRailSweepOptions<'_>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    sweep(first, second, sections, options, None, tolerance)
}

/// Adjust one or both supporting-face derivative strips while retaining rail positions.
pub fn two_rail_sweep_surface_matched(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    options: TwoRailSweepOptions<'_>,
    matching: TwoRailSweepMatching<'_>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    sweep(first, second, sections, options, Some(matching), tolerance)
}

fn sweep(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    mut options: TwoRailSweepOptions<'_>,
    mut matching: Option<TwoRailSweepMatching<'_>>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    if matching.is_some() {
        for section in sections {
            if section.is_closed()? || section.is_rational() {
                return Err(unsupported(
                    "surface-edge continuity uses open polynomial cross sections",
                ));
            }
        }
        options.refit_rails |= !first.is_closed()? && (first.is_rational() || second.is_rational());
    }
    let Placement {
        rails: prepared,
        mut rows,
        paired_stations,
        origin,
        to_local,
        refit_rails,
        closed_rails,
        closed_loop,
        closed_profiles,
    } = prepare_placement(
        first,
        second,
        sections,
        options,
        matching.is_some(),
        tolerance,
    )?;
    let TwoRailSweepOptions {
        ends,
        maintain_height,
        ..
    } = options;
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
    let transported = closed_rails || closed_profiles || matching.is_some();
    let exact = if transported && !refit_rails {
        transport::sweep_on_rail_basis(
            rails,
            &rows,
            maintain_height,
            closed_profiles,
            matching.is_some(),
        )?
    } else {
        None
    };
    let row_stations = || rows.iter().map(|row| row.stations[0]).collect();
    let (surface, stations, clock) = if let Some(surface) = exact {
        (surface, row_stations(), None)
    } else if refit_rails || transported {
        let fitted = refit::sweep(
            rails,
            &rows,
            &paired_stations,
            maintain_height,
            closed_profiles,
            tolerance,
        )?;
        (fitted.surface, fitted.stations, Some(fitted.clock))
    } else {
        let surface = if maintain_height || rows.len() == 1 || (affine_rails && interior_profiles) {
            transport::sweep(rails, &rows, maintain_height)?
        } else {
            skin(rails, &rows)?
        };
        (surface, row_stations(), None)
    };
    let world_offset = Vector3::try_from(origin.to_array())?;
    if closed_loop && let Some(matching) = &mut matching {
        matching.adjust_ends[1] = matching.adjust_ends[0];
    }
    let active_matching = matching.and_then(|matching| {
        let orders = matching.orders(surface.degree_v(), surface.control_point_count_v());
        (orders != [0; 2]).then_some((matching, orders))
    });
    let surface = if let Some((matching, orders)) = active_matching {
        matching::apply(
            surface,
            matching,
            orders,
            &stations,
            world_offset,
            tolerance,
        )?
    } else {
        surface
    };
    let mut surface = surface.transformed(AffineTransform3::from_translation(world_offset))?;
    if !ends.is_empty() && surface.degree_u() < 3 {
        surface = super::change_surface_degree(&surface, 3, surface.degree_v(), false)?;
    }
    if !ends.is_empty() {
        surface = exact_pole_rows(surface, ends, tolerance)?;
    }
    if closed_loop {
        surface = close_seam(surface, tolerance)?;
    }
    if closed_profiles && !surface.is_closed_v()? {
        return Err(unsupported("the profile seam could not be joined"));
    }
    let u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let v = [*surface.domain_v().start(), *surface.domain_v().end()];
    let mut constraints = Vec::new();
    for (i, (rail, v)) in prepared
        .iter()
        .map(|rail| &rail.original)
        .zip(v)
        .enumerate()
        .filter(|_| !closed_profiles)
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
        if active_matching.is_some_and(|(matching, _)| matching.adjusts(station))
            && row
                .original
                .control_points()
                .iter()
                .any(|control| control.point() != row.original.control_points()[0].point())
        {
            continue;
        }
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

fn close_seam(surface: NurbsSurface, tolerance: Tolerance) -> Result<NurbsSurface, Error> {
    let count_u = surface.control_point_count_u();
    let mut controls = surface.control_points().to_vec();
    for row in controls.chunks_exact_mut(count_u) {
        let first = row[0].point();
        let last = row.last_mut().unwrap();
        if first.distance_to(last.point())? > tolerance.absolute() {
            return Err(unsupported(
                "the closed sweep's seam did not meet within tolerance",
            ));
        }
        *last = WeightedPoint3::try_new(first, last.weight())?;
    }
    let surface = NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        count_u,
        surface.control_point_count_v(),
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )?;
    if !surface.is_closed_u()? {
        return Err(unsupported("the sweep's seam could not be joined"));
    }
    Ok(surface)
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
