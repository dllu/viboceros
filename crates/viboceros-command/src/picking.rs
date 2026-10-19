//! Camera context for view-dependent getters, separate from model edits.
use crate::CommandError;
use viboceros_geometry::{Frame3, NurbsCurve, Point3, Tolerance, WeightedPoint3};
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

#[derive(Clone, Copy, Debug)]
pub struct Projection {
    frame: Frame3,
    scale: [f64; 2],
    offset: [f64; 2],
    perspective: bool,
}
impl Projection {
    pub fn from_view(view: &ThreeDmNamedView) -> Result<Self, CommandError> {
        let direction = view.camera_direction.normalized_nonzero()?.as_vector();
        let hint = view.camera_up.normalized_nonzero()?.as_vector();
        let right = direction.cross(hint)?.normalized_nonzero()?.as_vector();
        let up = right.cross(direction)?;
        let frame =
            Frame3::try_from_directions(view.camera_location, right, up, Tolerance::DEFAULT)?;
        let [left, right, bottom, top, near, _] = view.frustum;
        let width =
            (i64::from(view.screen_port[1]) - i64::from(view.screen_port[0])).unsigned_abs() as f64;
        let height =
            (i64::from(view.screen_port[3]) - i64::from(view.screen_port[2])).unsigned_abs() as f64;
        let perspective = view.projection != ThreeDmProjection::Parallel;
        let factor = if perspective { near } else { 1. };
        let scale = [
            factor * width / (right - left),
            factor * height / (top - bottom),
        ];
        if scale.iter().any(|s| !s.is_finite() || *s <= 0.) {
            return Err(CommandError::Usage("Invalid picking view"));
        }
        let offset = [
            (f64::from(view.screen_port[0]) + f64::from(view.screen_port[1])) * 0.5
                - (right + left) * width / (2. * (right - left)),
            (f64::from(view.screen_port[2]) + f64::from(view.screen_port[3])) * 0.5
                - (top + bottom) * height / (2. * (top - bottom)),
        ];
        if offset.iter().any(|x| !x.is_finite()) {
            return Err(CommandError::Usage("Invalid picking view"));
        }
        Ok(Self {
            frame,
            scale,
            offset,
            perspective,
        })
    }
    fn homogeneous(self, point: Point3) -> Result<[f64; 3], CommandError> {
        let [x, y, z] = self.frame.coordinates_of(point)?;
        let w = if self.perspective { -z } else { 1. };
        Ok([
            x * self.scale[0] + self.offset[0] * w,
            y * self.scale[1] + self.offset[1] * w,
            w,
        ])
    }
    /// Point getters use integer device coordinates, with device Y increasing upwards.
    pub fn point(self, point: Point3) -> Result<Point3, CommandError> {
        let p = self.continuous_point(point)?;
        Ok(Point3::try_new(p.x().trunc(), p.y().trunc(), 0.)?)
    }
    /// Continuous device coordinates for control locations and projected guides.
    pub fn continuous_point(self, point: Point3) -> Result<Point3, CommandError> {
        let [x, y, w] = self.homogeneous(point)?;
        if w <= 0. {
            return Err(CommandError::Usage(
                "Knot guides could not be projected in this view",
            ));
        }
        Ok(Point3::try_new(x / w, y / w, 0.)?)
    }
    /// A perspective image remains rational. The current bounded picker admits
    /// guides whose complete control hull lies in front of the camera.
    pub fn curve(self, curve: &NurbsCurve) -> Result<NurbsCurve, CommandError> {
        let controls = curve
            .control_points()
            .iter()
            .map(|control| {
                let [x, y, w] = self.homogeneous(control.point())?;
                if w <= 0. {
                    return Err(CommandError::Usage(
                        "Knot guides could not be projected in this view",
                    ));
                }
                Ok(WeightedPoint3::try_new(
                    Point3::try_new(x / w, y / w, 0.)?,
                    control.weight() * w,
                )?)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(NurbsCurve::try_new_rational(
            curve.degree(),
            controls,
            curve.knots().to_vec(),
        )?)
    }
}
