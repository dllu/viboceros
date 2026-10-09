//! Exact source-edge/isocurve domains and contour-axis line parameters.
use super::*;
pub(super) fn cut(
    source: &Geometry,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    if let Geometry::Brep(brep) = source {
        return super::brep::cut(brep, frame, tolerance);
    }
    let mut result = section::section_geometry(source, frame, 1., true, tolerance)?;
    let Geometry::NurbsSurface(surface) = source else {
        return Ok(result);
    };
    let boundaries = [
        surface.isocurve_u(*surface.domain_v().start())?,
        surface.isocurve_v(*surface.domain_u().end())?,
        surface.isocurve_u(*surface.domain_v().end())?.reversed()?,
        surface
            .isocurve_v(*surface.domain_u().start())?
            .reversed()?,
    ];
    let iso_curves = bilinear_isocurves(surface, frame)?;
    let planar = surface.plane(tolerance)?.is_some();
    for geometry in &mut result {
        let Some(curve) = geometry.nurbs_curve_representation()? else {
            continue;
        };
        if !straight_segment(&curve, tolerance) {
            continue;
        }
        let domain = curve.domain();
        let start = curve.evaluate(*domain.start())?;
        let end = curve.evaluate(*domain.end())?;
        if let Some(boundary) = matching(&boundaries, start, end, tolerance)? {
            *geometry = Geometry::NurbsCurve(boundary);
            continue;
        }
        let direction = start.vector_to(end)?.normalized_nonzero()?.as_vector();
        let offset = frame.origin().vector_to(start)?;
        let mut iso = None;
        for (curve, constant) in &iso_curves {
            if let Some(curve) = matching(std::slice::from_ref(curve), start, end, tolerance)? {
                iso = Some((curve, *constant));
                break;
            }
        }
        let plane_fixed = if direction.cross(frame.x_axis().as_vector())?.length()?
            <= tolerance.angular()
        {
            Some(offset.dot(frame.y_axis().as_vector())?.abs())
        } else if direction.cross(frame.y_axis().as_vector())?.length()? <= tolerance.angular() {
            Some(offset.dot(frame.x_axis().as_vector())?.abs())
        } else {
            None
        };
        // Native captures with independently shifted UV ranges distinguish
        // competing isocurves by the magnitude of their fixed parameter.
        let use_plane = match (&iso, plane_fixed) {
            (Some((_, constant)), Some(plane)) => plane <= constant.abs(),
            (None, _) => planar || offset.cross(direction)?.length()? <= tolerance.absolute(),
            _ => false,
        };
        if use_plane {
            let first = offset.dot(direction)?;
            let last = frame.origin().vector_to(end)?.dot(direction)?;
            *geometry = Geometry::NurbsCurve(linear_with_domain(start, end, first..=last)?);
        } else if let Some((curve, _)) = iso {
            *geometry = Geometry::NurbsCurve(curve);
        }
    }
    Ok(result)
}
fn linear_with_domain(
    start: Point3,
    end: Point3,
    domain: std::ops::RangeInclusive<f64>,
) -> Result<NurbsCurve, GeometryError> {
    NurbsCurve::try_new(
        1,
        vec![start, end],
        vec![
            *domain.start(),
            *domain.start(),
            *domain.end(),
            *domain.end(),
        ],
    )
}
pub(super) fn straight_segment(curve: &NurbsCurve, tolerance: Tolerance) -> bool {
    let controls = curve.control_points();
    if controls
        .iter()
        .any(|p| p.weight().is_sign_positive() != controls[0].weight().is_sign_positive())
    {
        return false;
    }
    let certify = || -> Result<bool, GeometryError> {
        let domain = curve.domain();
        let start = curve.evaluate(*domain.start())?.to_array().map(exact);
        let end = curve.evaluate(*domain.end())?.to_array().map(exact);
        let delta = std::array::from_fn::<_, 3, _>(|i| &end[i] - &start[i]);
        let dot = |a: &[BigRational; 3], b: &[BigRational; 3]| {
            (0..3).fold(BigRational::zero(), |sum, i| sum + &a[i] * &b[i])
        };
        let length_squared = dot(&delta, &delta);
        if length_squared.is_zero() {
            return Ok(false);
        }
        let limit = exact(tolerance.absolute());
        let limit = &limit * &limit;
        for control in controls {
            let point = control.point().to_array().map(exact);
            let offset = std::array::from_fn::<_, 3, _>(|i| &point[i] - &start[i]);
            let projection = dot(&offset, &delta);
            let distance_squared = if projection <= BigRational::zero() {
                dot(&offset, &offset)
            } else if projection >= length_squared {
                let offset = std::array::from_fn::<_, 3, _>(|i| &point[i] - &end[i]);
                dot(&offset, &offset)
            } else {
                dot(&offset, &offset) - &projection * &projection / &length_squared
            };
            if distance_squared > limit {
                return Ok(false);
            }
        }
        Ok(true)
    };
    // Same-sign rational basis functions keep the entire curve in the
    // control hull. A continuous curve with these endpoints covers the
    // segment, so this bounds the whole locus rather than sampled stations.
    certify().unwrap_or(false)
}

