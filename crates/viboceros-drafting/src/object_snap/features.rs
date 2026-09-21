//! Cheap native curve features, shared by standalone curves and polycurve leaves.
//! NURBS arc-length features are supplied separately by the owning object's cache.
use super::ObjectSnapKind::{self, End, Mid, Quad};
use super::ObjectSnapModes;
use viboceros_geometry::{CurveRef, Point3};

#[cfg(test)]
mod tests;

pub(super) fn curve(
    source: CurveRef<'_>,
    modes: ObjectSnapModes,
    emit: &mut impl FnMut(ObjectSnapKind, Point3),
) {
    // Polygon/curve Center and Point-only queries must not walk all vertices
    // or evaluate disabled analytic features merely to discard their output.
    let modes = if matches!(source, CurveRef::PolyCurve(_)) {
        modes.with(Quad, false)
    } else {
        modes
    };
    if ![End, Mid, Quad]
        .into_iter()
        .any(|kind| modes.contains(kind))
    {
        return;
    }
    if let CurveRef::PolyCurve(polycurve) = source {
        // Flat native leaves share End/Mid enumeration. Curve-hover centers
        // are resolved separately after direct features.
        for segment in polycurve.segments() {
            leaf(segment.as_ref(), modes, emit);
        }
    } else {
        leaf(source, modes, emit);
    }
}

fn leaf(
    source: CurveRef<'_>,
    modes: ObjectSnapModes,
    emit: &mut impl FnMut(ObjectSnapKind, Point3),
) {
    match source {
        CurveRef::Line(line) => {
            if modes.contains(End) {
                emit(End, line.start());
                emit(End, line.end());
            }
            if modes.contains(Mid)
                && let Ok(point) = line.point_at(0.5)
            {
                emit(Mid, point);
            }
        }
        CurveRef::Circle(circle) => {
            if modes.contains(Mid)
                && let Ok(point) = circle.point_at_angle(std::f64::consts::PI)
            {
                emit(Mid, point);
            }
            if modes.contains(Quad)
                && let Ok(points) = circle.quadrants()
            {
                for point in points {
                    emit(Quad, point);
                }
            }
        }
        CurveRef::Arc(arc) => {
            if modes.contains(End) {
                for point in [arc.start(), arc.end()].into_iter().flatten() {
                    emit(End, point);
                }
            }
            if modes.contains(Mid)
                && let Ok(point) = arc.point_at(0.5)
            {
                emit(Mid, point);
            }
        }
        CurveRef::Ellipse(ellipse) => {
            if modes.contains(Mid)
                && let Ok(point) = ellipse.point_at_angle(std::f64::consts::PI)
            {
                emit(Mid, point);
            }
            if modes.contains(Quad)
                && let Ok(points) = ellipse.quadrants()
            {
                for point in points {
                    emit(Quad, point);
                }
            }
        }
        CurveRef::Polyline(polyline) => {
            if modes.contains(End) {
                for &point in polyline.vertices() {
                    emit(End, point);
                }
            }
            if modes.contains(Mid) {
                for line in polyline.segments() {
                    if let Ok(point) = line.point_at(0.5) {
                        emit(Mid, point);
                    }
                }
            }
        }
        CurveRef::NurbsCurve(curve) if modes.contains(End) => {
            let domain = curve.domain();
            for t in [*domain.start(), *domain.end()] {
                if let Ok(point) = curve.evaluate(t) {
                    emit(End, point);
                }
            }
        }
        CurveRef::NurbsCurve(_) => {}
        CurveRef::PolyCurve(_) => unreachable!("PolyCurve3 leaves cannot contain composites"),
    }
}
