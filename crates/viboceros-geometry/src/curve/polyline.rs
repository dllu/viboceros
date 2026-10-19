//! Polyline recognition retains the public native policy for NURBS spans.
use super::*;

impl CurveRef<'_> {
    /// Public native polyline recognition, without constructing new parameters.
    pub fn polyline_vertices(self) -> Result<Option<Vec<Point3>>, GeometryError> {
        Ok(match self {
            Self::Line(c) => Some(vec![c.start(), c.end()]),
            Self::Polyline(c) => Some(c.vertices().to_vec()),
            Self::NurbsCurve(c) => {
                if c.degree() == 1 {
                    return Ok(Some(c.control_points().iter().map(|p| p.point()).collect()));
                }
                let controls = c
                    .control_points()
                    .iter()
                    .map(|p| {
                        let [x, y, z] = p.point().to_array();
                        [x, y, z, p.weight()]
                    })
                    .collect::<Vec<_>>();
                viboceros_opennurbs::curve_polyline_vertices(c.degree(), &controls, c.knots())?
                    .map(|p| {
                        p.into_iter()
                            .map(Point3::try_from)
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
            }
            Self::PolyCurve(c) => {
                let mut vertices = Vec::new();
                for segment in c.segments() {
                    let Some(points) = segment.as_ref().polyline_vertices()? else {
                        return Ok(None);
                    };
                    if !vertices.is_empty() {
                        vertices.pop();
                    }
                    vertices.extend(points);
                }
                if c.is_closed()? && vertices.len() > 3 {
                    *vertices.last_mut().unwrap() = vertices[0];
                }
                Some(vertices)
            }
            _ => None,
        })
    }
}
