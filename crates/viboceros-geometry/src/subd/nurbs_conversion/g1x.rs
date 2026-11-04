//! Shared near-vertex normal-gap corrections after the local G1 projection.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct EdgeUse {
    patch: usize,
    corner: usize,
    u: bool,
    source_scale: f64,
}

fn index(surface: &NurbsSurface, corner: usize, i: usize, j: usize) -> usize {
    let (nu, nv) = (
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    );
    let u = if corner == 1 || corner == 2 {
        nu - 1 - i
    } else {
        i
    };
    let v = if corner >= 2 { nv - 1 - j } else { j };
    v * nu + u
}

pub(super) fn improve(
    source: &SubD,
    patches: &mut [SubDNurbsPatch],
    normals: &[Option<[f64; 3]>],
) -> Result<(), GeometryError> {
    let mut uses = BTreeMap::<(u32, u32), Vec<EdgeUse>>::new();
    for (patch_index, patch) in patches.iter().enumerate() {
        let face = &source.faces()[patch.face];
        for corner in 0..if patch.corner.is_some() { 1 } else { 4 } {
            let source_corner = patch.corner.unwrap_or(corner);
            let vertex = face[source_corner];
            if normals[vertex as usize].is_none() {
                continue;
            }
            for (other, u) in [
                (face[(source_corner + 1) % face.len()], corner % 2 == 0),
                (
                    face[(source_corner + face.len() - 1) % face.len()],
                    corner % 2 != 0,
                ),
            ] {
                uses.entry((vertex, other)).or_default().push(EdgeUse {
                    patch: patch_index,
                    corner,
                    u,
                    source_scale: if patch.corner.is_some() { 0.5 } else { 1. },
                });
            }
        }
    }
    for ((vertex, _), pair) in uses {
        let [a, b] = pair.as_slice() else {
            return Err(failure(
                "G1x requires two incident patches at a smooth edge",
            ));
        };
        if a.patch == b.patch {
            return Err(failure("G1x does not support repeated face-edge uses"));
        }
        let first = &patches[a.patch].surface;
        let second = &patches[b.patch].surface;
        if sampled_g1(first, *a, second, *b)? {
            continue;
        }
        let normal = normals[vertex as usize].ok_or_else(|| failure("Missing G1x limit normal"))?;
        let p = first.control_points()[index(first, a.corner, 0, 0)]
            .point()
            .to_array();
        let q = first.control_points()[index(
            first,
            a.corner,
            if a.u { 2 } else { 0 },
            if a.u { 0 } else { 2 },
        )]
        .point()
        .to_array();
        let distance = (0..3).map(|i| (q[i] - p[i]) * normal[i]).sum::<f64>();
        let delta = normal.map(|n| -distance * n);
        for step in 1..=4 {
            let candidate_a = shift(first, *a, delta, step as f64 / 4.)?;
            let candidate_b = shift(second, *b, delta, step as f64 / 4.)?;
            if step == 4 || sampled_g1(&candidate_a, *a, &candidate_b, *b)? {
                patches[a.patch].surface = candidate_a;
                patches[b.patch].surface = candidate_b;
                break;
            }
        }
    }
    Ok(())
}

fn failure(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}

fn shift(
    surface: &NurbsSurface,
    edge: EdgeUse,
    delta: [f64; 3],
    fraction: f64,
) -> Result<NurbsSurface, GeometryError> {
    let mut controls = surface
        .control_points()
        .iter()
        .map(|p| p.point())
        .collect::<Vec<_>>();
    for transverse in 0..2 {
        let (i, j) = if edge.u {
            (2, transverse)
        } else {
            (transverse, 2)
        };
        let at = index(surface, edge.corner, i, j);
        let old = controls[at].to_array();
        controls[at] = Point3::try_from(std::array::from_fn(|a| old[a] + fraction * delta[a]))?;
    }
    NurbsSurface::try_new(
        3,
        3,
        surface.control_point_count_u(),
        surface.control_point_count_v(),
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )
}

fn uv(surface: &NurbsSurface, edge: EdgeUse, fraction: f64) -> [f64; 2] {
    let backwards = [edge.corner == 1 || edge.corner == 2, edge.corner >= 2];
    let domains = [surface.domain_u(), surface.domain_v()];
    let mut uv = std::array::from_fn(|a| {
        if backwards[a] {
            *domains[a].end()
        } else {
            *domains[a].start()
        }
    });
    let axis = usize::from(!edge.u);
    let knots = if edge.u {
        surface.knots_u()
    } else {
        surface.knots_v()
    };
    let next = if backwards[axis] {
        knots[knots.len() - 5]
    } else {
        knots[4]
    };
    uv[axis] += (next - uv[axis]) * fraction;
    uv
}

fn normal(surface: &NurbsSurface, uv: [f64; 2]) -> Result<[f64; 3], GeometryError> {
    let (_, du, dv) = surface.evaluate_with_derivatives(uv[0], uv[1])?;
    let scaled = |v: [f64; 3]| -> Result<[f64; 3], GeometryError> {
        let scale = v.iter().fold(0_f64, |s, c| s.max(c.abs()));
        if scale == 0. || !scale.is_finite() {
            return Err(failure("Undefined G1x boundary tangent"));
        }
        Ok(v.map(|c| c / scale))
    };
    let a = scaled(du.to_array())?;
    let b = scaled(dv.to_array())?;
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = n[0].hypot(n[1]).hypot(n[2]);
    if !length.is_finite() || length == 0. {
        return Err(failure("Undefined G1x boundary normal"));
    }
    Ok(n.map(|c| c / length))
}

fn sampled_g1(
    a: &NurbsSurface,
    ea: EdgeUse,
    b: &NurbsSurface,
    eb: EdgeUse,
) -> Result<bool, GeometryError> {
    const COS_ONE_DEGREE: f64 = 0.9998476951563913;
    let reach = |s: &NurbsSurface, e: EdgeUse| {
        let end = uv(s, e, 0.);
        let next = uv(s, e, 1.);
        ((next[0] - end[0]).abs() + (next[1] - end[1]).abs()) * e.source_scale
    };
    let ra = reach(a, ea);
    let rb = reach(b, eb);
    let common = ra.min(rb);
    // Keep the seven interior eighths established by the G1x controls. A denser
    // test changes quarter-step decisions near one degree; this is a sampling
    // convention, not a bound on the continuous normal gap.
    for station in 1..8 {
        let f = station as f64 / 8.;
        let na = normal(a, uv(a, ea, f * common / ra))?;
        let nb = normal(b, uv(b, eb, f * common / rb))?;
        if (0..3).map(|i| na[i] * nb[i]).sum::<f64>() < COS_ONE_DEGREE {
            return Ok(false);
        }
    }
    Ok(true)
}
