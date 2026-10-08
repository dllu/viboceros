//! Control matching shares the independently exposed surface rebuild kernel.
use super::*;
use crate::surface_rebuild::rebuild_open as rebuilt;

pub(super) fn tweens(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
) -> Result<Vec<NurbsSurface>, GeometryError> {
    let count = [
        start
            .control_point_count_u()
            .max(end.control_point_count_u()),
        start
            .control_point_count_v()
            .max(end.control_point_count_v()),
    ];
    let degree = [
        start.degree_u().max(end.degree_u()),
        start.degree_v().max(end.degree_v()),
    ];
    check_count(
        count[0]
            .checked_mul(count[1])
            .ok_or_else(|| error("control matching count overflow"))?,
        number,
    )?;
    let incompatible = start.degree_u() != end.degree_u()
        || start.degree_v() != end.degree_v()
        || start.control_point_count_u() != end.control_point_count_u()
        || start.control_point_count_v() != end.control_point_count_v();
    let mut a = start.clone();
    let mut b = end.clone();
    let mut outputs = Vec::with_capacity(number);
    for index in 1..=number {
        if incompatible {
            a = rebuilt(&a, count, degree)?;
            b = rebuilt(&b, count, degree)?;
        }
        let t = index as Real / (number + 1) as Real;
        let controls = a
            .control_points()
            .iter()
            .zip(b.control_points())
            .map(|(a, b)| {
                let factor = t * (b.weight() / a.weight()).sqrt();
                let mut xyz = [0.; 3];
                for (k, x) in xyz.iter_mut().enumerate() {
                    *x = crate::exact_scalar::remap_scalar(
                        factor,
                        [0., 1.],
                        [a.point().to_array()[k], b.point().to_array()[k]],
                    )?;
                }
                WeightedPoint3::try_new(Point3::try_from(xyz)?, a.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        outputs.push(NurbsSurface::try_new_rational(
            degree[0],
            degree[1],
            count[0],
            count[1],
            controls,
            a.knots_u().to_vec(),
            a.knots_v().to_vec(),
        )?);
    }
    Ok(outputs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unequal_nets_reproduce_analytic_planes_and_reject_excess_work() {
        let plane = |n: usize, z: Real| {
            let controls = (0..n)
                .flat_map(|j| {
                    (0..n).map(move |i| {
                        Point3::try_new(
                            4. * i as Real / (n - 1) as Real,
                            6. * j as Real / (n - 1) as Real,
                            z,
                        )
                        .unwrap()
                    })
                })
                .collect();
            let knots = vec![0., 0., 0., 1., 1., 1.];
            NurbsSurface::try_new(
                n - 1,
                n - 1,
                n,
                n,
                controls,
                if n == 2 {
                    vec![0., 0., 1., 1.]
                } else {
                    knots.clone()
                },
                if n == 2 { vec![0., 0., 1., 1.] } else { knots },
            )
            .unwrap()
        };
        let a = plane(2, 0.);
        let b = plane(3, 4.);
        let outputs = try_tween_nurbs_surfaces(&a, &b, 3).unwrap();
        for (index, s) in outputs.iter().enumerate() {
            for j in 0..=8 {
                for i in 0..=8 {
                    let p = s.evaluate(i as Real / 8., j as Real / 8.).unwrap();
                    let expected =
                        Point3::try_new(i as Real / 2., j as Real * 0.75, (index + 1) as Real)
                            .unwrap();
                    assert!(p.distance_to(expected).unwrap() < 1e-12);
                }
            }
        }
        assert!(try_tween_nurbs_surfaces(&a, &b, MAX_SURFACE_TWEEN_COUNT + 1).is_err());
        assert!(check_count(256 * 256, 16).is_err());
    }
    #[test]
    fn control_matching_replays_twenty_four_native_unequal_and_knot_cases() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/tween_surfaces_control_initial.json"
        ))
        .unwrap();
        assert_eq!(q["results"].as_array().unwrap().len(), 24);
        replay_controls(&q);
    }
    #[test]
    fn control_matching_replays_third_tweens_and_equal_count_degree_changes() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/tween_surfaces_control_followup.json"
        ))
        .unwrap();
        assert_eq!(q["results"].as_array().unwrap().len(), 14);
        replay_controls(&q);
    }
    fn replay_controls(q: &serde_json::Value) {
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let a = super::super::tests::surface(&v["before"][0]["definition"]);
            let b = super::super::tests::surface(&v["before"][1]["definition"]);
            let number = v["spec"]["number"].as_u64().unwrap() as usize;
            let before = (a.clone(), b.clone());
            let output = tweens(&a, &b, number).unwrap();
            for (local, record) in output.iter().zip(
                v["command"]["after_script"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .skip(2),
            ) {
                let native = super::super::tests::surface(&record["definition"]);
                assert_eq!(
                    (
                        local.degree_u(),
                        local.degree_v(),
                        local.control_point_count_u(),
                        local.control_point_count_v()
                    ),
                    (
                        native.degree_u(),
                        native.degree_v(),
                        native.control_point_count_u(),
                        native.control_point_count_v()
                    )
                );
                for (a, b) in local
                    .knots_u()
                    .iter()
                    .chain(local.knots_v())
                    .zip(native.knots_u().iter().chain(native.knots_v()))
                {
                    assert!((a - b).abs() < 1e-12, "{}", v["case"]);
                }
                for (a, b) in local.control_points().iter().zip(native.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{} {:?} {:?}",
                        v["case"],
                        a,
                        b
                    );
                    assert!((a.weight() - b.weight()).abs() < 1e-12);
                }
            }
            assert_eq!((a, b), before);
        }
    }
    #[test]
    fn control_preparation_matches_public_surface_rebuild_nets() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/tween_surfaces_control_initial.json"
        ))
        .unwrap();
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let a = super::super::tests::surface(&v["before"][0]["definition"]);
            let b = super::super::tests::surface(&v["before"][1]["definition"]);
            let count = [
                a.control_point_count_u().max(b.control_point_count_u()),
                a.control_point_count_v().max(b.control_point_count_v()),
            ];
            let degree = [
                a.degree_u().max(b.degree_u()),
                a.degree_v().max(b.degree_v()),
            ];
            for (s, n) in [&a, &b]
                .into_iter()
                .zip(v["rebuild_sdk"].as_array().unwrap())
            {
                let candidate = rebuilt(s, count, degree).unwrap();
                let native = super::super::tests::surface(n);
                for (a, b) in candidate
                    .control_points()
                    .iter()
                    .zip(native.control_points())
                {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{}",
                        v["case"]
                    );
                }
            }
        }
    }
}
