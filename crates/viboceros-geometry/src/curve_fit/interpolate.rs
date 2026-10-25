//! Fixed endpoint/kink handles and portable or delegated collocation solves.
use super::*;
use crate::nurbs::bspline_basis_values;
use faer::{Mat, prelude::*};

pub(super) fn interpolate_fit<E: From<GeometryError>>(
    sampler: &FitSampler<'_>,
    degree: usize,
    knots: &[Real],
    kinks: &[FitKink],
    pins: &[Pin],
    solve: &mut Option<&mut FitSolver<'_, E>>,
) -> Result<NurbsCurve, E> {
    let control_count = knots.len() - degree - 1;
    if degree == 1 {
        let controls = (0..control_count)
            .map(|index| sampler.point_at_parameter(knots[index + 1]))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(NurbsCurve::try_new(degree, controls, knots.to_vec())?);
    }

    let mut controls = vec![None; control_count];
    constrain_endpoint_handles(sampler, degree, knots, &mut controls)?;
    for kink in kinks {
        constrain_kink_handles(sampler, degree, knots, kink, &mut controls)?;
    }

    if let Some(solve) = solve.as_deref_mut() {
        solve_external_fit_controls(sampler, degree, knots, &mut controls, pins, solve)?;
    } else if degree == 3 {
        solve_cubic_fit_controls(sampler, knots, &mut controls, pins)?;
    } else {
        solve_dense_fit_controls(sampler, degree, knots, &mut controls)?;
    }

    Ok(NurbsCurve::try_new(
        degree,
        controls
            .into_iter()
            .map(|point| point.expect("all curve-fit controls are constrained or solved"))
            .collect(),
        knots.to_vec(),
    )?)
}

