use super::*;
use viboceros_geometry::{
    CurveContinuity, NurbsCurve, NurbsSurface, SurfaceIso, SurfaceKnotDirection,
};

#[derive(Clone, Copy, Debug)]
struct Edge {
    axis: usize,
    high: bool,
}

fn edges(s: &NurbsSurface) -> Result<Vec<(Edge, NurbsCurve)>, CommandError> {
    let mut result = Vec::with_capacity(4);
    for axis in [0, 1] {
        for high in [false, true] {
            let d = if axis == 0 {
                s.domain_u()
            } else {
                s.domain_v()
            };
            let t = if high { *d.end() } else { *d.start() };
            result.push((
                Edge { axis, high },
                if axis == 0 {
                    s.isocurve_v(t)?
                } else {
                    s.isocurve_u(t)?
                },
            ));
        }
    }
    Ok(result)
}

fn support(g: &Geometry, model: Tolerance) -> Result<(&NurbsSurface, bool), CommandError> {
    match g {
        Geometry::NurbsSurface(s) => Ok((s, false)),
        Geometry::Brep(b) if b.faces().len() == 1 && b.faces()[0].is_untrimmed(model)? => {
            Ok((b.faces()[0].surface(), b.faces()[0].is_reversed()))
        }
        _ => Err(CommandError::Usage(
            "MergeSrf requires untrimmed single surfaces",
        )),
    }
}

fn reverse(s: &NurbsSurface, axis: usize) -> Result<NurbsSurface, CommandError> {
    Ok(if axis == 0 {
        s.try_reversed_u()?
    } else {
        s.try_reversed_v()?
    })
}

/// Discovers one complete shared boundary. Native output coefficients are never
/// inputs to orientation preparation, basis conversion or topology construction.
pub fn merge(
    first: &Geometry,
    second: &Geometry,
    options: Options,
    model: Tolerance,
) -> Result<Brep, CommandError> {
    let limit = options.effective_tolerance(model)?;
    let (first, reversed) = support(first, model)?;
    let (second, _) = support(second, model)?;
    let a = edges(first)?;
    let b = edges(second)?;
    let mut matches = Vec::new();
    for (ea, ca) in &a {
        for (eb, cb) in &b {
            for reverse in [false, true] {
                let cb = if reverse { cb.reversed()? } else { cb.clone() };
                let da = ca.domain();
                let db = cb.domain();
                let mut endpoints_match = true;
                for (u, v) in [(*da.start(), *db.start()), (*da.end(), *db.end())] {
                    if ca
                        .evaluate(u)?
                        .distance_bound_to(cb.evaluate(v)?, limit)?
                        .is_none()
                    {
                        endpoints_match = false;
                        break;
                    }
                }
                if !endpoints_match {
                    continue;
                }
                if ca.parameter_curve_deviation_bound(&cb, limit)?.is_some() {
                    matches.push((*ea, *eb, reverse));
                }
            }
        }
    }
    let [(ea, eb, reverse_along)] = matches.as_slice() else {
        return Err(CommandError::Usage(
            "MergeSrf needs one certified shared full boundary",
        ));
    };
    let mut incoming = if ea.axis == eb.axis {
        second.clone()
    } else {
        second.try_swapped_uv()?
    };
    if ea.high == eb.high {
        incoming = reverse(&incoming, ea.axis)?;
    }
    if *reverse_along {
        incoming = reverse(&incoming, 1 - ea.axis)?;
    }
    let direction = if ea.axis == 0 {
        SurfaceKnotDirection::U
    } else {
        SurfaceKnotDirection::V
    };
    let domain = if ea.axis == 0 {
        first.domain_u()
    } else {
        first.domain_v()
    };
    let seam = if ea.high {
        *domain.end()
    } else {
        *domain.start()
    };
    let (left, right) = if ea.high {
        (first.clone(), incoming)
    } else {
        let d = if ea.axis == 0 {
            incoming.domain_u()
        } else {
            incoming.domain_v()
        };
        let incoming_domain = seam - (d.end() - d.start())..=seam;
        let incoming = if ea.axis == 0 {
            incoming.try_reparameterized(incoming_domain, first.domain_v())?
        } else {
            incoming.try_reparameterized(first.domain_u(), incoming_domain)?
        };
        (incoming, first.clone())
    };
    #[cfg(feature = "native-smlib")]
    let surface = viboceros_smlib::merge_surface_supports(
        &left,
        &right,
        direction,
        viboceros_smlib::SurfaceMergeOptions {
            tolerance: limit,
            smooth: options.smooth,
            roundness: options.roundness,
        },
    )?;
    #[cfg(not(feature = "native-smlib"))]
    let surface = super::portable::join(&left, &right, direction, options)?;
    let mut brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        reversed,
        model,
    )?;
    let mut splits = Vec::new();
    for trim in brep.faces()[0].loops()[0].trims() {
        let parameter = match (ea.axis, trim.iso()) {
            (0, SurfaceIso::South) | (1, SurfaceIso::East) => seam,
            (0, SurfaceIso::North) | (1, SurfaceIso::West) => -seam,
            _ => continue,
        };
        if let Some(edge) = trim.edge() {
            let curve = brep.edges()[edge].curve();
            if curve
                .discontinuity_parameters(CurveContinuity::G1, model.angular())?
                .contains(&parameter)
            {
                splits.push((edge, vec![parameter]));
            }
        }
    }
    if !splits.is_empty() {
        brep = brep.try_split_edges_at_parameters(&splits, model)?;
    }
    Ok(brep)
}
