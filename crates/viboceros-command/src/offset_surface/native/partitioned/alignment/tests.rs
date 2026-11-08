use super::*;
use viboceros_geometry::{Point3, WeightedPoint3};

fn panel(start: f64, end: f64) -> NurbsSurface {
    let controls = (0..3)
        .flat_map(|j| {
            (0..3).map(move |i| {
                WeightedPoint3::try_new(
                    Point3::try_new(i as f64, j as f64, 0.1 * (i * j) as f64).unwrap(),
                    [1., 1.4, 0.8, 1.2, 1.1, 0.7, 1.6, 1., 1.3][j * 3 + i],
                )
                .unwrap()
            })
        })
        .collect();
    NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        controls,
        vec![start, start, start, end, end, end],
        vec![-7., -7., -7., -1., -1., -1.],
    )
    .unwrap()
}
fn inputs() -> Vec<NurbsSurface> {
    vec![
        panel(2., 3.),
        panel(3., 5.).try_insert_knot_v(-4., 2).unwrap(),
    ]
}

#[test]
fn common_basis_preserves_weighted_supports_and_nonunit_charts() {
    let original = inputs();
    let mut aligned = original.clone();
    supports(
        &mut aligned,
        &original.iter().collect::<Vec<_>>(),
        MAX_STATIONS,
    )
    .unwrap();
    assert_eq!(aligned[0].knots_v(), aligned[1].knots_v());
    for (before, after) in original.iter().zip(&aligned) {
        assert_eq!(before.domain_u(), after.domain_u());
        assert_eq!(before.domain_v(), after.domain_v());
        for i in 0..=16 {
            for j in 0..=16 {
                let u = before.parameter_at_u(i as f64 / 16.).unwrap();
                let v = before.parameter_at_v(j as f64 / 16.).unwrap();
                assert!(
                    before
                        .evaluate(u, v)
                        .unwrap()
                        .distance_bound_to(after.evaluate(u, v).unwrap(), 1e-12)
                        .unwrap()
                        .is_some()
                );
            }
        }
    }
}

#[test]
fn insufficient_station_budget_leaves_supports_unchanged() {
    let original = inputs();
    let mut aligned = original.clone();
    let error = supports(&mut aligned, &original.iter().collect::<Vec<_>>(), 0).unwrap_err();
    assert!(error.to_string().contains("station budget"));
    assert_eq!(original, aligned);
}
