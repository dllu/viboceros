//! Conservative polynomial Taylor-sign certificates for unseen continuation tails.
use super::*;
use viboceros_geometry::{BoundingBox3, NurbsCurve};

pub(super) fn excludes(
    curve: &NurbsCurve,
    at_start: bool,
    target: BoundingBox3,
    padding: Real,
) -> bool {
    let Ok(spans) = curve.try_bezier_spans() else {
        return false;
    };
    let Some(span) = (if at_start {
        spans.first()
    } else {
        spans.last()
    }) else {
        return false;
    };
    let controls = span.control_points();
    let weight = controls[0].weight();
    if controls.iter().any(|p| p.weight() != weight) {
        return false;
    }
    let mut points = controls
        .iter()
        .map(|p| p.point().to_array())
        .collect::<Vec<_>>();
    if at_start {
        points.reverse();
    }
    let endpoint = points.last().unwrap();
    let lo = target.min().to_array();
    let hi = target.max().to_array();
    for axis in 0..3 {
        let upper = endpoint[axis] > hi[axis] + padding;
        let lower = endpoint[axis] < lo[axis] - padding;
        if !upper && !lower {
            continue;
        }
        // At a Bezier end, Taylor coefficients have the signs of successive
        // terminal finite differences. Outward-rounded intervals retain zero
        // uncertainty instead of treating a tiny ambiguous sign as monotone.
        let mut net = points
            .iter()
            .map(|p| [p[axis], p[axis]])
            .collect::<Vec<_>>();
        let mut certified = true;
        while net.len() > 1 {
            net = net
                .windows(2)
                .map(|p| {
                    [
                        (p[1][0] - p[0][1]).next_down(),
                        (p[1][1] - p[0][0]).next_up(),
                    ]
                })
                .collect();
            let value = net.last().unwrap();
            certified &= if upper {
                value[0] >= 0.
            } else {
                value[1] <= 0.
            };
        }
        if certified {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::Point3;
    fn source(end_y: Real) -> NurbsCurve {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        NurbsCurve::try_new(
            2,
            vec![p(0., 6.), p(0.5, 7.5), p(1., end_y)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap()
    }
    fn target() -> BoundingBox3 {
        BoundingBox3::from_points([
            Point3::try_new(-4., -4., 0.).unwrap(),
            Point3::try_new(4., 4., 0.).unwrap(),
        ])
        .unwrap()
    }
    #[test]
    fn outward_parabolic_tails_are_certified_beyond_the_turn() {
        let original = source(10.);
        let curve = original.try_extended_to(-4. ..=5.).unwrap();
        assert!(excludes(&curve, true, target(), 1e-6));
        assert!(excludes(&curve, false, target(), 1e-6));
        let early = original.try_extended_to(-1. ..=2.).unwrap();
        assert!(!excludes(&early, true, target(), 1e-6));
    }
    #[test]
    fn initially_outward_tail_that_returns_is_not_certified() {
        let curve = source(8.);
        assert!(!excludes(&curve, false, target(), 1e-6));
        let returning = curve.try_extended_to(0. ..=4.).unwrap();
        assert_eq!(returning.evaluate(4.).unwrap().y(), 2.);
    }
}
