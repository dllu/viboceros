//! Intermediate tensor surfaces with explicit source correspondence.
use crate::{GeometryError, NurbsSurface, Point3, Real, WeightedPoint3};

pub const MAX_SURFACE_TWEEN_COUNT: usize = 4096;
pub const MAX_SURFACE_TWEEN_CONTROLS: usize = 1_000_000;

/// Sample normalized UV correspondence and interpolate each blended point grid.
/// Each axis has `sample_number + 1` stations; source trims are not sampled.
pub fn try_tween_nurbs_surfaces_sampled(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
    sample_number: usize,
) -> Result<Vec<NurbsSurface>, GeometryError> {
    if !(1..=MAX_SURFACE_TWEEN_COUNT).contains(&number) || !(2..=255).contains(&sample_number) {
        return Err(error("surface/sample count is outside the resource limit"));
    }
    let count = sample_number + 1;
    check_count(
        count
            .checked_mul(count)
            .ok_or_else(|| error("sample grid count overflow"))?,
        number,
    )?;
    let mut pairs = Vec::with_capacity(count * count);
    for j in 0..count {
        for i in 0..count {
            let fractions = [
                i as Real / sample_number as Real,
                j as Real / sample_number as Real,
            ];
            let point = |surface: &NurbsSurface| {
                let domains = [surface.domain_u(), surface.domain_v()];
                surface.evaluate(
                    crate::exact_scalar::remap_scalar(
                        fractions[0],
                        [0., 1.],
                        [*domains[0].start(), *domains[0].end()],
                    )?,
                    crate::exact_scalar::remap_scalar(
                        fractions[1],
                        [0., 1.],
                        [*domains[1].start(), *domains[1].end()],
                    )?,
                )
            };
            pairs.push((point(start)?, point(end)?));
        }
    }
    let mut result = Vec::with_capacity(number);
    for index in 1..=number {
        let fraction = index as Real / (number + 1) as Real;
        let points = pairs
            .iter()
            .map(|(a, b)| {
                let mut coordinates = [0.; 3];
                for (axis, coordinate) in coordinates.iter_mut().enumerate() {
                    *coordinate = crate::exact_scalar::interpolate_scalar(
                        [a.to_array()[axis], b.to_array()[axis]],
                        [0., 1.],
                        fraction,
                    )?;
                }
                Point3::try_from(coordinates)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        result.push(NurbsSurface::try_through_point_grid(
            &points, [count; 2], [3; 2], [false; 2],
        )?);
    }
    Ok(result)
}

/// Prepare equal-sized nets by exact degree elevation and common knots.
/// Compatible positive rational nets retain first-source weights and apply
/// the measured square-root weight ratio to control displacement.
/// Output domains use the number of distinct spans in the common basis.
/// Input directions and seams are retained; callers adjust them explicitly.
pub fn try_tween_nurbs_surfaces(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
) -> Result<Vec<NurbsSurface>, GeometryError> {
    tween_matched(start, end, number, false)
}

/// Match source degrees and knot multiplicities before interpolating controls.
/// Output domains retain the end surface's native UV intervals.
pub fn try_tween_nurbs_surfaces_refitted(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
) -> Result<Vec<NurbsSurface>, GeometryError> {
    tween_matched(start, end, number, true)
}
fn tween_matched(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
    refit: bool,
) -> Result<Vec<NurbsSurface>, GeometryError> {
    if !(1..=MAX_SURFACE_TWEEN_COUNT).contains(&number) {
        return Err(error("surface count is outside the resource limit"));
    }
    if start
        .control_points()
        .iter()
        .chain(end.control_points())
        .any(|c| c.weight() <= 0.)
    {
        return Err(error("surface tween requires positive weights"));
    }
    let rational = start
        .control_points()
        .iter()
        .chain(end.control_points())
        .any(|c| c.weight() != 1.);
    if !refit
        && rational
        && (start.degree_u() != end.degree_u()
            || start.degree_v() != end.degree_v()
            || start.control_point_count_u() != end.control_point_count_u()
            || start.control_point_count_v() != end.control_point_count_v())
    {
        return Err(error(
            "incompatible rational surface preparation is not yet verified",
        ));
    }
    let degree = [
        start.degree_u().max(end.degree_u()),
        start.degree_v().max(end.degree_v()),
    ];
    if !refit
        && (start.control_point_count_u() != end.control_point_count_u()
            || start.control_point_count_v() != end.control_point_count_v())
    {
        return Err(error(
            "unequal control nets require native common-chart fitting, still under investigation",
        ));
    }
    let mut prepared = [start, end]
        .map(|s| {
            check_count(s.control_points().len(), number)?;
            let predicted = [
                (s.degree_u(), s.knots_u(), s.domain_u(), degree[0]),
                (s.degree_v(), s.knots_v(), s.domain_v(), degree[1]),
            ]
            .map(|(old, knots, domain, new)| {
                new + 1
                    + knots
                        .chunk_by(|a, b| a == b)
                        .filter(|g| g[0] > *domain.start() && g[0] < *domain.end())
                        .map(|g| g.len() + new - old)
                        .sum::<usize>()
            });
            check_count(
                predicted[0]
                    .checked_mul(predicted[1])
                    .ok_or_else(|| error("degree preparation control count overflow"))?,
                number,
            )?;
            s.try_clamped_to_active_domain()?
                .try_change_degree(degree[0], degree[1], false)?
                .try_reparameterized(0. ..=1., 0. ..=1.)
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    for (axis, &axis_degree) in degree.iter().enumerate() {
        let mut union = prepared
            .iter()
            .flat_map(|s| {
                let knots = if axis == 0 { s.knots_u() } else { s.knots_v() };
                knots
                    .chunk_by(|a, b| a == b)
                    .filter(|g| g[0] > 0. && g[0] < 1.)
                    .map(|g| (g[0], g.len()))
            })
            .collect::<Vec<_>>();
        union.sort_by(|a, b| a.0.total_cmp(&b.0));
        let union = union
            .chunk_by(|a, b| a.0 == b.0)
            .map(|g| (g[0].0, g.iter().map(|x| x.1).max().unwrap()))
            .collect::<Vec<_>>();
        let count = axis_degree + 1 + union.iter().map(|x| x.1).sum::<usize>();
        let other = if axis == 0 {
            prepared[0]
                .control_point_count_v()
                .max(prepared[1].control_point_count_v())
        } else {
            prepared[0].control_point_count_u()
        };
        check_count(
            count
                .checked_mul(other)
                .ok_or_else(|| error("control count overflow"))?,
            number,
        )?;
        for s in &mut prepared {
            for &(knot, multiplicity) in &union {
                *s = if axis == 0 {
                    s.try_insert_knot_u(knot, multiplicity)?
                } else {
                    s.try_insert_knot_v(knot, multiplicity)?
                };
            }
        }
    }
    let a = &prepared[0];
    let b = &prepared[1];
    let spans_u = a
        .knots_u()
        .chunk_by(|a, b| a == b)
        .filter(|g| g[0] >= 0. && g[0] <= 1.)
        .count()
        - 1;
    let spans_v = a
        .knots_v()
        .chunk_by(|a, b| a == b)
        .filter(|g| g[0] >= 0. && g[0] <= 1.)
        .count()
        - 1;
    let knots_u = a
        .knots_u()
        .iter()
        .map(|x| x * spans_u as Real)
        .collect::<Vec<_>>();
    let knots_v = a
        .knots_v()
        .iter()
        .map(|x| x * spans_v as Real)
        .collect::<Vec<_>>();
    let mut result = Vec::with_capacity(number);
    for index in 1..=number {
        let fraction = index as Real / (number + 1) as Real;
        let controls = a
            .control_points()
            .iter()
            .zip(b.control_points())
            .map(|(a, b)| {
                let mut coordinates = [0.; 3];
                for (axis, coordinate) in coordinates.iter_mut().enumerate() {
                    *coordinate = crate::exact_scalar::remap_scalar(
                        fraction * (b.weight() / a.weight()).sqrt(),
                        [0., 1.],
                        [a.point().to_array()[axis], b.point().to_array()[axis]],
                    )?;
                }
                let point = Point3::try_from(coordinates)?;
                WeightedPoint3::try_new(point, a.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let surface = NurbsSurface::try_new_rational(
            degree[0],
            degree[1],
            a.control_point_count_u(),
            a.control_point_count_v(),
            controls,
            knots_u.clone(),
            knots_v.clone(),
        )?;
        result.push(if refit {
            surface.try_reparameterized(end.domain_u(), end.domain_v())?
        } else {
            surface
        });
    }
    Ok(result)
}

fn check_count(count: usize, number: usize) -> Result<(), GeometryError> {
    if count
        .checked_mul(number)
        .is_none_or(|n| n > MAX_SURFACE_TWEEN_CONTROLS)
    {
        return Err(error("aggregate control count exceeds the resource limit"));
    }
    Ok(())
}
fn error(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceTween { context }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refitted_polynomial_nets_retain_exact_parameter_correspondence_and_sources() {
        let a = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 0.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap()
        .try_insert_knot_u(0.25, 1)
        .unwrap();
        let b = NurbsSurface::try_new(
            2,
            2,
            3,
            3,
            (0..3)
                .flat_map(|v| {
                    (0..3).map(move |u| {
                        Point3::try_new(u as Real * 2., v as Real * 3., 4. + u as Real * v as Real)
                            .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
        .try_change_degree(3, 2, false)
        .unwrap()
        .try_insert_knot_v(0.75, 2)
        .unwrap()
        .try_reparameterized(2. ..=8., -4. ..=5.)
        .unwrap();
        let before = (a.clone(), b.clone());
        let result = try_tween_nurbs_surfaces_refitted(&a, &b, 2).unwrap();
        for (index, s) in result.iter().enumerate() {
            assert_eq!(s.domain_u(), b.domain_u());
            assert_eq!(s.domain_v(), b.domain_v());
            for j in 0..17 {
                for i in 0..17 {
                    let x = i as Real / 16.;
                    let y = j as Real / 16.;
                    let fraction = (index + 1) as Real / 3.;
                    let actual = s.evaluate(2. + 6. * x, -4. + 9. * y).unwrap();
                    let expected =
                        Point3::try_new(4. * x, 6. * y, fraction * (4. + 4. * x * y)).unwrap();
                    assert!(actual.distance_to(expected).unwrap() < 1e-12);
                }
            }
        }
        assert_eq!((a, b), before);
    }
    #[test]
    fn refitted_tweens_replay_original_native_requested_nets() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_command.json"
        ))
        .unwrap();
        replay_refitted(&q, 6);
    }
    #[test]
    fn refitted_tweens_replay_twenty_four_native_accepted_nets() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_refit.json"
        ))
        .unwrap();
        replay_refitted(&q, 24);
    }
    fn replay_refitted(q: &serde_json::Value, expected_cases: usize) {
        let mut cases = 0;
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["spec"]["method"] != "Refit" {
                continue;
            }
            let a = surface(&v["before"][0]["definition"]);
            let b = surface(&v["before"][1]["definition"]);
            let results = try_tween_nurbs_surfaces_refitted(
                &a,
                &b,
                v["spec"]["number"].as_u64().unwrap() as usize,
            )
            .unwrap();
            for (s, e) in results.iter().zip(
                v["command"]["after_script"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rev()
                    .take(results.len())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev(),
            ) {
                let n = surface(&e["definition"]);
                assert_eq!(
                    (
                        s.degree_u(),
                        s.degree_v(),
                        s.control_point_count_u(),
                        s.control_point_count_v()
                    ),
                    (
                        n.degree_u(),
                        n.degree_v(),
                        n.control_point_count_u(),
                        n.control_point_count_v()
                    )
                );
                for (a, b) in s
                    .knots_u()
                    .iter()
                    .chain(s.knots_v())
                    .zip(n.knots_u().iter().chain(n.knots_v()))
                {
                    assert!((a - b).abs() < 1e-12, "{} knots", v["case"]);
                }
                for (a, b) in s.control_points().iter().zip(n.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-7,
                        "{} {:?} {:?}",
                        v["case"],
                        a,
                        b
                    );
                    assert!((a.weight() - b.weight()).abs() < 1e-12);
                }
            }
            cases += 1;
        }
        assert_eq!(cases, expected_cases);
    }
    #[test]
    fn sampled_tween_grids_keep_analytic_planes_and_reject_excessive_work() {
        let plane = |z| {
            NurbsSurface::try_bilinear([
                Point3::try_new(0., 0., z).unwrap(),
                Point3::try_new(4., 0., z).unwrap(),
                Point3::try_new(4., 6., z).unwrap(),
                Point3::try_new(0., 6., z).unwrap(),
            ])
            .unwrap()
        };
        let a = plane(0.);
        let b = plane(9.);
        let before = (a.clone(), b.clone());
        for sample in [2, 3, 6] {
            let result = try_tween_nurbs_surfaces_sampled(&a, &b, 2, sample).unwrap();
            for (i, s) in result.iter().enumerate() {
                for y in 0..17 {
                    for x in 0..17 {
                        let u = *s.domain_u().start()
                            + (*s.domain_u().end() - *s.domain_u().start()) * x as Real / 16.;
                        let v = *s.domain_v().start()
                            + (*s.domain_v().end() - *s.domain_v().start()) * y as Real / 16.;
                        let p = s.evaluate(u, v).unwrap();
                        assert!(
                            p.distance_to(
                                Point3::try_new(
                                    x as Real / 4.,
                                    y as Real * 6. / 16.,
                                    3. * (i + 1) as Real
                                )
                                .unwrap()
                            )
                            .unwrap()
                                < 1e-12
                        );
                    }
                }
            }
        }
        assert_eq!((a.clone(), b.clone()), before);
        for (number, sample) in [(0, 4), (1, 1), (1, 256), (4096, 255)] {
            assert!(try_tween_nurbs_surfaces_sampled(&a, &b, number, sample).is_err());
        }
    }
    #[test]
    fn sampled_tweens_replay_all_native_sdk_surface_nets() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_command.json"
        ))
        .unwrap();
        replay_sampled(&q);
    }
    #[test]
    fn sampled_tweens_replay_twenty_native_commands_at_small_and_larger_grids() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_sampling.json"
        ))
        .unwrap();
        assert_eq!(q["results"].as_array().unwrap().len(), 20);
        replay_sampled(&q);
    }
    fn replay_sampled(q: &serde_json::Value) {
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let a = surface(&v["before"][0]["definition"]);
            let b = surface(&v["before"][1]["definition"]);
            let results = try_tween_nurbs_surfaces_sampled(
                &a,
                &b,
                v["spec"]["number"].as_u64().unwrap() as usize,
                v["spec"]["sample"].as_u64().unwrap() as usize,
            )
            .unwrap();
            let expected = v["sampling_sdk"].as_array().unwrap();
            assert_eq!(results.len(), expected.len());
            for (s, e) in results.iter().zip(expected) {
                let n = surface(e);
                assert_eq!(
                    (
                        s.degree_u(),
                        s.degree_v(),
                        s.control_point_count_u(),
                        s.control_point_count_v()
                    ),
                    (
                        n.degree_u(),
                        n.degree_v(),
                        n.control_point_count_u(),
                        n.control_point_count_v()
                    )
                );
                for (a, b) in s
                    .knots_u()
                    .iter()
                    .chain(s.knots_v())
                    .zip(n.knots_u().iter().chain(n.knots_v()))
                {
                    assert!((a - b).abs() < 1e-8, "{} knots {a} {b}", v["case"]);
                }
                for (a, b) in s.control_points().iter().zip(n.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-7,
                        "{} {:?} {:?}",
                        v["case"],
                        a,
                        b
                    );
                }
            }
        }
    }
    #[test]
    fn tween_limits_source_purity_and_extreme_coordinates() {
        let huge = f64::MAX * 0.5;
        let a = NurbsSurface::try_bilinear([
            Point3::try_new(-huge, 0., 0.).unwrap(),
            Point3::try_new(-huge, 1., 0.).unwrap(),
            Point3::try_new(-huge, 1., 1.).unwrap(),
            Point3::try_new(-huge, 0., 1.).unwrap(),
        ])
        .unwrap();
        let b = NurbsSurface::try_bilinear([
            Point3::try_new(huge, 0., 0.).unwrap(),
            Point3::try_new(huge, 1., 0.).unwrap(),
            Point3::try_new(huge, 1., 1.).unwrap(),
            Point3::try_new(huge, 0., 1.).unwrap(),
        ])
        .unwrap();
        let before = (a.clone(), b.clone());
        let result = try_tween_nurbs_surfaces(&a, &b, 1).unwrap();
        assert!(
            result[0]
                .control_points()
                .iter()
                .all(|c| c.point().x().is_finite())
        );
        assert_eq!((a.clone(), b.clone()), before);
        for count in [0, MAX_SURFACE_TWEEN_COUNT + 1] {
            assert!(try_tween_nurbs_surfaces(&a, &b, count).is_err());
        }
        assert!(check_count(MAX_SURFACE_TWEEN_CONTROLS, 2).is_err());
    }
    fn surface(v: &serde_json::Value) -> NurbsSurface {
        let counts = v["control_count"].as_array().unwrap();
        let degree = v["degree"].as_array().unwrap();
        let controls = v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect();
        NurbsSurface::try_new_rational(
            degree[0].as_u64().unwrap() as usize,
            degree[1].as_u64().unwrap() as usize,
            counts[0].as_u64().unwrap() as usize,
            counts[1].as_u64().unwrap() as usize,
            controls,
            serde_json::from_value(v["knots_u"].clone()).unwrap(),
            serde_json::from_value(v["knots_v"].clone()).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn control_tweens_replay_native_polynomial_and_rational_nets() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/tween_surfaces_command.json"
        ))
        .unwrap();
        let mut cases = 0;
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["spec"]["method"] != "None"
                || v["case"] == "counts_none"
                || v["case"] == "degrees_none"
            {
                continue;
            }
            let a = surface(&v["before"][0]["definition"]);
            let b = surface(&v["before"][1]["definition"]);
            let results =
                try_tween_nurbs_surfaces(&a, &b, v["spec"]["number"].as_u64().unwrap() as usize)
                    .unwrap();
            let expected = &v["command"]["after_script"].as_array().unwrap()[2..];
            assert_eq!(results.len(), expected.len());
            for (s, e) in results.iter().zip(expected) {
                let n = surface(&e["definition"]);
                assert_eq!(
                    (
                        s.degree_u(),
                        s.degree_v(),
                        s.control_point_count_u(),
                        s.control_point_count_v()
                    ),
                    (
                        n.degree_u(),
                        n.degree_v(),
                        n.control_point_count_u(),
                        n.control_point_count_v()
                    )
                );
                for (a, b) in s
                    .knots_u()
                    .iter()
                    .chain(s.knots_v())
                    .zip(n.knots_u().iter().chain(n.knots_v()))
                {
                    assert!((a - b).abs() < 1e-12);
                }
                for (a, b) in s.control_points().iter().zip(n.control_points()) {
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
            cases += 1;
        }
        assert_eq!(cases, 8);
    }
}
