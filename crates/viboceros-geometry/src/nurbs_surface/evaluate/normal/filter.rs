use super::*;
use crate::nurbs::exact::Direction;
mod bounds;
use bounds::Bound;

type NetPoint = [Bound; 4];

fn evaluate(mut net: Vec<NetPoint>, d: Direction<'_>) -> NetPoint {
    for level in 1..=d.degree {
        for local in (level..=d.degree).rev() {
            let i = d.span - d.degree + local;
            let left = Bound::point(d.knots[i]);
            let width = Bound::point(d.knots[i + d.degree - level + 1]) - left;
            let alpha = (Bound::point(d.parameter) - left) / width;
            let complement = Bound::point(1.) - alpha;
            net[local] =
                std::array::from_fn(|j| net[local - 1][j] * complement + net[local][j] * alpha);
        }
    }
    net[d.degree]
}

fn tensor(net: &[NetPoint], u: Direction<'_>, v: Direction<'_>) -> NetPoint {
    evaluate(
        net.chunks_exact(u.degree + 1)
            .map(|row| evaluate(row.to_vec(), u))
            .collect(),
        v,
    )
}

fn derivative(net: &[NetPoint], width: usize, d: Direction<'_>, along_u: bool) -> Vec<NetPoint> {
    let height = net.len() / width;
    let (out_width, out_height) = if along_u {
        (width - 1, height)
    } else {
        (width, height - 1)
    };
    // Multiply each partial by its positive active-span width. This preserves
    // normal direction while avoiding unrepresentable native derivative speeds.
    let span_width = Bound::point(d.knots[d.span + 1]) - Bound::point(d.knots[d.span]);
    let mut output = Vec::with_capacity(out_width * out_height);
    for j in 0..out_height {
        for i in 0..out_width {
            let k = d.span - d.degree + if along_u { i } else { j };
            let width_k = Bound::point(d.knots[k + d.degree + 1]) - Bound::point(d.knots[k + 1]);
            let factor = (span_width / width_k) * Bound::point(d.degree as Real);
            let lower = net[j * width + i];
            let upper = net[j * width + i + if along_u { 1 } else { width }];
            output.push(std::array::from_fn(|c| (upper[c] - lower[c]) * factor));
        }
    }
    output
}

pub(super) fn normal(
    s: &NurbsSurface,
    parameters: [Real; 2],
    spans: [usize; 2],
) -> Option<UnitVector3> {
    let u = Direction {
        knots: &s.knots_u,
        degree: s.degree_u,
        span: spans[0],
        parameter: parameters[0],
    };
    let v = Direction {
        knots: &s.knots_v,
        degree: s.degree_v,
        span: spans[1],
        parameter: parameters[1],
    };
    let origin = s.control_points[s.control_index(u.span - u.degree, v.span - v.degree)]
        .point()
        .to_array();
    let mut controls = Vec::with_capacity((u.degree + 1) * (v.degree + 1));
    let mut point_scale: Real = 0.;
    let mut weight_scale: Real = 0.;
    for j in v.span - v.degree..=v.span {
        for i in u.span - u.degree..=u.span {
            let cp = s.control_points[s.control_index(i, j)];
            let p = cp.point().to_array();
            let xyz: [Bound; 3] =
                std::array::from_fn(|k| Bound::point(p[k]) - Bound::point(origin[k]));
            for c in xyz {
                point_scale = point_scale.max(c.magnitude());
            }
            weight_scale = weight_scale.max(cp.weight().abs());
            controls.push([xyz[0], xyz[1], xyz[2], Bound::point(cp.weight())]);
        }
    }
    if point_scale == 0. || !point_scale.is_finite() {
        return None;
    }
    for cp in &mut controls {
        cp[3] = cp[3] / Bound::point(weight_scale);
        for i in 0..3 {
            cp[i] = (cp[i] / Bound::point(point_scale)) * cp[3];
        }
    }
    let h = tensor(&controls, u, v);
    if !h[3].excludes_zero() {
        return None;
    }
    let hu = tensor(
        &derivative(&controls, u.degree + 1, u, true),
        u.differentiated(),
        v,
    );
    let hv = tensor(
        &derivative(&controls, u.degree + 1, v, false),
        u,
        v.differentiated(),
    );
    let a: [Bound; 3] = std::array::from_fn(|i| hu[i] * h[3] - h[i] * hu[3]);
    let b: [Bound; 3] = std::array::from_fn(|i| hv[i] * h[3] - h[i] * hv[3]);
    let mut n: [Bound; 3] = std::array::from_fn(|i| {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        a[j] * b[k] - a[k] * b[j]
    });
    let scale = n.iter().map(|n| n.magnitude()).fold(0., Real::max);
    if scale == 0. || !scale.is_finite() {
        return None;
    }
    for c in &mut n {
        *c = *c / Bound::point(scale);
    }
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).positive_sqrt()?;
    for c in &mut n {
        *c = *c / length;
    }
    let candidate = Vector3::try_from(n.map(|c| c.midpoint()))
        .ok()?
        .normalized_nonzero()
        .ok()?;
    n.iter()
        .zip(candidate.as_vector().to_array())
        .all(|(bound, value)| bound.error_bound(value) <= 1e-12)
        .then_some(candidate)
}
