//! Cheap native curve features, shared by standalone curves and polycurve leaves.
//! NURBS arc-length features are supplied separately by the owning object's cache.
use super::ObjectSnapKind::{self, End, Mid, Quad};
use viboceros_geometry::{CurveRef, Point3};

pub(super) fn curve(source: CurveRef<'_>, emit: &mut impl FnMut(ObjectSnapKind, Point3)) {
    if let CurveRef::PolyCurve(polycurve) = source {
        // Flat native leaves share End/Mid enumeration. Curve-hover centers
        // are resolved separately after direct features.
        for segment in polycurve.segments() {
            leaf(segment.as_ref(), &mut |kind, point| {
                if matches!(kind, End | Mid) {
                    emit(kind, point);
                }
            });
        }
    } else {
        leaf(source, emit);
    }
}

fn leaf(source: CurveRef<'_>, emit: &mut impl FnMut(ObjectSnapKind, Point3)) {
    match source {
        CurveRef::Line(line) => {
            emit(End, line.start());
            emit(End, line.end());
            if let Ok(point) = line.point_at(0.5) {
                emit(Mid, point);
            }
        }
        CurveRef::Circle(circle) => {
            if let Ok(point) = circle.point_at_angle(std::f64::consts::PI) {
                emit(Mid, point);
            }
            if let Ok(points) = circle.quadrants() {
                for point in points {
                    emit(Quad, point);
                }
            }
        }
        CurveRef::Arc(arc) => {
            for point in [arc.start(), arc.end()].into_iter().flatten() {
                emit(End, point);
            }
            if let Ok(point) = arc.point_at(0.5) {
                emit(Mid, point);
            }
        }
        CurveRef::Ellipse(ellipse) => {
            if let Ok(point) = ellipse.point_at_angle(std::f64::consts::PI) {
                emit(Mid, point);
            }
            if let Ok(points) = ellipse.quadrants() {
                for point in points {
                    emit(Quad, point);
                }
            }
        }
        CurveRef::Polyline(polyline) => {
            for &point in polyline.vertices() {
                emit(End, point);
            }
            for line in polyline.segments() {
                if let Ok(point) = line.point_at(0.5) {
                    emit(Mid, point);
                }
            }
        }
        CurveRef::NurbsCurve(curve) => {
            let domain = curve.domain();
            for t in [*domain.start(), *domain.end()] {
                if let Ok(point) = curve.evaluate(t) {
                    emit(End, point);
                }
            }
        }
        CurveRef::PolyCurve(_) => unreachable!("PolyCurve3 leaves cannot contain composites"),
    }
}
