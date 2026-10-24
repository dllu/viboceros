//! Remove pure contacts already represented by overlap boundaries.
use super::*;
use rstar::{AABB, RTree, RTreeObject};

struct Entry {
    points: [Point3; 2],
    envelope: AABB<[f64; 3]>,
}
impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}
fn bounds(points: [Point3; 2], padding: f64) -> Result<AABB<[f64; 3]>, CommandError> {
    let [a, b] = points.map(Point3::to_array);
    let min = std::array::from_fn(|i| a[i].min(b[i]) - padding);
    let max = std::array::from_fn(|i| a[i].max(b[i]) + padding);
    if min.iter().chain(&max).any(|v| !v.is_finite()) {
        return Err(GeometryError::NonFinite {
            context: "mesh intersection contact bounds",
        }
        .into());
    }
    Ok(AABB::from_corners(min, max))
}

pub(super) fn subtract(
    contacts: Vec<[Point3; 2]>,
    contours: &[Polyline3],
    tolerance: Tolerance,
) -> Result<Vec<[Point3; 2]>, CommandError> {
    if contacts.is_empty() || contours.is_empty() {
        return Ok(contacts);
    }
    let entries = contours
        .iter()
        .flat_map(|p| p.vertices().windows(2))
        .map(|p| {
            let points = [p[0], p[1]];
            Ok(Entry {
                points,
                envelope: bounds(points, 0.)?,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let tree = RTree::bulk_load(entries);
    let mut out = Vec::new();
    let mut work = 0;
    for points in contacts {
        if points[0].is_near(points[1], Tolerance::MESH_VALIDATION) {
            continue;
        }
        let line = LineSegment::try_new(points[0], points[1], Tolerance::MESH_VALIDATION)?;
        let direction = line.direction(Tolerance::MESH_VALIDATION)?.as_vector();
        let length = line.length()?;
        let epsilon = (tolerance.absolute() / length).min(1.);
        let mut intervals = Vec::new();
        for edge in tree.locate_in_envelope_intersecting(bounds(points, tolerance.absolute())?) {
            work += 1;
            if work > MAX_SPAN_OUTPUT_OBJECTS * 10 {
                return Err(too_many_span_outputs("mesh contact coverage"));
            }
            let [a, b] = edge.points;
            let mut collinear = true;
            for p in [a, b] {
                if direction.cross(points[0].vector_to(p)?)?.length()? > tolerance.absolute() {
                    collinear = false;
                    break;
                }
            }
            if !collinear {
                continue;
            }
            let a = line.closest_parameter(a, Tolerance::MESH_VALIDATION)?;
            let b = line.closest_parameter(b, Tolerance::MESH_VALIDATION)?;
            if a != b {
                intervals.push([a.min(b), a.max(b)]);
            }
        }
        intervals.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
        let mut end = 0.;
        let mut append = |a: f64, b: f64| -> Result<(), CommandError> {
            if b - a > epsilon {
                let segment = [line.point_at(a)?, line.point_at(b)?];
                if !segment[0].is_near(segment[1], Tolerance::MESH_VALIDATION) {
                    if out.len() >= MAX_SPAN_OUTPUT_OBJECTS {
                        return Err(too_many_span_outputs("mesh contact fragments"));
                    }
                    out.push(segment);
                }
            }
            Ok(())
        };
        for [a, b] in intervals {
            if a > end {
                append(end, a)?;
            }
            end = end.max(b);
        }
        append(end, 1.)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(x: f64, y: f64) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }
    #[test]
    fn interval_subtraction_preserves_disconnected_gaps_in_both_directions() {
        let contours = [0., 2.].map(|x| {
            Polyline3::try_new(
                vec![
                    point(x, 0.),
                    point(x + 1., 0.),
                    point(x + 1., 1.),
                    point(x, 1.),
                    point(x, 0.),
                ],
                Tolerance::MESH_VALIDATION,
            )
            .unwrap()
        });
        for mut line in [
            [point(0., 0.), point(4., 0.)],
            [point(4., 0.), point(0., 0.)],
        ] {
            let result = subtract(vec![line], &contours, Tolerance::DEFAULT).unwrap();
            let length = result
                .iter()
                .map(|[a, b]| a.distance_to(*b).unwrap())
                .sum::<f64>();
            assert!((length - 2.).abs() < 1e-10, "{result:?}");
            for segment in &result {
                let x = (segment[0].x() + segment[1].x()) / 2.;
                assert!((1.0..2.0).contains(&x) || (3.0..4.0).contains(&x));
            }
            line[0] = point(line[0].x(), 0.25);
            line[1] = point(line[1].x(), 0.25);
            assert_eq!(
                subtract(vec![line], &contours, Tolerance::DEFAULT).unwrap(),
                vec![line]
            );
        }
    }
}
