use super::*;
use viboceros_geometry::SurfaceCurvature;

pub(super) struct CurvatureCommand;
const USAGE: &str = "Curvature [MarkCurvature=Yes|No] point-on-selected-curve-or-surface";

enum Evaluation {
    Curve {
        parameter: f64,
        point: Point3,
        tangent: UnitVector3,
        curvature: Vector3,
    },
    Surface {
        face: Option<usize>,
        parameter: [f64; 2],
        curvature: SurfaceCurvature,
        bounds_diagonal: f64,
    },
}

enum Target<'a> {
    Curve(CurveRef<'a>, f64),
    Surface(&'a NurbsSurface, Option<usize>, [f64; 2], bool, f64),
}

impl Target<'_> {
    fn point(&self) -> Result<Point3, GeometryError> {
        match self {
            Self::Curve(curve, t) => curve.evaluate(*t),
            Self::Surface(s, _, [u, v], _, _) => s.evaluate(*u, *v),
        }
    }
    fn evaluate(self) -> Result<Evaluation, GeometryError> {
        Ok(match self {
            Self::Curve(curve, parameter) => {
                let sample = curve.evaluate_with_tangent(parameter)?;
                Evaluation::Curve {
                    parameter,
                    point: sample.point(),
                    tangent: sample.tangent(),
                    curvature: curve.curvature_vector(parameter)?,
                }
            }
            Self::Surface(s, face, parameter, reversed, bounds_diagonal) => {
                let c = s.curvature_at(parameter[0], parameter[1])?;
                Evaluation::Surface {
                    face,
                    parameter,
                    curvature: if reversed { c.reversed() } else { c },
                    bounds_diagonal,
                }
            }
        })
    }
}

impl Evaluation {
    fn point(&self) -> Point3 {
        match self {
            Self::Curve { point, .. } => *point,
            Self::Surface { curvature, .. } => curvature.point,
        }
    }

    fn report(&self) -> Result<String, GeometryError> {
        Ok(match self {
            Self::Curve {
                parameter,
                point,
                curvature,
                ..
            } => {
                let magnitude = curvature.length()?;
                format!(
                    "Curve curvature at parameter {parameter:.15}: point {}; curvature {magnitude:.15}; radius {}",
                    point_text(*point),
                    if magnitude == 0.0 {
                        "infinite".to_owned()
                    } else {
                        format!("{:.15}", 1.0 / magnitude)
                    }
                )
            }
            Self::Surface {
                face,
                parameter,
                curvature: c,
                ..
            } => {
                format!(
                    "Surface curvature{} at parameter {:.15},{:.15}: point {}; normal {}; maximum-absolute principal {:.15}; minimum-absolute principal {:.15}; Gaussian {:.15}; mean {:.15}",
                    face.map_or(String::new(), |i| format!(" on face {i}")),
                    parameter[0],
                    parameter[1],
                    point_text(c.point),
                    vector_text(c.normal.as_vector()),
                    c.principal[0],
                    c.principal[1],
                    c.gaussian()?,
                    c.mean()
                )
            }
        })
    }

