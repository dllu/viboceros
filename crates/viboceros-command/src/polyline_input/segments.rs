//! Analytic segments used by both the finished curve and the drafting preview.
use crate::native_length;
use viboceros_document::Geometry;
use viboceros_geometry::{
    Circle3, CircularArc3, CurveSegment3, Frame3, GeometryError, LineSegment, Point3, PolyCurve3,
    Polyline3, Tolerance, UnitVector3, Vector3,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Line(LineSegment),
    Arc(CircularArc3),
}

impl Segment {
    pub fn end(self) -> Result<Point3, GeometryError> {
        match self {
            Self::Line(line) => Ok(line.end()),
            Self::Arc(arc) => arc.point_at(1.),
        }
    }

    pub fn tangent(self) -> Result<UnitVector3, GeometryError> {
        match self {
            Self::Line(line) => line.start().direction_to(line.end()),
            Self::Arc(arc) => {
                let (sin, cos) = arc.sweep_radians().sin_cos();
                let x = arc.x_axis().as_vector().to_array();
                let y = arc.y_axis().as_vector().to_array();
                Vector3::try_from(std::array::from_fn(|axis| {
                    (-sin).mul_add(x[axis], cos * y[axis])
                }))?
                .normalized_nonzero()
            }
        }
    }

    pub(crate) fn line(
        start: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Ok(Self::Line(LineSegment::try_new(start, end, tolerance)?))
    }

    pub(super) fn tangent_arc(
        start: Point3,
        tangent: UnitVector3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let direction = start.direction_to(end)?;
        if direction.as_vector().cross(tangent.as_vector())?.length()? <= tolerance.angular() {
            return Self::line(start, end, tolerance);
        }
        Ok(Self::Arc(CircularArc3::try_from_start_tangent_end(
            start, tangent, end, tolerance,
        )?))
    }

    pub(super) fn radius_arc(
        start: Point3,
        tangent: UnitVector3,
        end: Point3,
        radius: f64,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let offset = start.vector_to(end)?;
        let tangent = tangent.as_vector();
        let cross = tangent.cross(offset)?;
        if cross.length()? <= offset.length()? * tolerance.angular() {
            return Self::line(start, end, tolerance);
        }
        let normal = cross.normalized_nonzero()?;
        let inward = normal.as_vector().cross(tangent)?.normalized_nonzero()?;
        let center = start.translated(inward.as_vector().scaled(radius)?)?;
        Ok(Self::Arc(CircularArc3::try_from_center_start_end_on_plane(
            center, start, end, normal, tolerance,
        )?))
    }

    pub(super) fn centered_arc(
        center: Point3,
        start: Point3,
        end: Point3,
        plane: Frame3,
        incoming: Option<UnitVector3>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let center = arc_center(center, start, incoming)?;
        let radial = center.direction_to(start)?.as_vector();
        let direction = center.direction_to(end)?;
        if let Some(tangent) = incoming {
            let radius = center.distance_to(start)?;
            let end = center.translated(direction.as_vector().scaled(radius)?)?;
            return Self::tangent_arc(start, tangent, end, tolerance);
        }
        let preferred = plane.x_axis().as_vector();
        let mut normal = radial.cross(direction.as_vector())?;
        if normal.length()? <= tolerance.angular() {
            normal = radial.cross(preferred)?;
            if normal.length()? <= tolerance.angular() {
                normal = radial.cross(plane.y_axis().as_vector())?;
            }
        } else if normal.cross(radial)?.dot(preferred)? < 0. {
            normal = normal.scaled(-1.)?;
        }
        let normal = normal.normalized_nonzero()?;
        Ok(Self::Arc(CircularArc3::try_from_center_start_end_on_plane(
            center, start, end, normal, tolerance,
        )?))
    }

    pub(super) fn angle_arc(
        center: Point3,
        start: Point3,
        degrees: f64,
        tangent: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let center = arc_center(center, start, Some(tangent))?;
        let radial = center.direction_to(start)?;
        let normal = radial.as_vector().cross(tangent.as_vector())?;
        let circle = Circle3::try_from_frame(
            center,
            start.distance_to(center)?,
            radial,
            normal.normalized_nonzero()?,
            tolerance,
        )?;
        let sweep = degrees.to_radians();
        Ok(Self::Arc(CircularArc3::try_from_circle_sweep(
            circle, sweep,
        )?))
    }

    fn physical(self) -> Result<CurveSegment3, GeometryError> {
        Ok(match self {
            Self::Line(line) => {
                let length =
                    native_length::length3d(line.start().vector_to(line.end())?.to_array());
                CurveSegment3::Line(line.try_reparameterized(0.0..=length)?)
            }
            Self::Arc(arc) => {
                let length = arc.radius() * arc.sweep_radians();
                CurveSegment3::Arc(arc.try_reparameterized(0.0..=length)?)
            }
        })
    }
}

fn arc_center(
    center: Point3,
    start: Point3,
    incoming: Option<UnitVector3>,
) -> Result<Point3, GeometryError> {
    let Some(axis) = incoming.map(UnitVector3::as_vector) else {
        return Ok(center);
    };
    let along = start.vector_to(center)?.dot(axis)?;
    center.translated(axis.scaled(-along)?)
}

pub(crate) fn line_polyline(
    vertices: Vec<Point3>,
    tolerance: Tolerance,
) -> Result<Polyline3, GeometryError> {
    let mut parameters = Vec::with_capacity(vertices.len());
    parameters.push(0.);
    for pair in vertices.windows(2) {
        let d = pair[0].vector_to(pair[1])?;
        parameters.push(parameters.last().unwrap() + native_length::length3d(d.to_array()));
    }
    Polyline3::try_with_parameters(vertices, parameters, tolerance)
}

pub(super) fn geometry(
    start: Point3,
    segments: &[Segment],
    closing: Option<Segment>,
    tolerance: Tolerance,
) -> Result<Geometry, GeometryError> {
    let chain = segments.iter().copied().chain(closing);
    let all_lines = chain
        .clone()
        .all(|segment| matches!(segment, Segment::Line(_)));
    if all_lines {
        let mut vertices = Vec::with_capacity(1 + segments.len() + usize::from(closing.is_some()));
        vertices.push(start);
        for segment in chain {
            vertices.push(segment.end()?);
        }
        return Ok(Geometry::Polyline(line_polyline(vertices, tolerance)?));
    }

    let curves = chain
        .map(Segment::physical)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Geometry::PolyCurve(PolyCurve3::try_new(curves)?))
}
