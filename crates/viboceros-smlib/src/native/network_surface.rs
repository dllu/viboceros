//! Gordon networks, native intersections, and complete source-image acceptance.
use super::*;
use viboceros_geometry::{AffineTransform3, CurveCurveIntersectionEvent, NurbsCurve2, Point2};
unsafe extern "C" {
    fn vb_curves_network_surface(
        u: *const *const NativeCurve,
        nu: usize,
        v: *const *const NativeCurve,
        nv: usize,
        u_parameters: *const f64,
        v_parameters: *const f64,
        tolerance: f64,
        u_stations: *mut f64,
        v_stations: *mut f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
fn failure(message: &str) -> Error {
    Error::Kernel(format!("Gordon network: {message}"))
}

mod prepared;
pub use prepared::prepared_gordon_surface;
mod fitting;
pub use fitting::{
    GordonCurveFamilyFit, GordonCurveFitClock, GordonCurveFitInput, fit_gordon_curve_family,
    fit_gordon_curve_family_with_clock,
};

/// Build a position-matched open curve network. Families, order and direction
/// are inferred from intersections; original curves and their domains survive.
/// Every complete input image must certify against the resulting surface at
/// its edge or interior tolerance. No topology/injectivity claim is made.
pub fn gordon_network_surface(
    curves: &[NurbsCurve],
    edge: Tolerance,
    interior: Tolerance,
) -> Result<NurbsSurface, Error> {
    let WorkingNetwork {
        mut local,
        origin,
        scale,
        tolerance,
    } = prepare(curves, edge, interior)?;
    let n = local.len();
    let mut contacts = vec![vec![None; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            let events = curve_intersection_events(&local[i], &local[j], tolerance)?;
            match events.as_slice() {
                [] => {}
                [CurveCurveIntersectionEvent::Point(p)] => {
                    contacts[i][j] = Some(p.first_parameter());
                    contacts[j][i] = Some(p.second_parameter());
                }
                _ => return Err(failure("curves must cross at most once without overlaps")),
            }
        }
    }
    let mut a = (0..n)
        .filter(|&i| contacts[0][i].is_none())
        .collect::<Vec<_>>();
    let mut b = (0..n)
        .filter(|&i| contacts[0][i].is_some())
        .collect::<Vec<_>>();
    if !(2..=32).contains(&a.len()) || !(2..=32).contains(&b.len()) {
        return Err(failure("needs two crossing families of 2–32 curves"));
    }
    for (i, row) in contacts.iter().enumerate() {
        for (j, contact) in row.iter().enumerate().skip(i + 1) {
            if contact.is_some() != (a.contains(&i) != a.contains(&j)) {
                return Err(failure(
                    "each curve must cross every opposite-family curve and no same-family curves",
                ));
            }
        }
    }
    b.sort_by(|&i, &j| contacts[0][i].unwrap().total_cmp(&contacts[0][j].unwrap()));
    a.sort_by(|&i, &j| {
        contacts[b[0]][i]
            .unwrap()
            .total_cmp(&contacts[b[0]][j].unwrap())
    });
    let mut reversed = vec![false; n];
    for (family, other) in [(&a, &b), (&b, &a)] {
        for &i in family {
            let parameters = other
                .iter()
                .map(|&j| contacts[i][j].unwrap())
                .collect::<Vec<_>>();
            let increasing = parameters.windows(2).all(|p| p[1] - p[0] > 1e-10);
            let decreasing = parameters.windows(2).all(|p| p[0] - p[1] > 1e-10);
            if !increasing && !decreasing {
                return Err(failure("inconsistent or coincident intersection order"));
            }
            if decreasing {
                local[i] = local[i].reversed()?.try_reparameterized(0. ..=1.)?;
                reversed[i] = true;
            }
            // Complete curves are the network constraints; no unrequested
            // trimming or extrapolation beyond the four outer curves.
            let ends = [local[i].evaluate(0.)?, local[i].evaluate(1.)?];
            let t0 = if decreasing {
                1. - parameters[0]
            } else {
                parameters[0]
            };
            let t1 = if decreasing {
                1. - parameters[parameters.len() - 1]
            } else {
                parameters[parameters.len() - 1]
            };
            if local[i].evaluate(t0)?.distance_to(ends[0])? > tolerance.absolute()
                || local[i].evaluate(t1)?.distance_to(ends[1])? > tolerance.absolute()
            {
                return Err(failure(
                    "outer-family crossings must meet the complete curve endpoints",
                ));
            }
        }
    }
    let mut parameters = vec![vec![]; n];
    for (family, other) in [(&a, &b), (&b, &a)] {
        for &i in family {
            parameters[i] = other.iter().map(|&j| contacts[i][j].unwrap()).collect();
        }
    }
    construct(
        curves,
        WorkingNetwork {
            local,
            origin,
            scale,
            tolerance,
        },
        Layout {
            families: [a, b],
            parameters,
            reversed,
            strict: false,
        },
        edge,
        interior,
    )
}

struct WorkingNetwork {
    local: Vec<NurbsCurve>,
    origin: Point3,
    scale: f64,
    tolerance: Tolerance,
}
struct Layout {
    families: [Vec<usize>; 2],
    parameters: Vec<Vec<f64>>,
    reversed: Vec<bool>,
    strict: bool,
}
fn prepare(
    curves: &[NurbsCurve],
    edge: Tolerance,
    interior: Tolerance,
) -> Result<WorkingNetwork, Error> {
    if !(4..=64).contains(&curves.len())
        || curves
            .iter()
            .map(|c| c.control_points().len())
            .sum::<usize>()
            > 2048
    {
        return Err(failure(
            "requires 4–64 curves and at most 2048 total controls",
        ));
    }
    let origin = curves[0].evaluate(*curves[0].domain().start())?;
    let scale = curves
        .iter()
        .flat_map(|c| c.control_points())
        .try_fold(0_f64, |s, p| {
            Ok::<_, Error>(s.max(origin.distance_to(p.point())?))
        })?;
    if !scale.is_finite() || scale <= edge.absolute().max(interior.absolute()) {
        return Err(failure("collapsed network"));
    }
    let tolerance = Tolerance::try_new(
        edge.absolute().max(interior.absolute()) / scale,
        edge.relative(),
        edge.angular(),
    )?;
    let mut local = vec![];
    for c in curves {
        if c.degree() > 16 || c.control_points().len() > 128 {
            return Err(failure("degree limit 16; control limit 128 per curve"));
        }
        let p = working_curve::normalized(c, origin, scale)?
            .ok_or_else(|| failure("unsupported input basis or rational weights"))?;
        if p.evaluate(0.)?.distance_to(p.evaluate(1.)?)? <= tolerance.absolute() {
            return Err(failure("requires open noncollapsed curves"));
        }
        local.push(p);
    }
    Ok(WorkingNetwork {
        local,
        origin,
        scale,
        tolerance,
    })
}

fn construct(
    curves: &[NurbsCurve],
    working: WorkingNetwork,
    layout: Layout,
    edge: Tolerance,
    interior: Tolerance,
) -> Result<NurbsSurface, Error> {
    let WorkingNetwork {
        local,
        origin,
        scale,
        ..
    } = working;
    let [a, b] = &layout.families;
    let reversed = &layout.reversed;
    let native = local
        .iter()
        .map(KernelCurve::from_nurbs_for_modeling)
        .collect::<Result<Vec<_>, _>>()?;
    let u = a
        .iter()
        .map(|&i| native[i].handle.as_ptr().cast_const())
        .collect::<Vec<_>>();
    let v = b
        .iter()
        .map(|&i| native[i].handle.as_ptr().cast_const())
        .collect::<Vec<_>>();
    let [original_u, original_v] = [(a, b), (b, a)].map(|(family, other)| {
        let mut parameters = Vec::with_capacity(family.len() * other.len());
        for &i in family {
            for &t in &layout.parameters[i] {
                parameters.push(if reversed[i] { 1. - t } else { t });
            }
        }
        parameters
    });
    let mut u_stations = vec![0.; b.len()];
    let mut v_stations = vec![0.; a.len()];
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curves_network_surface(
            u.as_ptr(),
            u.len(),
            v.as_ptr(),
            v.len(),
            original_u.as_ptr(),
            original_v.as_ptr(),
            edge.absolute().min(interior.absolute()) / scale,
            u_stations.as_mut_ptr(),
            v_stations.as_mut_ptr(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let local_surface = brep::surface_from_native(pointer)?;
    let surface = local_surface.transformed(AffineTransform3::try_new(
        [[scale, 0., 0.], [0., scale, 0.], [0., 0., scale]],
        Vector3::try_from(origin.to_array())?,
    )?)?;
    // Native Gordon alignment averages the original intersection parameters
    // and maps curve knots piecewise affinely. Use that correspondence as a UV
    // proposal, then independently certify it against the original world data.
    // This avoids fitting a general inverse for each rational input isocurve.
    let uv_point = |u: f64, v: f64| {
        Point2::try_new(
            u.mul_add(
                surface.domain_u().end() - surface.domain_u().start(),
                *surface.domain_u().start(),
            ),
            v.mul_add(
                surface.domain_v().end() - surface.domain_v().start(),
                *surface.domain_v().start(),
            ),
        )
    };
    let mut certificate = surface
        .prepare_surface_curve_bounds()?
        .ok_or_else(|| failure("unsupported target certificate basis"))?;
    for (family, other, fixed, moving, along_u) in [
        (a, b, &v_stations, &u_stations, true),
        (b, a, &u_stations, &v_stations, false),
    ] {
        for (index, &i) in family.iter().enumerate() {
            let limit = if index == 0 || index + 1 == family.len() {
                edge
            } else {
                interior
            };
            let source = &curves[i];
            let domain = source.domain();
            let mut nodes = other
                .iter()
                .enumerate()
                .map(|(j, _)| {
                    Ok((
                        layout.parameters[i][j],
                        if along_u {
                            uv_point(moving[j], fixed[index])?
                        } else {
                            uv_point(fixed[index], moving[j])?
                        },
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            if reversed[i] {
                nodes.reverse();
            }
            let mut knots = vec![*domain.start(), *domain.start()];
            knots.extend(
                nodes[1..nodes.len() - 1]
                    .iter()
                    .map(|(t, _)| t.mul_add(domain.end() - domain.start(), *domain.start())),
            );
            knots.extend([*domain.end(), *domain.end()]);
            let uv = NurbsCurve2::try_new(1, nodes.into_iter().map(|(_, p)| p).collect(), knots)?;
            if certificate.bound(&uv, source, limit.absolute())?.is_none() {
                if layout.strict {
                    return Err(failure(
                        "prepared curve did not certify at its prescribed isoparameter",
                    ));
                }
                surface
                    .try_pullback_curve_certified(source, limit)
                    .map_err(|e| {
                        failure(&format!(
                            "complete input curve {i} could not certify within tolerance: {e}"
                        ))
                    })?;
            }
        }
    }
    Ok(surface)
}
