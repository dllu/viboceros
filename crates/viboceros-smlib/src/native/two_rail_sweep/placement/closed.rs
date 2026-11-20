use super::{Error, NurbsCurve, Point3, PreparedRail, Section, Tolerance, unsupported};
use crate::native::change_curve_seam;
use viboceros_geometry::Vector3;

const STATION_EPSILON: f64 = 1e-10;

pub(super) fn contact(
    rail: &NurbsCurve,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Option<f64>, Error> {
    let Some(station) = super::contact(rail, point, tolerance)? else {
        return Ok(None);
    };
    for &knot in &rail.knots()[rail.degree()..=rail.control_points().len()] {
        if (knot - station).abs() <= STATION_EPSILON
            && rail.evaluate(knot)?.distance_to(point)? <= tolerance.absolute()
        {
            return Ok(Some(knot));
        }
    }
    Ok(Some(station))
}

fn phase(station: f64, seam: f64) -> f64 {
    (station - seam).rem_euclid(1.)
}

fn ordered(phases: &[f64]) -> bool {
    phases.windows(2).all(|p| p[1] - p[0] > STATION_EPSILON)
        && *phases.last().unwrap() < 1. - STATION_EPSILON
}

fn direction(rows: &[Section], axis: usize) -> [Vec<f64>; 2] {
    let seam = rows[0].stations[axis];
    [
        rows.iter()
            .map(|row| phase(row.stations[axis], seam))
            .collect(),
        rows.iter()
            .map(|row| phase(seam, row.stations[axis]))
            .collect(),
    ]
}

fn seam_tangent(rail: &NurbsCurve, station: f64) -> Result<Vector3, Error> {
    // A seam at a corner has two tangents. Compare their bisector so the
    // result does not depend on which side an authored curve evaluates first.
    let step = f64::EPSILON.sqrt();
    let unit = |t| -> Result<Vector3, Error> {
        let tangent = rail.derivative_at(phase(t, 0.))?;
        Ok(tangent
            .normalized_nonzero()
            .map_or(tangent, |v| v.as_vector()))
    };
    let a = unit(station - step)?;
    let b = unit(station + step)?;
    Ok(Vector3::try_new(
        a.x() + b.x(),
        a.y() + b.y(),
        a.z() + b.z(),
    )?)
}

pub(super) fn retain(
    rails: &mut [PreparedRail; 2],
    rows: &mut Vec<Section>,
    closed_loop: bool,
) -> Result<bool, Error> {
    let invalid =
        || unsupported("closed-rail profile contacts must progress around both rails in order");
    let first = direction(rows, 0);
    // With two profiles the first rail determines which of the two arcs to retain.
    let reverse_first = !ordered(&first[0]);
    let first = &first[usize::from(reverse_first)];
    if !ordered(first) {
        return Err(invalid());
    }
    let second = direction(rows, 1);
    let reverse_second = match [ordered(&second[0]), ordered(&second[1])] {
        [true, false] => false,
        [false, true] => true,
        [false, false] => return Err(invalid()),
        [true, true] => {
            // Curve parameter speeds can differ even for identical loops.
            // Use the tangents at the picked sections to pair the directions.
            let mut alignment = 0.;
            for row in rows.iter() {
                let [a, b] = row.stations;
                alignment +=
                    seam_tangent(&rails[0].curve, a)?.dot(seam_tangent(&rails[1].curve, b)?)?;
            }
            if alignment.abs() > STATION_EPSILON {
                if reverse_first {
                    alignment > 0.
                } else {
                    alignment < 0.
                }
            } else {
                let mismatch = |other: &[f64]| {
                    first
                        .iter()
                        .zip(other)
                        .map(|(a, b)| (a - b).abs())
                        .sum::<f64>()
                };
                mismatch(&second[0]) > mismatch(&second[1])
            }
        }
    };

    let mut refit = false;
    for (axis, reverse) in [reverse_first, reverse_second].into_iter().enumerate() {
        let rail = &mut rails[axis];
        refit |= rows
            .iter()
            .any(|row| !rail.curve.knots().contains(&row.stations[axis]));
        if reverse {
            rail.reverse(rows, axis)?;
        }
        let seam = rows[0].stations[axis];
        if seam > 0. && seam < 1. {
            let domain = rail.original.domain();
            let parameter = rail
                .curve
                .knots()
                .iter()
                .position(|&knot| knot == seam)
                .map_or_else(
                    || (1. - seam).mul_add(*domain.start(), seam * domain.end()),
                    |i| rail.original.knots()[i],
                );
            rail.original = change_curve_seam(&rail.original, parameter)?;
            rail.curve = change_curve_seam(&rail.curve, seam)?.try_reparameterized(0. ..=1.)?;
        }
        let end = if closed_loop {
            1.
        } else {
            phase(rows.last().unwrap().stations[axis], seam)
        };
        if end <= STATION_EPSILON {
            return Err(invalid());
        }
        rail.trim([0., end])?;
        for row in rows.iter_mut() {
            row.stations[axis] = phase(row.stations[axis], seam) / end;
        }
    }
    if closed_loop {
        let mut last = rows[0].clone();
        last.stations = [1.; 2];
        rows.push(last);
    }
    Ok(refit)
}
