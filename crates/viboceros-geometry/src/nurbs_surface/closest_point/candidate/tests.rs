use super::*;

fn p(values: [Real; 3]) -> Point3 {
    Point3::try_from(values).unwrap()
}

#[test]
fn partial_candidate_selection_matches_full_exact_stable_grid_sort() {
    for target in [p([0.; 3]), p([0., 0., 1e100]), p([Real::MAX; 3])] {
        for count in [0, 1, 15, 16, 17, 33, 1089] {
            let mut grid = (0..count)
                .map(|i| {
                    // Many exact squared-distance ties, plus a singular-point row.
                    let point = if i < 33 {
                        p([0.; 3])
                    } else {
                        p([
                            (i % 7) as Real - 3.,
                            (i % 5) as Real - 2.,
                            (i % 3) as Real - 1.,
                        ])
                    };
                    Candidate::new(
                        target,
                        point,
                        ((i % 33) as Real / 32., (i / 33) as Real / 32.),
                    )
                })
                .collect::<Vec<_>>();
            let mut expected = grid.clone();
            expected.sort_by(|a, b| target.compare_distances(a.point(), b.point()));
            expected.truncate(16);
            retain_closest_seeds(&mut grid, target);
            assert_eq!(
                grid.iter().map(|c| c.parameters).collect::<Vec<_>>(),
                expected.iter().map(|c| c.parameters).collect::<Vec<_>>()
            );
        }
    }
}
