use super::*;
use viboceros_opennurbs::SubDPatchStencil;

pub(super) fn surface(net: &SubDPatchStencil) -> Result<NurbsSurface, GeometryError> {
    let s = net.side;
    let n = s + 3;
    let stride = s + 1;
    let mut controls = vec![[0.; 3]; n * n];
    for v in 0..=s {
        for u in 0..=s {
            controls[(v + 1) * n + u + 1] = net.controls[v * stride + u];
        }
    }
    for j in 0..=s {
        controls[j + 1] = net.halo[0][j];
        controls[(j + 1) * n + n - 1] = net.halo[1][j];
        controls[(n - 1) * n + n - 2 - j] = net.halo[2][j];
        controls[(n - 2 - j) * n] = net.halo[3][j];
    }
    let corners = [
        (1, 1, 1isize, 1isize),
        (n - 2, 1, -1, 1),
        (n - 2, n - 2, -1, -1),
        (1, n - 2, 1, -1),
    ];
    for (corner, &(u, v, du, dv)) in corners.iter().enumerate() {
        let index = |i: isize, j: isize| {
            (v.checked_add_signed(j * dv).unwrap()) * n + u.checked_add_signed(i * du).unwrap()
        };
        let q = controls[index(0, 0)];
        // A double-crease corner of an original quad is formed one level
        // earlier. N-gon sectors start after the first subdivision. Refining
        // its uniform cubic tensor to this level changes the corner control
        // by (2Q-X-Y)/8 and its two outside side controls by -4 times that.
        // This is the tensor refinement mask, not a fitted positional offset.
        if net.crease_corners[corner] && net.corner.is_none() {
            let a = controls[index(1, 0)];
            let b = controls[index(0, 1)];
            let delta = std::array::from_fn::<_, 3, _>(|k| ((q[k] - a[k]) + (q[k] - b[k])) / 8.);
            for (i, j, factor) in [(0, 0, 1.), (-1, 0, -4.), (0, -1, -4.)] {
                for (k, d) in delta.iter().enumerate() {
                    controls[index(i, j)][k] += factor * d;
                }
            }
        }
        let q = controls[index(0, 0)];
        let limit = net.limit_corners[corner];
        // Uniform cubic value at a knot: [1,4,1] / 6 on each axis. Solve the
        // single missing outside diagonal from the exact limit corner.
        let outside = std::array::from_fn(|k| {
            let sides = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .map(|&(i, j)| controls[index(i, j)][k] - q[k])
                .sum::<f64>();
            let diagonals = [(1, 1), (-1, 1), (1, -1)]
                .iter()
                .map(|&(i, j)| controls[index(i, j)][k] - q[k])
                .sum::<f64>();
            q[k] + 36. * (limit[k] - q[k]) - 4. * sides - diagonals
        });
        controls[index(-1, -1)] = outside;
    }
    let points = controls
        .into_iter()
        .map(Point3::try_from)
        .collect::<Result<_, _>>()?;
    let knots = (-3..=s as i32 + 3)
        .map(|i| i as f64 / s as f64)
        .collect::<Vec<_>>();
    NurbsSurface::try_new(3, 3, n, n, points, knots.clone(), knots)?.try_clamped_to_active_domain()
}