fn constrained_sites(
    knots: &[Real],
    degree: usize,
    controls: &[Option<Point3>],
    pins: &[Pin],
) -> Result<(Vec<Real>, Vec<Option<Point3>>), GeometryError> {
    let mut parameters = (0..controls.len())
        .map(|i| {
            if i == 0 {
                Ok(knots[degree])
            } else if i + 1 == controls.len() {
                Ok(knots[controls.len()])
            } else {
                greville_parameter(knots, degree, i)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut targets = vec![None; controls.len()];
    for pin in pins {
        if pin.distance == knots[degree] || pin.distance == knots[controls.len()] {
            continue;
        }
        let start = knots.partition_point(|k| *k < pin.distance);
        let end = knots.partition_point(|k| *k <= pin.distance);
        if end - start == degree {
            continue;
        } // The source kink already fixes this point and both handles.
        if degree != 3 || end - start != 1 || start < 4 {
            return Err(GeometryError::InvalidCurveFitConstraints {
                context: "constraint needs a simple cubic knot",
            });
        }
        // At a simple cubic knot r, active controls are r-3..r. Row r-2
        // fits in the existing five-diagonal backend; its neighboring Greville
        // sites lie strictly on opposite sides of the requested station.
        let row = start - 2;
        if row >= controls.len() || controls[row].is_some() || targets[row].is_some() {
            return Err(GeometryError::InvalidCurveFitConstraints {
                context: "constraint conflicts with a fixed collocation row",
            });
        }
        parameters[row] = pin.distance;
        targets[row] = Some(pin.point);
    }
    Ok((parameters, targets))
}

fn solve_external_fit_controls<E: From<GeometryError>>(
    sampler: &FitSampler<'_>,
    degree: usize,
    knots: &[Real],
    controls: &mut [Option<Point3>],
    pins: &[Pin],
    solve: &mut FitSolver<'_, E>,
) -> Result<(), E> {
    if controls.iter().all(Option::is_some) {
        return Ok(());
    }
    let origin = controls[0].unwrap().to_array();
    let (parameters, pin_targets) = constrained_sites(knots, degree, controls, pins)?;
    let fixed_rows = controls
        .iter()
        .map(|p| u8::from(p.is_some()))
        .collect::<Vec<_>>();
    let mut targets = Vec::with_capacity(controls.len() * 4);
    for (i, &t) in parameters.iter().enumerate() {
        let p = if let Some(p) = controls[i] {
            p
        } else {
            pin_targets[i]
                .map(Ok)
                .unwrap_or_else(|| sampler.point_at_parameter(t))?
        };
        let xyz = p.to_array();
        targets.extend([
            xyz[0] - origin[0],
            xyz[1] - origin[1],
            xyz[2] - origin[2],
            1.,
        ]);
    }
    let system = crate::NurbsInterpolationSystem {
        degree,
        knots,
        parameters: &parameters,
        periodic: false,
        columns: 4,
        targets: &targets,
    };
    let output = crate::surface_rebuild::checked_solve(&system, &mut |_| {
        solve(&CurveFitInterpolation {
            system: crate::NurbsInterpolationSystem {
                degree,
                knots,
                parameters: &parameters,
                periodic: false,
                columns: 4,
                targets: &targets,
            },
            fixed_rows: &fixed_rows,
        })
    })?;
    for (i, control) in controls.iter_mut().enumerate() {
        if control.is_none() {
            *control = Some(Point3::try_from(std::array::from_fn(|k| {
                output[i * 4 + k] + origin[k]
            }))?);
        }
    }
    Ok(())
}

pub(super) fn solve_cubic_fit_controls(
    sampler: &FitSampler<'_>,
    knots: &[Real],
    controls: &mut [Option<Point3>],
    pins: &[Pin],
) -> Result<(), GeometryError> {
    let origin = controls[0].unwrap().to_array();
    let (parameters, pin_targets) = constrained_sites(knots, 3, controls, pins)?;
    let mut rows = Vec::with_capacity(controls.len());
    let mut targets: Vec<[Real; 3]> = Vec::with_capacity(controls.len());
    for (i, fixed) in controls.iter().enumerate() {
        let (row, point) = if let Some(point) = fixed {
            ([0.0, 0.0, 1.0, 0.0, 0.0], *point)
        } else if pins.is_empty() {
            let (row, parameter, _, _) = crate::spline_collocation::collocation_row(knots, i)?;
            (row, sampler.point_at_parameter(parameter)?)
        } else {
            let parameter = parameters[i];
            let row = crate::spline_collocation::collocation_row_at(knots, i, parameter)?;
            (
                row,
                pin_targets[i]
                    .map(Ok)
                    .unwrap_or_else(|| sampler.point_at_parameter(parameter))?,
            )
        };
        rows.push(row);
        targets.push(std::array::from_fn(|k| point.to_array()[k] - origin[k]));
    }
    let rhs = Mat::from_fn(controls.len(), 3, |i, k| targets[i][k]);
    let solution = crate::spline_collocation::solve(&rows, rhs)?;
    for (i, control) in controls.iter_mut().enumerate() {
        if control.is_none() {
            *control = Some(Point3::try_from(std::array::from_fn(|k| {
                solution[(i, k)] + origin[k]
            }))?);
        }
    }
    Ok(())
}

pub(super) fn solve_dense_fit_controls(
    sampler: &FitSampler<'_>,
    degree: usize,
    knots: &[Real],
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let control_count = controls.len();

    let unknown = controls
        .iter()
        .enumerate()
        .filter_map(|(index, point)| point.is_none().then_some(index))
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        let mut rows = Vec::with_capacity(unknown.len());
        let mut targets = Vec::with_capacity(unknown.len());
        for &control_index in &unknown {
            let parameter = greville_parameter(knots, degree, control_index)?;
            let basis = bspline_basis_values(knots, degree, control_count, parameter)?;
            let mut target = sampler.point_at_parameter(parameter)?.to_array();
            for (fixed_index, fixed) in controls.iter().enumerate() {
                if let Some(point) = fixed {
                    let coordinates = point.to_array();
                    for coordinate in 0..3 {
                        target[coordinate] = (-basis[fixed_index])
                            .mul_add(coordinates[coordinate], target[coordinate]);
                    }
                }
            }
            rows.push(
                unknown
                    .iter()
                    .map(|&index| basis[index])
                    .collect::<Vec<_>>(),
            );
            targets.push(target);
        }

        let count = unknown.len();
        let matrix = Mat::from_fn(count, count, |row, column| rows[row][column]);
        let right_hand_side = Mat::from_fn(count, 3, |row, column| targets[row][column]);
        let solution = matrix.full_piv_lu().solve(&right_hand_side);
        for (row, &control_index) in unknown.iter().enumerate() {
            controls[control_index] = Some(Point3::try_new(
                solution[(row, 0)],
                solution[(row, 1)],
                solution[(row, 2)],
            )?);
        }
    }

    Ok(())
}

pub(super) fn constrain_endpoint_handles(
    sampler: &FitSampler<'_>,
    degree: usize,
    knots: &[Real],
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let last = controls.len() - 1;
    let start = sampler.sample_at_parameter(0.0)?;
    let end = sampler.sample_at_parameter(sampler.end_parameter())?;
    let start_handle_length =
        (knots[degree + 1] - knots[degree]) / degree as Real * sampler.speed_at_parameter(0.)?;
    let end_handle_length = (knots[controls.len()] - knots[controls.len() - 1]) / degree as Real
        * sampler.speed_at_parameter(sampler.end_parameter())?;
    controls[0] = Some(start.point());
    controls[1] = Some(
        start
            .point()
            .translated(start.tangent().as_vector().scaled(start_handle_length)?)?,
    );
    controls[last - 1] = Some(
        end.point()
            .translated(end.tangent().as_vector().scaled(-end_handle_length)?)?,
    );
    controls[last] = Some(end.point());
    Ok(())
}

pub(super) fn constrain_kink_handles(
    sampler: &FitSampler<'_>,
    degree: usize,
    knots: &[Real],
    kink: &FitKink,
    controls: &mut [Option<Point3>],
) -> Result<(), GeometryError> {
    let run_start = knots.partition_point(|knot| *knot < kink.parameter);
    let junction = run_start - 1;
    let point = sampler.point_at_parameter(kink.parameter)?;
    let speed = sampler.speed_at_parameter(kink.parameter)?;
    let incoming_length = (kink.parameter - knots[junction]) / degree as Real * speed;
    let outgoing_length = (knots[junction + degree + 1] - kink.parameter) / degree as Real * speed;
    controls[junction - 1] =
        Some(point.translated(kink.incoming_tangent.as_vector().scaled(-incoming_length)?)?);
    controls[junction] = Some(point);
    controls[junction + 1] =
        Some(point.translated(kink.outgoing_tangent.as_vector().scaled(outgoing_length)?)?);
    Ok(())
}

fn greville_parameter(
    knots: &[Real],
    degree: usize,
    control_index: usize,
) -> Result<Real, GeometryError> {
    let values = &knots[control_index + 1..=control_index + degree];
    let scale = values.iter().copied().fold(0.0_f64, Real::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    let normalized_sum = values.iter().map(|value| value / scale).sum::<Real>();
    let parameter = scale * (normalized_sum / degree as Real);
    if parameter.is_finite() {
        Ok(parameter)
    } else {
        Err(GeometryError::NonFinite {
            context: "curve-fit Greville parameter",
        })
    }
}
