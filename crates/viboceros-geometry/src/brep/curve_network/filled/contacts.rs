//! Exact line cuts avoid adaptive near-parallel intersection searches.
use super::*;
use crate::CurveCurveIntersection;
use crate::exact_scalar::{rational, scalar};

pub(in crate::brep::curve_network) fn same_line(a: &NurbsCurve, b: &NurbsCurve) -> Option<bool> {
    let linear = |c: &NurbsCurve| {
        c.degree() == 1
            && c.control_points().len() == 2
            && c.control_points()[0].weight().is_sign_positive()
                == c.control_points()[1].weight().is_sign_positive()
    };
    if !linear(a) || !linear(b) {
        return None;
    }
    let ends = |c: &NurbsCurve| [c.control_points()[0].point(), c.control_points()[1].point()];
    let x = ends(a);
    let y = ends(b);
    Some(x == y || x == [y[1], y[0]])
}

fn parameter(curve: &NurbsCurve, fraction: Real) -> Result<Real, GeometryError> {
    let controls = curve.control_points();
    let a = rational(controls[0].weight());
    let b = rational(controls[1].weight());
    let f = rational(fraction);
    let u = &f * &a / ((rational(1.) - &f) * b + &f * a);
    let domain = curve.domain();
    scalar(&(rational(*domain.start()) + (rational(*domain.end()) - rational(*domain.start())) * u))
}

pub(in crate::brep::curve_network) fn line_contacts(
    a: &SurfaceCutArrangementCurve,
    b: &SurfaceCutArrangementCurve,
    tolerance: Tolerance,
    axes: [usize; 2],
) -> Result<Option<Vec<CurveCurveIntersection>>, GeometryError> {
    let linear = |c: &NurbsCurve| {
        c.degree() == 1
            && c.control_points().len() == 2
            && c.control_points()[0].weight().is_sign_positive()
                == c.control_points()[1].weight().is_sign_positive()
    };
    if !linear(&a.spatial) || !linear(&b.spatial) {
        return Ok(None);
    }
    let ends = |c: &NurbsCurve| {
        [0, 1].map(|i| {
            let p = c.control_points()[i].point().to_array();
            [p[axes[0]], p[axes[1]]]
        })
    };
    let cuts = boolean::exact_planar_line_cuts(ends(&a.spatial), ends(&b.spatial))?;
    let stations = match cuts {
        boolean::LineCuts::None => Vec::new(),
        boolean::LineCuts::Point(p) => vec![p],
        boolean::LineCuts::Overlap { first, second } => {
            vec![[first[0], second[0]], [first[1], second[1]]]
        }
    };
    let mut result = Vec::new();
    for [u, v] in stations {
        let first_parameter = parameter(&a.spatial, u)?;
        let second_parameter = parameter(&b.spatial, v)?;
        let first = a.spatial.evaluate(first_parameter)?;
        let second = b.spatial.evaluate(second_parameter)?;
        if first.distance_to(second)? > tolerance.absolute() {
            return invalid("a planar line cut has no shared spatial witness");
        }
        let point = first.translated(first.vector_to(second)?.scaled(0.5)?)?;
        result.push(CurveCurveIntersection {
            first_parameter,
            second_parameter,
            point,
        });
    }
    Ok(Some(result))
}

/// Straight rays have zero geometric bend. Order their stored directions with
/// exact determinants, even when atan2 rounds two distinct directions equally.
pub(in crate::brep::curve_network) fn sort_line_rays(
    halfedges: &mut [usize],
    edges: &[SurfaceCutArrangementEdge],
    axes: [usize; 2],
) -> bool {
    use num_traits::{Signed, Zero};
    if halfedges
        .iter()
        .any(|&h| same_line(&edges[h / 2].spatial, &edges[h / 2].spatial).is_none())
    {
        return false;
    }
    let mut rays = halfedges
        .iter()
        .map(|&h| {
            let controls = edges[h / 2].spatial.control_points();
            let a = controls[usize::from(!h.is_multiple_of(2))]
                .point()
                .to_array();
            let b = controls[usize::from(h.is_multiple_of(2))]
                .point()
                .to_array();
            let ray = axes.map(|i| rational(b[i]) - rational(a[i]));
            let lower = ray[1].is_negative() || (ray[1].is_zero() && ray[0].is_negative());
            (h, lower, ray)
        })
        .collect::<Vec<_>>();
    rays.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| {
            let determinant = &a.2[0] * &b.2[1] - &a.2[1] * &b.2[0];
            if determinant.is_positive() {
                std::cmp::Ordering::Less
            } else if determinant.is_negative() {
                std::cmp::Ordering::Greater
            } else {
                a.0.cmp(&b.0)
            }
        })
    });
    for (h, ray) in halfedges.iter_mut().zip(rays) {
        *h = ray.0;
    }
    true
}
