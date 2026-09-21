//! Cached corner-average Center targets for closed piecewise-linear boundaries.
use super::{SnapMetric, proximity};
use std::collections::{BTreeMap, HashMap};
use viboceros_document::{Geometry, GeometrySnapshot, Object, ObjectId};
use viboceros_geometry::{
    BoundingBox3, Brep, BrepFace, CurveRef, FiniteSum, LineSegment, NurbsCurve, NurbsSurface,
    Point3, Real, Tolerance,
};

mod source;

#[derive(Debug)]
struct Target {
    point: Point3,
    segments: Vec<LineSegment>,
    bounds: BoundingBox3,
}

#[derive(Debug)]
struct Entry {
    source: GeometrySnapshot,
    tolerance: Tolerance,
    targets: Vec<Target>,
}

#[derive(Debug, Default)]
pub(super) struct Cache {
    entries: BTreeMap<ObjectId, Entry>,
    #[cfg(test)]
    builds: usize,
    #[cfg(test)]
    source_comparisons: usize,
}

pub(super) fn supported(geometry: &Geometry) -> bool {
    matches!(
        geometry,
        Geometry::Polyline(_)
            | Geometry::PolyCurve(_)
            | Geometry::NurbsCurve(_)
            | Geometry::NurbsSurface(_)
            | Geometry::Brep(_)
    )
}

impl Cache {
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn retain_objects(&mut self, live: &HashMap<ObjectId, &Geometry>) {
        self.entries.retain(|id, _| live.contains_key(id));
    }

    pub(super) fn visit(
        &mut self,
        object: &Object,
        tolerance: Tolerance,
        metric: &impl SnapMetric,
        emit: &mut impl FnMut(Point3, Real),
    ) {
        let geometry = object.geometry();
        let snapshot = object.geometry_snapshot();
        let id = object.id();
        if !supported(geometry) {
            return;
        }
        if !self.entries.get_mut(&id).is_some_and(|entry| {
            if entry.tolerance != tolerance {
                return false;
            }
            if entry.source.shares_storage_with(snapshot) {
                return true;
            }
            #[cfg(test)]
            {
                self.source_comparisons += 1;
            }
            if !source::same_target_source(&entry.source, geometry) {
                return false;
            }
            entry.source = snapshot.clone();
            true
        }) {
            #[cfg(test)]
            {
                self.builds += 1;
            }
            self.entries.insert(
                id,
                Entry {
                    source: snapshot.clone(),
                    tolerance,
                    targets: targets(geometry, tolerance),
                },
            );
        }
        for target in &self.entries[&id].targets {
            if metric.offset(target.point).is_none()
                || proximity::outside_bounds(
                    target.bounds.min().to_array(),
                    target.bounds.max().to_array(),
                    metric,
                )
            {
                continue;
            }
            let distance = target
                .segments
                .iter()
                .filter_map(|&line| proximity::line_distance(line, metric))
                .min_by(Real::total_cmp);
            if let Some(distance) = distance.filter(|&d| d <= metric.capture_radius()) {
                emit(target.point, distance);
            }
        }
    }
}

fn targets(geometry: &Geometry, tolerance: Tolerance) -> Vec<Target> {
    match geometry {
        Geometry::NurbsSurface(surface) => surface_target(surface, tolerance).into_iter().collect(),
        Geometry::Brep(brep) => brep
            .faces()
            .iter()
            .filter_map(|face| face_target(brep, face, tolerance))
            .collect(),
        _ => geometry
            .curve_ref()
            .and_then(vertices)
            .and_then(target)
            .into_iter()
            .collect(),
    }
}

fn target(mut points: Vec<Point3>) -> Option<Target> {
    // Singular surface sides collapse to a point, not an additional corner.
    // Keep non-adjacent repeated occurrences and all nonzero collinear sides.
    points.dedup();
    if points.len() < 4 || points.first() != points.last() {
        return None;
    }
    // Count every corner occurrence, including collinear/repeated vertices;
    // omit only the duplicated closing endpoint. Do not use an area centroid.
    let mut sums: [FiniteSum; 3] = std::array::from_fn(|_| FiniteSum::default());
    for point in &points[..points.len() - 1] {
        for (sum, coordinate) in sums.iter_mut().zip(point.to_array()) {
            sum.add(coordinate).ok()?;
        }
    }
    let point = Point3::try_new(
        sums[0].mean().ok()?,
        sums[1].mean().ok()?,
        sums[2].mean().ok()?,
    )
    .ok()?;
    let segments = points
        .windows(2)
        .map(|pair| LineSegment::try_new(pair[0], pair[1], Tolerance::NUMERICAL_VALIDATION).ok())
        .collect::<Option<Vec<_>>>()?;
    Some(Target {
        point,
        segments,
        bounds: BoundingBox3::from_points(points).ok()?,
    })
}