fn matching(
    curves: &[NurbsCurve],
    start: Point3,
    end: Point3,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve>, GeometryError> {
    for curve in curves.iter().filter(|c| straight_segment(c, tolerance)) {
        let domain = curve.domain();
        let a = curve.evaluate(*domain.start())?;
        let b = curve.evaluate(*domain.end())?;
        if start.is_near(a, tolerance) && end.is_near(b, tolerance) {
            return Ok(Some(linear_with_domain(a, b, curve.domain())?));
        }
        if start.is_near(b, tolerance) && end.is_near(a, tolerance) {
            let reversed = curve.reversed()?;
            return Ok(Some(linear_with_domain(b, a, reversed.domain())?));
        }
    }
    Ok(None)
}
// A fixed-parameter bilinear section is admitted only when the two opposite
// rational edge equations have exactly the same root. This preserves source
// UV parameter domains without fitting or using endpoint sampling to infer it.
fn bilinear_isocurves(
    surface: &NurbsSurface,
    frame: Frame3,
) -> Result<Vec<(NurbsCurve, f64)>, GeometryError> {
    if surface.degree_u() != 1
        || surface.degree_v() != 1
        || surface.control_point_count_u() != 2
        || surface.control_point_count_v() != 2
    {
        return Ok(Vec::new());
    }
    let value = |u, v| {
        let p = surface.control_point(u, v).unwrap();
        projected(p.point(), frame.origin(), frame.z_axis().as_vector()) * exact(p.weight())
    };
    let f = [value(0, 0), value(1, 0), value(0, 1), value(1, 1)];
    let mut result = Vec::new();
    for (axis, edges) in [(0, [[0, 1], [2, 3]]), (1, [[0, 2], [1, 3]])] {
        let mut root = None;
        let mut valid = true;
        for [a, b] in edges {
            let slope = &f[b] - &f[a];
            if slope.is_zero() {
                if !f[a].is_zero() {
                    valid = false;
                    break;
                }
                continue;
            }
            let r = -&f[a] / slope;
            if r < BigRational::zero()
                || r > BigRational::from_integer(BigInt::from(1))
                || root.as_ref().is_some_and(|first| first != &r)
            {
                valid = false;
                break;
            }
            root = Some(r);
        }
        if valid && let Some(root) = root {
            let domain = if axis == 0 {
                surface.domain_u()
            } else {
                surface.domain_v()
            };
            let parameter = (exact(*domain.start())
                + root * (exact(*domain.end()) - exact(*domain.start())))
            .to_f64()
            .filter(|v| v.is_finite())
            .ok_or(GeometryError::NonFinite {
                context: "Contour isocurve parameter",
            })?;
            let curve = if axis == 0 {
                surface.isocurve_v(parameter)?
            } else {
                surface.isocurve_u(parameter)?
            };
            result.push((curve, parameter));
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }
    #[test]
    fn line_certification_rejects_a_collinear_curve_that_overshoots_its_endpoints() {
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(10., 0.), p(1., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        assert!(curve.evaluate(0.5).unwrap().x() > 5.);
        assert!(!straight_segment(&curve, Tolerance::DEFAULT));
    }
    #[test]
    fn line_certification_handles_higher_degree_and_rejects_hidden_transverse_lobes() {
        let curve = NurbsCurve::try_new(
            3,
            vec![p(0., 0.), p(0.25, 0.), p(0.75, 0.), p(1., 0.)],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        assert!(straight_segment(&curve, Tolerance::DEFAULT));
        let lobe = NurbsCurve::try_new(
            3,
            vec![p(0., 0.), p(0.25, 1.), p(0.75, -1.), p(1., 0.)],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        assert_eq!(lobe.evaluate(0.5).unwrap().y(), 0.);
        assert!(lobe.evaluate(0.25).unwrap().y().abs() > 0.1);
        assert!(!straight_segment(&lobe, Tolerance::DEFAULT));
    }
}
