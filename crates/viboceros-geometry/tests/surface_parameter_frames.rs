use viboceros_geometry::{
    Frame3, NurbsSurface, Point3, PointMorph, SurfacePointMorph, Tolerance, Vector3,
};

const SCALES: [i32; 4] = [-20, 0, 40, 80];
const DOMAINS: [(i32, i32); 7] = [
    (0, 0),
    (-200, -200),
    (200, 200),
    (-600, -600),
    (600, 600),
    (-600, 600),
    (600, -600),
];

fn setup(exponent: i32) -> (f64, Tolerance, Frame3) {
    let scale = 2_f64.powi(exponent);
    let tolerance = Tolerance::try_new(1e-7 * scale, 1e-12, 1e-9).unwrap();
    let pose = Frame3::try_from_unit_normal(
        Point3::try_new(5. * scale, -7. * scale, 11. * scale).unwrap(),
        Vector3::try_new(2., 3., 6.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
    )
    .unwrap();
    (scale, tolerance, pose)
}

fn local_point(u: f64, v: f64, warp: f64) -> [f64; 3] {
    [2. * u + v, 3. * v, warp * u * v]
}
fn surface(pose: Frame3, scale: f64, warp: f64) -> NurbsSurface {
    NurbsSurface::try_bilinear([(0., 0.), (1., 0.), (1., 1.), (0., 1.)].map(|(u, v)| {
        pose.point_at(local_point(u, v, warp).map(|x| x * scale))
            .unwrap()
    }))
    .unwrap()
}
fn unit(v: [f64; 3]) -> Vector3 {
    Vector3::try_from(v)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
        .as_vector()
}
fn axes(u: f64, v: f64, warp: f64, orientation: usize) -> [Vector3; 3] {
    let mut du = [2., 0., warp * v];
    let mut dv = [1., 3., warp * u];
    match orientation {
        1 => du = du.map(|x| -x),
        2 => dv = dv.map(|x| -x),
        3 => std::mem::swap(&mut du, &mut dv),
        _ => {}
    }
    let x = unit(du);
    let z = x
        .cross(unit(dv))
        .unwrap()
        .normalized_nonzero()
        .unwrap()
        .as_vector();
    let y = z
        .cross(x)
        .unwrap()
        .normalized_nonzero()
        .unwrap()
        .as_vector();
    [x, y, z]
}

fn report(label: &str, checked: usize, failures: &[String]) {
    let failed_cases = failures
        .iter()
        .map(|failure| {
            failure
                .split(':')
                .next()
                .unwrap()
                .split(" at ")
                .next()
                .unwrap()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    eprintln!(
        "SURFACE_PARAMETERS {label}={checked} failed_cases={failed_cases} errors={}",
        failures.len()
    );
}

#[test]
fn regular_surface_frames_keep_pose_and_handedness_under_uv_rescaling_and_reversal() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for exponent in SCALES {
        let (scale, tolerance, pose) = setup(exponent);
        for warp in [0., 1.] {
            for (ue, ve) in DOMAINS {
                let uw = 2_f64.powi(ue);
                let vw = 2_f64.powi(ve);
                let source = surface(pose, scale, warp)
                    .try_reparameterized(0. ..=uw, 0. ..=vw)
                    .unwrap();
                for (orientation, candidate) in [
                    source.clone(),
                    source.try_reversed_u().unwrap(),
                    source.try_reversed_v().unwrap(),
                    source.try_swapped_uv().unwrap(),
                ]
                .into_iter()
                .enumerate()
                {
                    let before = candidate.clone();
                    for (u, v) in [(0., 0.), (0.25, 0.5), (1., 1.)] {
                        checked += 1;
                        let uv = match orientation {
                            1 => [-u * uw, v * vw],
                            2 => [u * uw, -v * vw],
                            3 => [v * vw, u * uw],
                            _ => [u * uw, v * vw],
                        };
                        let label = format!(
                            "scale=2^{exponent} UV=2^({ue},{ve}) warp={warp} orientation={orientation} point=({u},{v})"
                        );
                        match candidate.frame_at(uv[0], uv[1], tolerance) {
                            Ok(frame) => {
                                let expected = pose
                                    .point_at(local_point(u, v, warp).map(|x| x * scale))
                                    .unwrap();
                                if frame.origin().distance_to(expected).unwrap() / scale > 2e-12 {
                                    failures.push(format!("{label}: wrong origin"));
                                }
                                for (actual, expected) in
                                    frame.axes().into_iter().zip(axes(u, v, warp, orientation))
                                {
                                    let expected = pose.vector_at(expected.to_array()).unwrap();
                                    if actual.as_vector().dot(expected).unwrap() < 1. - 2e-12 {
                                        failures.push(format!("{label}: wrong axis"));
                                    }
                                }
                                let [x, y, z] = frame.axes().map(|a| a.as_vector());
                                assert!(x.cross(y).unwrap().dot(z).unwrap() > 1. - 2e-14);
                                assert!(
                                    x.dot(y).unwrap().abs() < 2e-14
                                        && x.dot(z).unwrap().abs() < 2e-14
                                        && y.dot(z).unwrap().abs() < 2e-14
                                );
                            }
                            Err(error) => failures.push(format!("{label}: {error:?}")),
                        }
                    }
                    assert_eq!(candidate, before);
                }
            }
        }
    }
    report("frames", checked, &failures);
    assert_eq!(checked, 672);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn splop_differentials_and_normals_follow_the_same_analytic_map_in_every_uv_unit() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for exponent in SCALES {
        let (scale, tolerance, pose) = setup(exponent);
        let source = Frame3::try_from_unit_normal(
            Point3::try_new(scale, -2. * scale, 3. * scale).unwrap(),
            unit([0., 0., 1.]).normalized_nonzero().unwrap(),
        )
        .unwrap();
        for warp in [0., 1.] {
            let [x, y, _] = axes(0.25, 0.5, warp, 0);
            let du = Vector3::try_new(2., 0., 0.5 * warp).unwrap();
            let dv = Vector3::try_new(1., 3., 0.25 * warp).unwrap();
            let u_speed = du.length().unwrap();
            let v_along_x = dv.dot(x).unwrap();
            let v_speed = dv.dot(y).unwrap();
            for (ue, ve) in DOMAINS {
                let uw = 2_f64.powi(ue);
                let vw = 2_f64.powi(ve);
                let target = surface(pose, scale, warp)
                    .try_reparameterized(0. ..=uw, 0. ..=vw)
                    .unwrap();
                let before = target.clone();
                for (angle, flip) in [
                    (0., false),
                    (std::f64::consts::FRAC_PI_3, false),
                    (std::f64::consts::FRAC_PI_3, true),
                ] {
                    checked += 1;
                    let label = format!(
                        "scale=2^{exponent} UV=2^({ue},{ve}) warp={warp} angle={angle} flip={flip}"
                    );
                    let result = SurfacePointMorph::try_new(
                        source,
                        &target,
                        0.25 * uw,
                        0.5 * vw,
                        1.25,
                        angle,
                        flip,
                        tolerance,
                    );
                    match result {
                        Ok(morph) => {
                            for coordinates in [[0., 0., 0.], [0.2, 0.3, 0.4], [-0.1, 0.15, -0.2]] {
                                let [a, b, c] = coordinates;
                                let (sin, cos) = angle.sin_cos();
                                let tx = 1.25 * (cos * a - sin * b);
                                let ty = 1.25 * (sin * a + cos * b) * if flip { -1. } else { 1. };
                                let tz = 1.25 * c * if flip { -1. } else { 1. };
                                let u = 0.25 + (tx - v_along_x * ty / v_speed) / u_speed;
                                let v = 0.5 + ty / v_speed;
                                let normal = unit([-3. * warp * v, warp * v - 2. * warp * u, 6.]);
                                let q = local_point(u, v, warp);
                                let n = normal.to_array();
                                let expected = pose
                                    .point_at(std::array::from_fn(|i| scale * (q[i] + tz * n[i])))
                                    .unwrap();
                                let input =
                                    source.point_at(coordinates.map(|x| x * scale)).unwrap();
                                match morph.morph_point(input) {
                                    Ok(actual) => {
                                        if actual.distance_to(expected).unwrap() / scale > 2e-11 {
                                            failures.push(format!(
                                                "{label} at {coordinates:?}: normalized error={}",
                                                actual.distance_to(expected).unwrap() / scale
                                            ));
                                        }
                                    }
                                    Err(error) => failures
                                        .push(format!("{label} at {coordinates:?}: {error:?}")),
                                }
                            }
                        }
                        Err(error) => failures.push(format!("{label}: {error:?}")),
                    }
                }
                assert_eq!(target, before);
            }
        }
    }
    report("morphs", checked, &failures);
    assert_eq!(checked, 168);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn surface_frame_and_morph_admission_depends_on_angle_and_rejects_zero_derivatives() {
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let source =
        Frame3::try_from_unit_normal(origin, unit([0., 0., 1.]).normalized_nonzero().unwrap())
            .unwrap();
    let mut checked = 0;
    for absolute in [1e-24, 1e-7, 0.1, 1., 1e17, 1e100] {
        let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-9).unwrap();
        for (ue, ve) in DOMAINS {
            for (separation, accepted) in [(0., false), (0.5e-9, false), (2e-9, true), (1e-6, true)]
            {
                let uw = 2_f64.powi(ue);
                let vw = 2_f64.powi(ve);
                let surface = NurbsSurface::try_bilinear([
                    origin,
                    Point3::try_new(1., 0., 0.).unwrap(),
                    Point3::try_new(2., separation, 0.).unwrap(),
                    Point3::try_new(1., separation, 0.).unwrap(),
                ])
                .unwrap()
                .try_reparameterized(0. ..=uw, 0. ..=vw)
                .unwrap();
                let frame = surface.frame_at(0.5 * uw, 0.5 * vw, tolerance);
                assert_eq!(
                    frame.is_ok(),
                    accepted,
                    "distance={absolute} UV=2^({ue},{ve}) separation={separation}"
                );
                let morph = SurfacePointMorph::try_new(
                    source,
                    &surface,
                    0.5 * uw,
                    0.5 * vw,
                    1.,
                    0.,
                    false,
                    tolerance,
                );
                assert_eq!(
                    morph.is_ok(),
                    accepted,
                    "distance={absolute} UV=2^({ue},{ve}) separation={separation}"
                );
                if let Ok(morph) = morph {
                    let actual = morph
                        .morph_point(Point3::try_new(0., 0., 0.2).unwrap())
                        .unwrap();
                    let expected = Point3::try_new(1., 0.5 * separation, 0.2).unwrap();
                    assert!(actual.distance_to(expected).unwrap() < 2e-12);
                }
                checked += 1;
            }
        }
        let collapsed = NurbsSurface::try_bilinear([
            origin,
            origin,
            Point3::try_new(0., 1., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
        ])
        .unwrap();
        assert!(collapsed.frame_at(0.5, 0.5, tolerance).is_err());
        assert!(
            SurfacePointMorph::try_new(source, &collapsed, 0.5, 0.5, 1., 0., false, tolerance)
                .is_err()
        );
        checked += 1;
    }
    assert_eq!(checked, 174);
    eprintln!("SURFACE_PARAMETERS angular_admission={checked}");
}

#[test]
fn constrained_morph_normals_remain_directions_and_retain_the_plane_map() {
    let mut checked = 0;
    for exponent in SCALES {
        let (scale, tolerance, pose) = setup(exponent);
        let source = Frame3::try_from_unit_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            unit([0., 0., 1.]).normalized_nonzero().unwrap(),
        )
        .unwrap();
        for (ue, ve) in DOMAINS {
            let uw = 2_f64.powi(ue);
            let vw = 2_f64.powi(ve);
            let target = surface(pose, scale, 0.)
                .try_reparameterized(0. ..=uw, 0. ..=vw)
                .unwrap();
            let original = SurfacePointMorph::try_new(
                source,
                &target,
                0.25 * uw,
                0.5 * vw,
                1.,
                0.,
                false,
                tolerance,
            )
            .unwrap();
            assert!(
                original
                    .clone()
                    .with_constrained_normal(Vector3::try_new(0., 0., 0.).unwrap())
                    .is_err()
            );
            assert!(
                original
                    .clone()
                    .with_surface_orientation_policy(Some(pose.x_axis().as_vector()))
                    .is_err()
            );
            for ne in [-200, 0, 200] {
                let normal = pose.z_axis().as_vector().scaled(2_f64.powi(ne)).unwrap();
                for morph in [
                    original.clone().with_constrained_normal(normal).unwrap(),
                    original
                        .clone()
                        .with_surface_orientation_policy(Some(normal))
                        .unwrap(),
                ] {
                    let input = Point3::try_new(0.1 * scale, 0.2 * scale, 0.3 * scale).unwrap();
                    let actual = morph.morph_point(input).unwrap();
                    let expected = pose
                        .point_at([1.1 * scale, 1.7 * scale, 0.3 * scale])
                        .unwrap();
                    assert!(actual.distance_to(expected).unwrap() / scale < 2e-11);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 168);
    eprintln!("SURFACE_PARAMETERS constrained_normals={checked}");
}