    fn markers(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, GeometryError> {
        let mut result = vec![Geometry::Point(self.point())];
        match self {
            Self::Curve {
                point,
                tangent,
                curvature,
                ..
            } => {
                let k = curvature.length()?;
                if marker_curvature(k) {
                    let direction = curvature.normalized_nonzero()?;
                    let radius = 1.0 / k;
                    let center = point.translated(direction.as_vector().scaled(radius)?)?;
                    let normal = direction
                        .as_vector()
                        .cross(tangent.as_vector())?
                        .normalized_nonzero()?;
                    result.push(Geometry::Circle(Circle3::try_from_frame(
                        center,
                        radius,
                        direction.opposite(),
                        normal,
                        marker_tolerance(tolerance, radius)?,
                    )?));
                }
            }
            Self::Surface {
                curvature: c,
                bounds_diagonal,
                ..
            } => {
                for (&k, direction) in c.principal.iter().zip(c.directions) {
                    if !marker_curvature(k) {
                        let offset = direction.as_vector().scaled(0.1 * bounds_diagonal)?;
                        result.push(Geometry::Line(LineSegment::try_new(
                            c.point.translated(offset.scaled(-1.0)?)?,
                            c.point.translated(offset)?,
                            marker_tolerance(tolerance, 0.1 * bounds_diagonal)?,
                        )?));
                        continue;
                    }
                    let radius = 1.0 / k;
                    let center = c.point.translated(c.normal.as_vector().scaled(radius)?)?;
                    let first = center.translated(direction.as_vector().scaled(-radius)?)?;
                    let last = center.translated(direction.as_vector().scaled(radius)?)?;
                    result.push(Geometry::Arc(CircularArc3::try_from_three_points(
                        first,
                        c.point,
                        last,
                        marker_tolerance(tolerance, radius.abs())?,
                    )?));
                }
            }
        }
        Ok(result)
    }
}

// Rhino's public osculating-circle helper excludes radii outside this range.
// This is a marker policy, not a clamp on reported principal curvatures.
fn marker_curvature(k: f64) -> bool {
    (1e-16..=1e16).contains(&k.abs())
}

fn marker_tolerance(tolerance: Tolerance, size: f64) -> Result<Tolerance, GeometryError> {
    Tolerance::try_new(
        tolerance.absolute().min(size * 1e-9),
        tolerance.relative(),
        tolerance.angular(),
    )
}

impl Command for CurvatureCommand {
    fn name(&self) -> &'static str {
        "Curvature"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (mut cursor, mut mark, mut point) = (0, false, None);
        while cursor < arguments.len() {
            let token = arguments[cursor];
            if let Some((name, value)) = token.split_once('=')
                && option_name_eq(name, "MarkCurvature")
            {
                mark = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                cursor += 1;
            } else if option_name_eq(token, "MarkCurvature") {
                mark = !mark;
                cursor += 1;
            } else if point.is_none() {
                let (value, consumed) = parse_point(&arguments[cursor..])?;
                point = Some(value);
                cursor += consumed;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let target = point.ok_or(CommandError::Usage(USAGE))?;
        let tolerance = document.tolerance();
        let mut best: Option<(f64, Target<'_>)> = None;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        for object in document.selected_objects() {
            let geometry = object.geometry();
            let value = if let Some(curve) = geometry_curve_ref(geometry) {
                let parameter = curve.closest_parameter(target, tolerance)?;
                Some(Target::Curve(curve, parameter))
            } else {
                match geometry {
                    Geometry::NurbsSurface(surface) => {
                        let (u, v) = surface.closest_parameters(target, tolerance)?;
                        let bounds = geometry.bounds();
                        Some(Target::Surface(
                            surface,
                            None,
                            [u, v],
                            false,
                            bounds.min().distance_to(bounds.max())?,
                        ))
                    }
                    Geometry::Brep(brep) => {
                        if let Some((face, u, v)) =
                            brep.closest_face_parameters(target, tolerance)?
                        {
                            let f = &brep.faces()[face];
                            Some(Target::Surface(
                                f.surface(),
                                Some(face),
                                [u, v],
                                f.is_reversed(),
                                geometry
                                    .bounds()
                                    .min()
                                    .distance_to(geometry.bounds().max())?,
                            ))
                        } else {
                            None
                        }
                    }
                    _ => return Err(CommandError::CurvatureRequiresCurveOrSurface),
                }
            };
            if let Some(value) = value {
                let distance = target.distance_to(value.point()?)?;
                if best.as_ref().is_none_or(|(d, _)| distance < *d) {
                    best = Some((distance, value));
                }
            }
        }
        let (_, target) = best.ok_or(CommandError::CurvatureOutsideTrimmedFaces)?;
        let value = target.evaluate()?;
        let message = value.report()?;
        if mark {
            let markers = value.markers(tolerance)?;
            for geometry in markers {
                document.add_geometry(geometry)?;
            }
        }
        Ok(message)
    }
}

fn point_text(point: Point3) -> String {
    let [x, y, z] = point.to_array();
    format!("{x:.15},{y:.15},{z:.15}")
}
fn vector_text(vector: Vector3) -> String {
    let [x, y, z] = vector.to_array();
    format!("{x:.15},{y:.15},{z:.15}")
}

#[cfg(test)]
mod tests;
