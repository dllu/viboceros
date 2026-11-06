//! Exact closed walks and endpoint restrictions of newly joined line chains.
use super::*;
use viboceros_geometry::{LineSegment, Polyline3};

pub(super) fn contours(
    polyline: &Polyline3,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<NurbsCurve>, CommandError> {
    let walks = split(polyline.vertices(), budget)?;
    if polyline.is_closed() || !walks.is_empty() {
        return Ok(walks);
    }
    let points = simplified(polyline.vertices(), tolerance)?;
    if points.len() < 4 {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for end in [false, true] {
        let endpoint = if end {
            *points.last().unwrap()
        } else {
            points[0]
        };
        for i in 0..points.len() - 1 {
            budget.work(1)?;
            if (!end && i == 0) || (end && i == points.len() - 2) {
                continue;
            }
            let line =
                LineSegment::try_new(points[i], points[i + 1], Tolerance::NUMERICAL_VALIDATION)?;
            let closest = line.closest_point(endpoint, Tolerance::NUMERICAL_VALIDATION)?;
            if closest.distance_to(points[i])? <= tolerance.absolute()
                || closest.distance_to(points[i + 1])? <= tolerance.absolute()
                || closest.distance_to(endpoint)? >= tolerance.absolute()
            {
                continue;
            }
            budget.work(if end { points.len() - i } else { i + 2 })?;
            let joint = closest.translated(closest.vector_to(endpoint)?.scaled(0.5)?)?;
            let mut cycle = if end {
                vec![joint]
            } else {
                points[..=i].to_vec()
            };
            if end {
                cycle.extend_from_slice(&points[i + 1..points.len() - 1]);
                cycle.push(joint);
            } else {
                cycle[0] = joint;
                cycle.push(joint);
            }
            result.extend(split(&cycle, budget)?);
        }
    }
    Ok(result)
}

fn simplified(points: &[Point3], tolerance: Tolerance) -> Result<Vec<Point3>, GeometryError> {
    let mut result = Vec::with_capacity(points.len());
    for &point in points {
        while result.len() >= 2 {
            let i = result.len() - 2;
            if result[i] == point {
                break;
            }
            let line = LineSegment::try_new(result[i], point, Tolerance::NUMERICAL_VALIDATION)?;
            let scale = [result[i], result[i + 1], point]
                .into_iter()
                .flat_map(|p| p.to_array())
                .map(Real::abs)
                .fold(Real::MIN_POSITIVE, Real::max);
            let epsilon = tolerance.absolute().min(64. * Real::EPSILON * scale);
            if line
                .closest_point(result[i + 1], Tolerance::NUMERICAL_VALIDATION)?
                .distance_to(result[i + 1])?
                > epsilon
            {
                break;
            }
            result.pop();
        }
        result.push(point);
    }
    Ok(result)
}

fn split(points: &[Point3], budget: &mut Budget) -> Result<Vec<NurbsCurve>, CommandError> {
    budget.work(points.len())?;
    let mut stack = Vec::<Point3>::new();
    let mut positions = BTreeMap::new();
    let mut result = Vec::new();
    let key = |p: Point3| p.to_array().map(|x| if x == 0. { 0 } else { x.to_bits() });
    for &point in points {
        if let Some(&i) = positions.get(&key(point)) {
            if stack.len() - i + 1 >= 4 {
                budget.output(stack.len() - i + 1)?;
            }
            let mut cycle = stack[i..].to_vec();
            cycle.push(point);
            if cycle.len() >= 4 {
                result.push(
                    Polyline3::try_new(cycle, Tolerance::NUMERICAL_VALIDATION)?
                        .to_native_nurbs()?,
                );
            }
            for p in &stack[i + 1..] {
                positions.remove(&key(*p));
            }
            stack.truncate(i + 1);
        } else {
            positions.insert(key(point), stack.len());
            stack.push(point);
        }
    }
    Ok(result)
}
