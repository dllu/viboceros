//! Bounded shared-family refinement followed by strict original-image proofs.
use super::*;
use viboceros_geometry::{CurveFitStation, WeightedPoint2};

pub struct GordonNetworkSourceImage {
    pub source_index: usize,
    pub uv: NurbsCurve2,
    pub distance_bound: f64,
}

pub struct FittedGordonNetwork {
    pub surface: NurbsSurface,
    pub fitted_curves: [Vec<NurbsCurve>; 2],
    /// Original input indices in the discovered, oriented family order.
    pub families: [Vec<usize>; 2],
    /// Unit U stations locate V curves and vice versa.
    pub stations: [Vec<f64>; 2],
    pub spans_per_interval: [Vec<usize>; 2],
    pub family_fit_attempts: [usize; 2],
    /// Sampled private-frame diagnostic, not a continuous fit certificate.
    pub maximum_sampled_point_distance: f64,
    /// Complete original-world proofs, in original input order.
    pub source_images: Vec<GordonNetworkSourceImage>,
}

/// Discover and orient an open network, fit shared C2 cubic families, construct
/// the native Gordon tensor, and certify every complete original image on its
/// prescribed isoparameter. No unchecked intermediate candidate is returned.
///
/// Common stations average normalized original contact arc lengths. Curve-fit
/// tolerance is one tenth of the smaller model tolerance. Uniform span counts
/// double from four, within the existing per-curve and total prepared-control
/// caps. Only a measured fit rejection requests refinement; invalid inputs and
/// native failures stop. A final bounded geometric refit may run at the cap.
/// Exact/near-exact contacts are required; general best-fit crossing adjustment,
/// Rhino interior fitting, topology and performance remain unqualified.
pub fn fitted_gordon_network_surface(
    curves: &[NurbsCurve],
    edge: Tolerance,
    interior: Tolerance,
    numerical: Tolerance,
    policy: GordonCurveFitClock,
) -> Result<FittedGordonNetwork, Error> {
    fitting::validate_clock(policy)?;
    let fit_tolerance = edge.absolute().min(interior.absolute()) * 0.1;
    if !fit_tolerance.is_finite() || fit_tolerance <= 0. {
        return Err(failure("unrepresentable curve-fit tolerance"));
    }
    let mut working = prepare(curves, edge, interior)?;
    working.tolerance = Tolerance::try_new(
        fit_tolerance / working.scale,
        numerical.relative(),
        numerical.angular(),
    )?;
    let (working, layout) = discover(working, true)?;
    let families = &layout.families;
    let minimum = [families[1].len() - 1, families[0].len() - 1].map(|intervals| 4 * intervals + 3);
    if families[0].len() * minimum[0] + families[1].len() * minimum[1] > 2048 {
        return Err(failure(
            "shared cubic families exceed the prepared control budget",
        ));
    }
    let mut oriented = Vec::with_capacity(curves.len());
    let mut parameters = layout.parameters.clone();
    for (i, curve) in curves.iter().enumerate() {
        let unit = curve.try_reparameterized(0. ..=1.)?;
        oriented.push(if layout.reversed[i] {
            for t in &mut parameters[i] {
                *t = 1. - *t;
            }
            unit.reversed()?.try_reparameterized(0. ..=1.)?
        } else {
            unit
        });
        parameters[i][0] = 0.;
        *parameters[i].last_mut().unwrap() = 1.;
    }
    let length_tolerance = Tolerance::try_new(
        numerical.absolute() / working.scale,
        numerical.relative(),
        numerical.angular(),
    )?;
    let stations = [0, 1]
        .map(|axis| {
            common_stations(
                &working.local,
                &parameters,
                &families[axis],
                length_tolerance,
            )
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let stations: [Vec<f64>; 2] = stations.try_into().map_err(|_| Error::InvalidOutput)?;
    let mut targets = vec![vec![]; families[0].len()];
    for (j, &a) in families[0].iter().enumerate() {
        for (i, &b) in families[1].iter().enumerate() {
            let pa = oriented[a].evaluate(parameters[a][i])?;
            let pb = oriented[b].evaluate(parameters[b][j])?;
            let a_endpoint = i == 0 || i + 1 == families[1].len();
            let b_endpoint = j == 0 || j + 1 == families[0].len();
            targets[j].push(match (a_endpoint, b_endpoint) {
                (true, false) => pa,
                (false, true) => pb,
                _ => pa.midpoint(pb)?,
            });
        }
    }
    let mut fitted: Vec<Vec<NurbsCurve>> = Vec::with_capacity(2);
    let mut spans = [vec![], vec![]];
    let mut attempts = [0; 2];
    let mut maximum = 0_f64;
    for axis in 0..2 {
        // Reserve the opposite family's minimum before refining the first;
        // the second can use everything left by the actually accepted basis.
        let reserved = if axis == 0 {
            families[1].len() * minimum[1]
        } else {
            families[0].len() * fitted[0][0].control_points().len()
        };
        let cap = 128.min((2048 - reserved) / families[axis].len());
        let budget = ((cap - 3) / (stations[axis].len() - 1)).min(124);
        let contacts = families[axis]
            .iter()
            .enumerate()
            .map(|(j, &source)| {
                parameters[source]
                    .iter()
                    .zip(&stations[axis])
                    .enumerate()
                    .map(|(i, (&s, &t))| CurveFitStation {
                        source_parameter: s,
                        fit_parameter: t,
                        point: if axis == 0 {
                            targets[j][i]
                        } else {
                            targets[i][j]
                        },
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let requests = families[axis]
            .iter()
            .zip(&contacts)
            .map(|(&i, stations)| GordonCurveFitInput {
                curve: &oriented[i],
                stations,
            })
            .collect::<Vec<_>>();
        let (fit, grid, count) = refine_family(
            &requests,
            stations[axis].len() - 1,
            budget,
            fit_tolerance,
            numerical,
            policy,
        )?;
        maximum = maximum.max(fit.maximum_sampled_point_distance);
        fitted.push(fit.curves);
        spans[axis] = grid;
        attempts[axis] = count;
    }
    let surface = prepared_gordon_surface(
        &fitted[0],
        &fitted[1],
        &stations[0],
        &stations[1],
        edge,
        interior,
    )?;
    let source_images = certify_originals(&surface, curves, &layout, &stations, edge, interior)?;
    Ok(FittedGordonNetwork {
        surface,
        fitted_curves: fitted.try_into().map_err(|_| Error::InvalidOutput)?,
        families: layout.families,
        stations,
        spans_per_interval: spans,
        family_fit_attempts: attempts,
        maximum_sampled_point_distance: maximum,
        source_images,
    })
}

fn common_stations(
    curves: &[NurbsCurve],
    parameters: &[Vec<f64>],
    family: &[usize],
    tolerance: Tolerance,
) -> Result<Vec<f64>, Error> {
    let count = parameters[family[0]].len();
    let mut values = vec![vec![]; count];
    for &index in family {
        let lengths = parameters[index]
            .windows(2)
            .map(|p| {
                trim_curve(&curves[index], p[0]..=p[1])?
                    .length(tolerance)
                    .map_err(Error::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let total = lengths.iter().sum::<f64>();
        if !total.is_finite() || total <= 0. || lengths.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return Err(failure("invalid original contact arc lengths"));
        }
        let mut accumulated = 0.;
        for j in 1..count - 1 {
            accumulated += lengths[j - 1];
            values[j].push(accumulated / total);
        }
    }
    let mut stations = vec![0.];
    for values in &values[1..count - 1] {
        stations.push(if values.iter().all(|v| *v == values[0]) {
            values[0]
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        });
    }
    stations.push(1.);
    if stations.iter().any(|v| !v.is_finite()) || stations.windows(2).any(|p| p[0] >= p[1]) {
        return Err(failure("common arc-length stations collapsed"));
    }
    Ok(stations)
}

fn refine_family(
    inputs: &[GordonCurveFitInput<'_>],
    intervals: usize,
    maximum: usize,
    tolerance: f64,
    numerical: Tolerance,
    policy: GordonCurveFitClock,
) -> Result<(GordonCurveFamilyFit, Vec<usize>, usize), Error> {
    let mut n = 4;
    let mut attempts = 0;
    loop {
        let grid = vec![n; intervals];
        attempts += 1;
        match fitting::fit_attempt(inputs, &grid, tolerance, numerical, policy, false) {
            Ok(fit) => return Ok((fit, grid, attempts)),
            Err(fitting::FitAttemptError::Fatal(error)) => return Err(error),
            Err(fitting::FitAttemptError::NeedsRefinement { .. }) => {
                if n < maximum {
                    n = (n * 2).min(maximum);
                    continue;
                }
                attempts += 1;
                let fit = fitting::fit_attempt(inputs, &grid, tolerance, numerical, policy, true)
                    .map_err(fitting::FitAttemptError::into_error)?;
                return Ok((fit, grid, attempts));
            }
        }
    }
}

fn certify_originals(
    surface: &NurbsSurface,
    curves: &[NurbsCurve],
    layout: &Layout,
    stations: &[Vec<f64>; 2],
    edge: Tolerance,
    interior: Tolerance,
) -> Result<Vec<GordonNetworkSourceImage>, Error> {
    let mut images = vec![];
    let mut certificate = None;
    for axis in 0..2 {
        let along_u = axis == 0;
        let fixed_domain = if along_u {
            surface.domain_v()
        } else {
            surface.domain_u()
        };
        let moving_domain = if along_u {
            surface.domain_u()
        } else {
            surface.domain_v()
        };
        for (j, &index) in layout.families[axis].iter().enumerate() {
            let limit = if j == 0 || j + 1 == layout.families[axis].len() {
                edge
            } else {
                interior
            };
            let fixed = stations[1 - axis][j].mul_add(
                fixed_domain.end() - fixed_domain.start(),
                *fixed_domain.start(),
            );
            let mut endpoints = [*moving_domain.start(), *moving_domain.end()]
                .map(|t| {
                    if along_u {
                        Point2::try_new(t, fixed)
                    } else {
                        Point2::try_new(fixed, t)
                    }
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            if layout.reversed[index] {
                endpoints.reverse();
            }
            let (mut uv, mut bound) = surface.try_pullback_curve_certified_with_bound(
                &curves[index],
                Some([endpoints[0], endpoints[1]]),
                limit,
            )?;
            let fixed_coordinate = |p: Point2| if along_u { p.y() } else { p.x() };
            if uv
                .control_points()
                .iter()
                .any(|p| fixed_coordinate(p.point()) != fixed)
            {
                let points = uv
                    .control_points()
                    .iter()
                    .map(|p| {
                        let q = p.point();
                        WeightedPoint2::try_new(
                            if along_u {
                                Point2::try_new(q.x(), fixed)?
                            } else {
                                Point2::try_new(fixed, q.y())?
                            },
                            p.weight(),
                        )
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                uv = NurbsCurve2::try_new_rational(uv.degree(), points, uv.knots().to_vec())?;
                if certificate.is_none() {
                    certificate = Some(
                        surface
                            .prepare_surface_curve_bounds()?
                            .ok_or_else(|| failure("unsupported original-image certificate"))?,
                    );
                }
                bound = certificate
                    .as_mut()
                    .unwrap()
                    .bound(&uv, &curves[index], limit.absolute())?
                    .ok_or_else(|| {
                        failure("complete original did not certify at its prescribed isoparameter")
                    })?;
            }
            if uv.domain() != curves[index].domain() {
                return Err(Error::InvalidOutput);
            }
            images.push(GordonNetworkSourceImage {
                source_index: index,
                uv,
                distance_bound: bound,
            });
        }
    }
    images.sort_by_key(|image| image.source_index);
    Ok(images)
}
