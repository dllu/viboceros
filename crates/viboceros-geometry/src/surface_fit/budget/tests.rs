use super::*;

fn surface(u: Vec<Real>, v: Vec<Real>) -> NurbsSurface {
    let nu = u.len() - 4;
    let nv = v.len() - 4;
    NurbsSurface::try_new(
        3,
        3,
        nu,
        nv,
        (0..nv)
            .flat_map(|j| (0..nu).map(move |i| Point3::try_new(i as Real, j as Real, 0.).unwrap()))
            .collect(),
        u,
        v,
    )
    .unwrap()
}
fn knots(interior: &[Real]) -> Vec<Real> {
    [vec![0.; 4], interior.to_vec(), vec![1.; 4]].concat()
}
fn budget(u: &[Real], v: &[Real]) -> SurfaceFitRefinementBudget {
    SurfaceFitRefinementBudget::try_from_surface(&surface(knots(u), knots(v)), [3, 3])
        .unwrap()
        .unwrap()
}

#[test]
fn repeated_knots_do_not_buy_more_refinement_capacity() {
    let once = budget(&[0.5], &[0.5]);
    let triple = budget(&[0.5, 0.5, 0.5], &[0.5, 0.5, 0.5]);
    assert_eq!(once, triple);
    assert_eq!(once.initial_control_count(), [4, 4]);
    assert_eq!(once.maximum_control_count(), [5, 5]);
    let distinct = budget(&[0.25, 0.5, 0.75], &[0.25, 0.5, 0.75]);
    assert_eq!(distinct.maximum_control_count(), [7, 7]);
    assert!(distinct.admits_control_count([5, 5]));
    assert!(!triple.admits_control_count([5, 5]));
}

#[test]
fn initial_patch_and_strict_reduction_are_distinct_cases() {
    assert!(budget(&[], &[]).admits_control_count([4, 4]));
    let b = budget(&[0.5], &[0.5]);
    for allowed in [[4, 4], [5, 4], [4, 5]] {
        assert!(b.admits_control_count(allowed));
    }
    for rejected in [[5, 5], [6, 4], [4, 6], [3, 4], [0, 0], [usize::MAX, 4]] {
        assert!(!b.admits_control_count(rejected));
    }
}

#[test]
fn one_axis_cannot_spend_the_other_axes_budget() {
    let b = budget(&[0.5], &[]);
    assert_eq!(b.maximum_control_count(), [5, 4]);
    assert!(!b.admits_control_count([5, 4]));
    let b = budget(&[0.25, 0.5, 0.75], &[0.5, 0.5, 0.5]);
    assert!(b.admits_control_count([5, 5]));
    assert!(!b.admits_control_count([4, 6]));
}

#[test]
fn degree_changes_and_unsupported_ends_are_unknown_not_rejected_candidates() {
    let source = surface(knots(&[0.5]), knots(&[0.5]));
    let before = source.clone();
    assert!(
        SurfaceFitRefinementBudget::try_from_surface(&source, [2, 3])
            .unwrap()
            .is_none()
    );
    assert!(
        SurfaceFitRefinementBudget::try_from_surface(&source, [5, 3])
            .unwrap()
            .is_none()
    );
    assert!(SurfaceFitRefinementBudget::try_from_surface(&source, [0, 3]).is_err());
    assert!(SurfaceFitRefinementBudget::try_from_surface(&source, [3, 12]).is_err());
    assert_eq!(source, before);
    let open = surface(vec![-3., -2., -1., 0., 1., 2., 3., 4.], knots(&[]));
    assert!(
        SurfaceFitRefinementBudget::try_from_surface(&open, [3, 3])
            .unwrap()
            .is_none()
    );
    let broken = surface(knots(&[0.5; 4]), knots(&[]));
    assert!(
        SurfaceFitRefinementBudget::try_from_surface(&broken, [3, 3])
            .unwrap()
            .is_none()
    );
}