fn vertices(curve: CurveRef<'_>) -> Option<Vec<Point3>> {
    match curve {
        CurveRef::Line(line) => Some(vec![line.start(), line.end()]),
        CurveRef::Polyline(polyline) => Some(polyline.vertices().to_vec()),
        CurveRef::NurbsCurve(curve) => nurbs_vertices(curve),
        CurveRef::PolyCurve(curve) => {
            let mut points = Vec::new();
            for segment in curve.segments() {
                append(&mut points, vertices(segment.as_ref())?)?;
            }
            Some(points)
        }
        _ => None,
    }
}

fn nurbs_vertices(curve: &NurbsCurve) -> Option<Vec<Point3>> {
    let sign = curve.control_points()[0].weight().is_sign_positive();
    if !curve
        .control_points()
        .iter()
        .all(|p| p.weight().is_sign_positive() == sign)
    {
        return None; // A projective pole can turn a segment into unbounded rays.
    }
    if curve.degree() > 1 {
        let mut points = Vec::new();
        for span in curve.try_bezier_spans().ok()? {
            if !span.is_linear_at_zero_tolerance().ok()? {
                return None;
            }
            append(
                &mut points,
                vec![
                    span.evaluate(*span.domain().start()).ok()?,
                    span.evaluate(*span.domain().end()).ok()?,
                ],
            )?;
        }
        return Some(points);
    }
    let sampler = curve.parameter_sampler().ok()?;
    let mut points = Vec::new();
    for span in sampler.spans() {
        append(
            &mut points,
            vec![span.evaluate(0.).ok()?, span.evaluate(1.).ok()?],
        )?;
    }
    Some(points)
}

fn append(points: &mut Vec<Point3>, part: Vec<Point3>) -> Option<()> {
    let skip = usize::from(!points.is_empty());
    if skip == 1 && points.last() != part.first() {
        return None; // Never invent a segment across a discontinuity or gap.
    }
    points.extend(part.into_iter().skip(skip));
    Some(())
}

fn surface_target(surface: &NurbsSurface, tolerance: Tolerance) -> Option<Target> {
    planar_surface(surface, tolerance)?;
    let u = surface.domain_u();
    let v = surface.domain_v();
    let curves = [
        surface.isocurve_u(*v.start()).ok()?,
        surface.isocurve_v(*u.end()).ok()?,
        surface.isocurve_u(*v.end()).ok()?.reversed().ok()?,
        surface.isocurve_v(*u.start()).ok()?.reversed().ok()?,
    ];
    let mut points = Vec::new();
    for curve in &curves {
        append(&mut points, boundary_vertices(curve)?)?;
    }
    target(points)
}

fn face_target(brep: &Brep, face: &BrepFace, tolerance: Tolerance) -> Option<Target> {
    if face.loops().len() != 1 {
        return None;
    }
    planar_surface(face.surface(), tolerance)?;
    let mut points = Vec::new();
    for trim in face.loops()[0].trims() {
        let Some(edge) = trim.edge() else { continue }; // Validated singular trim.
        let mut part = boundary_vertices(brep.edges().get(edge)?.curve())?;
        if trim.is_reversed_3d() {
            part.reverse();
        }
        append(&mut points, part)?;
    }
    target(points)
}

fn planar_surface(surface: &NurbsSurface, tolerance: Tolerance) -> Option<()> {
    let plane = surface.plane(tolerance).ok()??;
    // Snap recognition uses a model-space absolute tolerance, not a growing
    // allowance proportional to the distance of the model from world zero.
    for control in surface.control_points() {
        if plane.signed_distance_to(control.point()).ok()?.abs() > tolerance.absolute() {
            return None;
        }
    }
    Some(())
}

fn boundary_vertices(curve: &NurbsCurve) -> Option<Vec<Point3>> {
    // A straight surface edge contributes its endpoints, not subdivisions of
    // its parameterization. Standalone polylines retain every stored vertex.
    if curve.is_linear_at_zero_tolerance().ok()? {
        let sign = curve.control_points()[0].weight().is_sign_positive();
        if !curve
            .control_points()
            .iter()
            .all(|p| p.weight().is_sign_positive() == sign)
        {
            return None;
        }
        Some(vec![
            curve.evaluate(*curve.domain().start()).ok()?,
            curve.evaluate(*curve.domain().end()).ok()?,
        ])
    } else {
        nurbs_vertices(curve)
    }
}

#[cfg(test)]
mod tests;
