use super::*;
use crate::bounds::trimmed_surfaces::tests::{circle, face, paraboloid, polygon};

fn region(curves: Vec<Vec<crate::NurbsCurve2>>) -> Region {
    Region::new(
        &face(paraboloid(), curves),
        [[-1., 1.]; 2],
        &mut Budget::default(),
    )
    .unwrap()
}
fn locate(r: &mut Region, x: f64, y: f64) -> Location {
    r.locate(
        [[0.5 + x * 0.5; 2], [0.5 + y * 0.5; 2]],
        &mut Budget::default(),
    )
    .unwrap()
}

#[test]
fn rational_hulls_classify_disk_points_and_rectangles_without_tessellation() {
    for gauge in [1., -1., 1e-200, 1e200] {
        let mut r = region(vec![vec![circle(0.8, gauge)]]);
        for x in -9..=9 {
            for y in -9..=9 {
                let radius = f64::from(x * x + y * y) / 100.;
                let actual = locate(&mut r, f64::from(x) / 10., f64::from(y) / 10.);
                if (radius - 0.64).abs() < 1e-12 {
                    assert_eq!(actual, Location::Uncertain);
                } else {
                    assert_eq!(
                        actual,
                        if radius < 0.64 {
                            Location::Inside
                        } else {
                            Location::Outside
                        },
                        "{x} {y}"
                    );
                }
            }
        }
        assert_eq!(
            r.locate([[0.3, 0.7]; 2], &mut Budget::default()).unwrap(),
            Location::Inside
        );
        assert_eq!(
            r.locate([[0.8, 0.9]; 2], &mut Budget::default()).unwrap(),
            Location::Outside
        );
        assert_eq!(
            r.locate([[0., 1.]; 2], &mut Budget::default()).unwrap(),
            Location::Uncertain
        );
    }
}

#[test]
fn thin_holes_and_concave_loops_are_not_lost_to_model_tolerance() {
    let mut r = region(vec![vec![circle(0.8, 1.)], vec![circle(1e-7, 1.)]]);
    assert_eq!(locate(&mut r, 0., 0.), Location::Outside);
    assert_eq!(locate(&mut r, 2e-7, 0.), Location::Inside);
    let mut r = region(vec![polygon(&[
        [-0.8, -0.8],
        [0.8, -0.8],
        [0.8, -0.3],
        [-0.3, -0.3],
        [-0.3, 0.8],
        [-0.8, 0.8],
    ])]);
    assert_eq!(locate(&mut r, 0., 0.), Location::Outside);
    assert_eq!(locate(&mut r, -0.7, 0.2), Location::Inside);
    assert_eq!(locate(&mut r, 0.2, -0.7), Location::Inside);
}

#[test]
fn boundary_ambiguity_stays_uncertain_instead_of_selecting_a_side() {
    let mut r = region(vec![vec![circle(0.8, 1.)]]);
    for offset in [-1e-14, 0., 1e-14] {
        assert_eq!(locate(&mut r, 0.8 + offset, 0.), Location::Uncertain);
    }
}
