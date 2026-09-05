//! Document geometry and representation-preserving operations.

use viboceros_geometry::{
    AffineTransform3, BoundingBox3, Brep, Circle3, CircularArc3, Curve3, CurveRef, Ellipse3,
    GeometryError, LineSegment, NurbsCurve, NurbsSurface, Point3, PointCloud3, PointMorph,
    PolyCurve3, Polyline3, Tolerance, TriangleMesh,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(Point3),
    PointCloud(PointCloud3),
    Line(LineSegment),
    Circle(Circle3),
    Arc(CircularArc3),
    Ellipse(Ellipse3),
    Polyline(Polyline3),
    NurbsCurve(NurbsCurve),
    PolyCurve(PolyCurve3),
    NurbsSurface(NurbsSurface),
    Brep(Brep),
    Mesh(TriangleMesh),
}

impl Geometry {
    pub fn curve_ref(&self) -> Option<CurveRef<'_>> {
        match self {
            Self::Line(curve) => Some(CurveRef::Line(curve)),
            Self::Circle(curve) => Some(CurveRef::Circle(curve)),
            Self::Arc(curve) => Some(CurveRef::Arc(curve)),
            Self::Ellipse(curve) => Some(CurveRef::Ellipse(curve)),
            Self::Polyline(curve) => Some(CurveRef::Polyline(curve)),
            Self::NurbsCurve(curve) => Some(CurveRef::NurbsCurve(curve)),
            Self::PolyCurve(curve) => Some(CurveRef::PolyCurve(curve)),
            _ => None,
        }
    }

    pub fn bounds(&self) -> BoundingBox3 {
        match self {
            Self::Point(point) => BoundingBox3::from_points([*point]).unwrap(),
            Self::PointCloud(cloud) => cloud.bounds(),
            Self::Line(line) => BoundingBox3::from_points([line.start(), line.end()]).unwrap(),
            Self::Circle(circle) => circle.bounds(),
            Self::Arc(arc) => arc.bounds(),
            Self::Ellipse(ellipse) => ellipse.bounds(),
            Self::Polyline(polyline) => polyline.bounds(),
            Self::NurbsCurve(curve) => curve.control_point_bounds(),
            Self::PolyCurve(curve) => curve.control_point_bounds(),
            Self::NurbsSurface(surface) => surface.control_point_bounds(),
            Self::Brep(brep) => brep.bounds(),
            Self::Mesh(mesh) => mesh.bounds(),
        }
    }

    /// Returns exact NURBS geometry for a supported non-NURBS curve.
    ///
    /// Polylines receive chord-length parameters. Other families retain their
    /// native intervals (by default, length for lines/circular curves and
    /// radians for ellipses). Polycurve control structure need not be minimal.
    /// Existing NURBS geometry and non-curve objects return `None`.
    pub fn converted_to_nurbs_curve(&self) -> Result<Option<NurbsCurve>, GeometryError> {
        if matches!(self, Self::NurbsCurve(_)) {
            return Ok(None);
        }
        if let Self::Polyline(curve) = self {
            return curve.to_nurbs().map(Some);
        }
        self.curve_ref().map(CurveRef::to_nurbs).transpose()
    }

    /// Returns an exact NURBS representation for every supported curve,
    /// retaining its native interval and cloning existing NURBS geometry.
    /// Unlike explicit `ToNURBS`, this does not give polylines new
    /// chord-length parameters: algorithmic consumers need the original map.
    pub fn nurbs_curve_representation(&self) -> Result<Option<NurbsCurve>, GeometryError> {
        self.curve_ref().map(CurveRef::to_nurbs).transpose()
    }

    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Point(point) => Self::Point(transform.transform_point(*point)?),
            Self::PointCloud(cloud) => Self::PointCloud(cloud.transformed(transform)?),
            Self::Line(line) => Self::Line(line.transformed(transform, tolerance)?),
            Self::Circle(circle) => match circle.transformed_similarity(transform, tolerance)? {
                Some(circle) => Self::Circle(circle),
                None => Self::NurbsCurve(circle.to_nurbs()?.transformed(transform)?),
            },
            Self::Arc(arc) => match arc.transformed_similarity(transform, tolerance)? {
                Some(arc) => Self::Arc(arc),
                None => Self::NurbsCurve(arc.to_nurbs()?.transformed(transform)?),
            },
            Self::Ellipse(ellipse) => {
                match ellipse.transformed_orthogonal(transform, tolerance)? {
                    Some(ellipse) => Self::Ellipse(ellipse),
                    None => Self::NurbsCurve(ellipse.to_nurbs()?.transformed(transform)?),
                }
            }
            Self::Polyline(polyline) => Self::Polyline(polyline.transformed(transform, tolerance)?),
            Self::NurbsCurve(curve) => Self::NurbsCurve(curve.transformed(transform)?),
            Self::PolyCurve(curve) => Self::PolyCurve(curve.transformed(transform)?),
            Self::NurbsSurface(surface) => Self::NurbsSurface(surface.transformed(transform)?),
            Self::Brep(brep) => Self::Brep(brep.transformed(transform, tolerance)?),
            Self::Mesh(mesh) => Self::Mesh(mesh.transformed(transform, tolerance)?),
        })
    }

    /// Applies a non-affine point morph while retaining the richest geometry
    /// representation supported by the kernel. Linear primitives become
    /// cubic NURBS curves so their interiors follow the morph.
    pub fn morphed(
        &self,
        morph: &(impl PointMorph + ?Sized),
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Point(point) => Self::Point(morph.morph_point(*point)?),
            Self::PointCloud(cloud) => Self::PointCloud(morph.morph_point_cloud(cloud)?),
            Self::Line(line) => Self::NurbsCurve(morph.morph_line(*line, tolerance)?),
            Self::Circle(circle) => {
                Self::NurbsCurve(morph.morph_nurbs_curve(&circle.to_nurbs()?, tolerance)?)
            }
            Self::Arc(arc) => {
                Self::NurbsCurve(morph.morph_nurbs_curve(&arc.to_nurbs()?, tolerance)?)
            }
            Self::Ellipse(ellipse) => {
                Self::NurbsCurve(morph.morph_nurbs_curve(&ellipse.to_nurbs()?, tolerance)?)
            }
            Self::Polyline(polyline) => {
                Self::NurbsCurve(morph.morph_polyline(polyline, tolerance)?)
            }
            Self::NurbsCurve(curve) => Self::NurbsCurve(morph.morph_nurbs_curve(curve, tolerance)?),
            Self::PolyCurve(curve) => Self::PolyCurve(PolyCurve3::try_with_segment_domains(
                curve
                    .segments()
                    .iter()
                    .map(|segment| match segment {
                        viboceros_geometry::CurveSegment3::Line(line) => {
                            morph.morph_line(*line, tolerance)
                        }
                        viboceros_geometry::CurveSegment3::Polyline(polyline) => {
                            morph.morph_polyline(polyline, tolerance)
                        }
                        _ => morph.morph_nurbs_curve(&segment.to_nurbs()?, tolerance),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                curve.parameters().to_vec(),
            )?),
            Self::NurbsSurface(surface) => {
                Self::NurbsSurface(morph.morph_nurbs_surface(surface, tolerance)?)
            }
            Self::Brep(brep) => Self::Brep(morph.morph_brep(brep, tolerance)?),
            Self::Mesh(mesh) => Self::Mesh(morph.morph_mesh(mesh, tolerance)?),
        })
    }

    /// Returns the defining locations duplicated by Rhino's `ExtractPt`
    /// command. Point objects produce no new points; closed curve seams and
    /// periodic NURBS controls follow Rhino's unique-grip ordering.
    pub fn extract_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        Ok(match self {
            Self::Point(_) => Vec::new(),
            Self::PointCloud(cloud) => cloud.points().to_vec(),
            Self::Line(line) => vec![line.start(), line.end()],
            Self::Circle(circle) => circle.to_nurbs()?.extract_point_locations()?,
            Self::Arc(arc) => arc.to_nurbs()?.extract_point_locations()?,
            Self::Ellipse(ellipse) => ellipse.to_nurbs()?.extract_point_locations()?,
            Self::Polyline(polyline) => {
                let mut points = polyline.vertices().to_vec();
                if polyline.is_closed() {
                    points.pop();
                }
                points
            }
            Self::NurbsCurve(curve) => curve.extract_point_locations()?,
            Self::PolyCurve(curve) => curve.extract_point_locations()?,
            Self::NurbsSurface(surface) => surface.extract_point_locations(),
            Self::Brep(brep) => brep
                .vertices()
                .iter()
                .map(|vertex| vertex.point())
                .collect(),
            Self::Mesh(mesh) => mesh.vertices().to_vec(),
        })
    }
}

impl From<Curve3> for Geometry {
    fn from(curve: Curve3) -> Self {
        match curve {
            Curve3::Line(curve) => Self::Line(curve),
            Curve3::Circle(curve) => Self::Circle(curve),
            Curve3::Arc(curve) => Self::Arc(curve),
            Curve3::Ellipse(curve) => Self::Ellipse(curve),
            Curve3::Polyline(curve) => Self::Polyline(curve),
            Curve3::NurbsCurve(curve) => Self::NurbsCurve(curve),
            Curve3::PolyCurve(curve) => Self::PolyCurve(curve),
        }
    }
}
