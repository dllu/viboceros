//! Plane-defining points and immutable frame/reference admission.
use super::*;
enum Definition {
    Frame(Frame3),
    Reference(String),
}
impl VibocerosApp {
    pub(in crate::app) fn accept_project_plane_point(&mut self, point: Point3) -> bool {
        let Some(mut getter) = self
            .intersection_prompt
            .as_ref()
            .and_then(|p| p.plane_getter().cloned())
        else {
            return false;
        };
        let result = (|| -> Result<Option<Definition>, String> {
            let tolerance = self.document.tolerance();
            if getter.mode == PlaneMode::AroundCurve {
                let id = getter.curve.unwrap();
                let curve = self
                    .document
                    .object(id)
                    .and_then(|o| o.geometry().curve_ref())
                    .ok_or("Reference curve is unavailable")?;
                let parameter = curve
                    .closest_parameter(point, tolerance)
                    .map_err(|e| e.to_string())?;
                infinite_plane::around_curve(&self.document, id, parameter)
                    .map_err(|e| e.to_string())?;
                return Ok(Some(Definition::Reference(format!(
                    "AroundCurve:{id}:{parameter}"
                ))));
            }
            let frame = match (getter.mode, getter.points.as_slice()) {
                (PlaneMode::Through, _) => Some(getter.frame.with_origin(point)),
                (_, []) => {
                    getter.points.push(point);
                    None
                }
                (PlaneMode::Rectangle, [start]) => {
                    let p = getter
                        .frame
                        .with_origin(*start)
                        .coordinates_of(point)
                        .map_err(|e| e.to_string())?;
                    if p[0].abs() <= tolerance.absolute() || p[1].abs() <= tolerance.absolute() {
                        return Err("Rectangle needs nonzero width and height".into());
                    }
                    Some(getter.frame.with_origin(*start))
                }
                (_, [start]) => {
                    start.direction_to(point).map_err(|e| e.to_string())?;
                    if getter.mode == PlaneMode::Vertical {
                        let frame = viboceros_command::construction_plane::vertical(
                            getter.frame,
                            *start,
                            point,
                            tolerance,
                        )
                        .map_err(|e| e.to_string())?;
                        return Ok(Some(Definition::Frame(frame)));
                    }
                    getter.points.push(point);
                    None
                }
                (PlaneMode::ThreePoint, [start, end]) => Some(
                    Frame3::try_from_points(*start, *end, point, tolerance)
                        .map_err(|e| e.to_string())?,
                ),
                _ => return Err("Choose an infinite plane point mode".into()),
            };
            Ok(frame.map(Definition::Frame))
        })();
        match result {
            Ok(Some(definition)) => {
                match definition {
                    Definition::Frame(frame) => self.accept_project_plane_frame(frame),
                    Definition::Reference(value) => self.add_project_plane(value),
                }
                true
            }
            Ok(None) => {
                self.intersection_prompt
                    .as_mut()
                    .unwrap()
                    .set_plane_getter(Some(getter));
                self.update_project_plane_marker();
                self.log_intersection_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
