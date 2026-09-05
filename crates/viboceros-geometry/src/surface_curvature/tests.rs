use super::*;
use crate::{AffineTransform3, Frame3, Tolerance};

fn world() -> Frame3 {
    Frame3::try_from_points(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn exact_rational_spheres_cylinders_and_tori_have_analytic_curvatures() {
    for surface in [
        NurbsSurface::try_sphere(world(), 2.0).unwrap(),
        NurbsSurface::try_cylinder(world(), 2.0, -1.0, 4.0).unwrap(),
    ] {
        let sphere = surface.degree_v() > 1;
        for u in [0.0, 0.13, 0.25, 0.53, 0.75, 1.0] {
            for v in [0.05, 0.3, 0.5, 0.8, 0.95] {
                let c = surface
                    .curvature_at(
                        surface.parameter_at_u(u).unwrap(),
                        surface.parameter_at_v(v).unwrap(),
                    )
                    .unwrap();
                close(c.principal[0], -0.5);
                close(c.principal[1], if sphere { -0.5 } else { 0.0 });
            }
        }
    }
    let torus = NurbsSurface::try_torus(world(), 5.0, 2.0).unwrap();
    for u in [0.0, 0.2, 0.5, 1.0] {
        for v in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
            let c = torus
                .curvature_at(
                    torus.parameter_at_u(u).unwrap(),
                    torus.parameter_at_v(v).unwrap(),
                )
                .unwrap();
            let rho = c.point.x().hypot(c.point.y());
            close(c.principal[0], -0.5);
            close(c.principal[1], -(rho - 5.0) / (2.0 * rho));
        }
    }
}

#[test]
fn native_surface_curvature_respects_domain_maps_reversal_and_large_translation() {
    let surface = NurbsSurface::try_torus(world(), 5.0, 2.0).unwrap();
    let u = surface.parameter_at_u(0.23).unwrap();
    let v = surface.parameter_at_v(0.37).unwrap();
    let base = surface.curvature_at(u, v).unwrap();
    let mapped = surface
        .try_reparameterized(-8.0..=13.0, 100.0..=1000.0)
        .unwrap();
    let c = mapped
        .curvature_at(
            mapped.parameter_at_u(0.23).unwrap(),
            mapped.parameter_at_v(0.37).unwrap(),
        )
        .unwrap();
    for (a, b) in tensor(base)
        .into_iter()
        .flatten()
        .zip(tensor(c).into_iter().flatten())
    {
        close(a, b);
    }
    let reversed = surface
        .try_reversed_u()
        .unwrap()
        .curvature_at(-u, v)
        .unwrap();
    for (a, b) in tensor(base)
        .into_iter()
        .flatten()
        .zip(tensor(reversed).into_iter().flatten())
    {
        close(a, -b);
    }
    let translated = surface
        .transformed(AffineTransform3::from_translation(vector([
            1e12, -2e12, 3e12,
        ])))
        .unwrap();
    let moved = translated.curvature_at(u, v).unwrap();
    assert_eq!(moved.normal, base.normal);
    assert_eq!(moved.principal, base.principal);
    assert_eq!(moved.directions, base.directions);
}

fn vector(p: [Real; 3]) -> Vector3 {
    Vector3::try_from(p).unwrap()
}
fn graph(shape: [Real; 3], matrix: [[Real; 2]; 2], scale: Real) -> SurfaceJet2 {
    let [[a, b], [c, d]] = matrix;
    let [l, m, n] = shape;
    SurfaceJet2 {
        point: Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        derivative_u: vector([a * scale, c * scale, 0.0]),
        derivative_v: vector([b * scale, d * scale, 0.0]),
        derivative_uu: vector([0.0, 0.0, (l * a * a + 2.0 * m * a * c + n * c * c) * scale]),
        derivative_uv: vector([
            0.0,
            0.0,
            (l * a * b + m * (a * d + b * c) + n * c * d) * scale,
        ]),
        derivative_vv: vector([0.0, 0.0, (l * b * b + 2.0 * m * b * d + n * d * d) * scale]),
    }
}
fn close(a: Real, b: Real) {
    assert!(
        (a - b).abs() <= 2e-12 * (1.0 + a.abs().max(b.abs())),
        "{a} != {b}"
    );
}
fn tensor(c: SurfaceCurvature) -> [[Real; 3]; 3] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            (0..2)
                .map(|k| {
                    c.principal[k]
                        * c.directions[k].as_vector().to_array()[i]
                        * c.directions[k].as_vector().to_array()[j]
                })
                .sum()
        })
    })
}

