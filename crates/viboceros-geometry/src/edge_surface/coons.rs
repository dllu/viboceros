use super::*;

pub(super) fn surface(
    u: [&NurbsCurve; 2],
    v: [&NurbsCurve; 2],
) -> Result<NurbsSurface, GeometryError> {
    build(u, v, 0)
}

fn build(
    u: [&NurbsCurve; 2],
    v: [&NurbsCurve; 2],
    elevations: usize,
) -> Result<NurbsSurface, GeometryError> {
    let [south, north] = u;
    let [west, east] = v;
    let (nu, nv) = (south.control_points().len(), west.control_points().len());
    let origin = south.control_points()[0].point();
    let homogeneous = |c: &WeightedPoint3| -> Result<[Real; 4], GeometryError> {
        let p = origin.vector_to(c.point())?.to_array();
        Ok([
            p[0] * c.weight(),
            p[1] * c.weight(),
            p[2] * c.weight(),
            c.weight(),
        ])
    };
    let boundaries = [south, north, west, east].map(|c| {
        c.control_points()
            .iter()
            .map(homogeneous)
            .collect::<Result<Vec<_>, _>>()
    });
    let [s, n, w, e] = boundaries;
    let (s, n, w, e) = (s?, n?, w?, e?);
    let gu = basis::greville(south)?;
    let gv = basis::greville(west)?;
    let mut controls = Vec::with_capacity(nu * nv);
    for j in 0..nv {
        for i in 0..nu {
            let bottom = mix(s[0], s[nu - 1], gu[i]);
            let top = mix(n[0], n[nu - 1], gu[i]);
            let corner = mix(bottom, top, gv[j]);
            let a = mix(s[i], n[i], gv[j]);
            let b = mix(w[j], e[j], gu[i]);
            controls.push(std::array::from_fn::<_, 4, _>(|axis| {
                a[axis] + b[axis] - corner[axis]
            }));
        }
    }
    crate::require_finite(
        controls.iter().flatten().copied(),
        "homogeneous Coons controls",
    )?;
    if controls.iter().any(|c| c[3] == 0.0) {
        // A valid finite patch can contain projective controls at infinity.
        // The current Euclidean weighted-control representation cannot store
        // them. Exact degree elevation changes the basis, not the surface.
        // Coons blending is linear in homogeneous coordinates, so elevating
        // both U boundaries before recomputing the grid is exact.
        if elevations == 4 {
            return Err(GeometryError::InvalidControlNet {
                context: "Coons control at infinity remains after exact degree elevation",
            });
        }
        let degree = south.degree() + 1;
        basis::check_count(south, degree)?;
        basis::check_count(north, degree)?;
        let a = south.try_change_degree(degree, false)?;
        let b = north.try_change_degree(degree, false)?;
        return build([&a, &b], v, elevations + 1);
    }
    let mut result = Vec::with_capacity(controls.len());
    for j in 0..nv {
        for i in 0..nu {
            let cp = if j == 0 {
                south.control_points()[i]
            } else if j == nv - 1 {
                north.control_points()[i]
            } else if i == 0 {
                west.control_points()[j]
            } else if i == nu - 1 {
                east.control_points()[j]
            } else {
                let h = controls[j * nu + i];
                let o = origin.to_array();
                WeightedPoint3::try_new(
                    Point3::try_from(std::array::from_fn(|axis| h[axis] / h[3] + o[axis]))?,
                    h[3],
                )?
            };
            result.push(cp);
        }
    }
    NurbsSurface::try_new_rational(
        south.degree(),
        west.degree(),
        nu,
        nv,
        result,
        south.knots().to_vec(),
        west.knots().to_vec(),
    )
}

fn mix(a: [Real; 4], b: [Real; 4], t: Real) -> [Real; 4] {
    std::array::from_fn(|i| {
        if a[i] == b[i] {
            a[i]
        } else {
            (1.0 - t).mul_add(a[i], t * b[i])
        }
    })
}
