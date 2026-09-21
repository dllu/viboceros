use super::*;

fn options() -> CurveJoinOptions {
    CurveJoinOptions {
        tolerance: 0.01,
        preserve_direction: false,
        style: CurveJoinStyle::Seeded,
    }
}

fn branched_chain(length: usize, branches: usize) -> Vec<Curve3> {
    (0..length)
        .map(|i| line([i as Real, 0.0], [i as Real + 1.0, 0.0]))
        .chain((0..branches).map(|i| line([0.0, 0.0], [-10.0, 10.0 + i as Real])))
        .collect()
}

#[test]
fn seeded_join_extends_long_chains_without_repeatedly_scanning_unused_branches() {
    let curves = branched_chain(40_000, 600);
    let before = curves.clone();
    let joined = join_curves(&curves, options(), Tolerance::DEFAULT).unwrap();
    assert_eq!(joined.len(), 600);
    assert_eq!(joined[0].source_indices(), (0..=40_000).collect::<Vec<_>>());
    let Curve3::Polyline(curve) = joined[0].curve() else {
        panic!("linear chain");
    };
    assert_eq!(curve.vertices().len(), 40_002);
    assert_eq!(curve.vertices()[0], p(-10.0, 10.0));
    assert_eq!(*curve.vertices().last().unwrap(), p(40_000.0, 0.0));
    assert_eq!(curve.parameters()[1], 0.0);
    assert_eq!(*curve.parameters().last().unwrap(), 40_000.0);
    assert_eq!(curves, before);
}

#[test]
fn seeded_closed_chain_ignores_a_dense_unrelated_candidate_graph() {
    let mut curves = vec![
        line([0.0, 0.0], [1.0, 0.0]),
        line([1.0, 0.0], [1.0, 1.0]),
        line([1.0, 1.0], [0.0, 0.0]),
    ];
    curves.extend((0..2000).map(|_| line([100.0, 0.0], [101.0, 0.0])));
    let before = curves.clone();
    let joined = join_curves(&curves, options(), Tolerance::DEFAULT).unwrap();
    assert_eq!(joined.len(), 2001);
    assert_eq!(joined[0].source_indices(), &[0, 1, 2]);
    assert!(joined[0].curve().as_ref().is_closed().unwrap());
    assert!(joined[1..].iter().all(|c| c.source_indices().len() == 1));
    assert_eq!(curves, before);
}

#[test]
#[ignore = "manual seeded-join timing; no wall-clock CI threshold"]
fn seeded_join_benchmark() {
    use std::{hint::black_box, time::Instant};
    for (length, branches) in [(1000, 200), (10_000, 200), (40_000, 600)] {
        let curves = branched_chain(length, branches);
        let mut times = Vec::new();
        for _ in 0..7 {
            let now = Instant::now();
            let result = join_curves(black_box(&curves), options(), Tolerance::DEFAULT);
            times.push(now.elapsed().as_secs_f64());
            match result {
                Ok(joined) => {
                    assert_eq!(joined[0].source_indices().len(), length + 1);
                    assert_eq!(joined.len(), branches);
                }
                Err(error) => {
                    println!("chain_{length}_branches_{branches}: {error}");
                    times.clear();
                    break;
                }
            }
        }
        if !times.is_empty() {
            times.sort_by(Real::total_cmp);
            println!(
                "chain_{length}_branches_{branches}: median {:.6} s; samples {times:?}",
                times[3]
            );
        }
    }
}