#[test]
fn arbitrary_graphs_obey_shape_operator_and_eigenframe_invariants() {
    for shape in [
        [2.0, 0.0, 0.5],
        [-2.0, 0.0, 0.5],
        [1.0, 2.0, -3.0],
        [0.0, 2.0, 0.0],
        [2.0, 0.0, 2.0],
        [0.0; 3],
    ] {
        for matrix in [
            [[1.0, 0.0], [0.0, 1.0]],
            [[2.0, 1.0], [-0.5, 3.0]],
            [[1.0, 3.0], [2.0, 1.0]],
        ] {
            let jet = graph(shape, matrix, 1.0);
            let value = jet.curvature().unwrap();
            let orientation = (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]).signum();
            close(value.mean(), orientation * (shape[0] + shape[2]) * 0.5);
            close(
                value.gaussian().unwrap(),
                shape[0] * shape[2] - shape[1] * shape[1],
            );
            let actual = tensor(value);
            close(actual[0][0], orientation * shape[0]);
            close(actual[0][1], orientation * shape[1]);
            close(actual[1][1], orientation * shape[2]);
            assert!(value.principal[0].abs() >= value.principal[1].abs() - 1e-14);
            let [a, b] = value.directions.map(UnitVector3::as_vector);
            close(a.dot(b).unwrap(), 0.0);
            close(
                a.cross(b).unwrap().dot(value.normal.as_vector()).unwrap(),
                1.0,
            );
            assert_eq!(value.reversed().reversed(), value);
            close(value.reversed().mean(), -value.mean());
            close(
                value.reversed().gaussian().unwrap(),
                value.gaussian().unwrap(),
            );
        }
    }
}

#[test]
fn curvature_is_scale_covariant_and_independent_of_parameter_units() {
    let identity = [[1.0, 0.0], [0.0, 1.0]];
    for scale in [1e-140, 1.0, 1e140] {
        let c = graph([2.0, 0.25, -1.0], identity, scale)
            .curvature()
            .unwrap();
        let a = tensor(c);
        close(a[0][0] * scale, 2.0);
        close(a[0][1] * scale, 0.25);
        close(a[1][1] * scale, -1.0);
    }
    for (u, v) in [
        (1e-150, 1e150),
        (1e150, 1e-150),
        (1e-140, 1e-140),
        (1e140, 1e140),
    ] {
        let c = graph([2.0, 0.25, -1.0], [[u, 0.0], [0.0, v]], 1.0)
            .curvature()
            .unwrap();
        let a = tensor(c);
        close(a[0][0], 2.0);
        close(a[0][1], 0.25);
        close(a[1][1], -1.0);
    }
}

#[test]
fn metric_scaling_does_not_round_a_subnormal_tangent_product() {
    let c = graph([1e120, 0.0, 1e120], [[1e-160, 0.0], [0.0, 1e-160]], 1.0)
        .curvature()
        .unwrap();
    for k in c.principal {
        close(k * 1e-120, 1.0);
    }
}

#[test]
fn small_eigenvalue_survives_large_curvature_ratio() {
    let c = graph([1.0, 0.0, 1e-25], [[1.0, 0.0], [0.0, 1.0]], 1.0)
        .curvature()
        .unwrap();
    assert_eq!(c.principal, [1.0, 1e-25]);
    close(c.gaussian().unwrap() / 1e-25, 1.0);
}

#[test]
fn gaussian_overflow_does_not_hide_representable_principal_curvatures() {
    let c = graph([1.0, 0.0, 1.0], [[1.0, 0.0], [0.0, 1.0]], 1e-200)
        .curvature()
        .unwrap();
    close(c.principal[0] * 1e-200, 1.0);
    assert!(c.gaussian().is_err());
    assert!(c.mean().is_finite());
}

#[test]
fn stationary_and_parallel_tangents_are_errors_not_flat_surface_results() {
    for matrix in [
        [[0.0, 0.0], [0.0, 1.0]],
        [[1.0, 1.0], [1.0, 1.0]],
        [[1.0, 1.0], [0.0, 1e-16]],
    ] {
        assert!(graph([0.0; 3], matrix, 1.0).curvature().is_err());
    }
}
