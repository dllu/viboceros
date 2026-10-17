//! Edit-free tilted Arc construction shared by commands and viewport input.
use crate::CommandError;
use viboceros_geometry::{Circle3, CircularArc3, Frame3, GeometryError, Point3, Real, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    center: Point3,
    start: Option<Point3>,
    frame: Option<Frame3>,
    orientation_height: Option<Real>,
    length: bool,
    clockwise: bool,
    cursor_angle: Option<Real>,
    cursor_x: Real,
    direction_locked: bool,
    length_start: Option<Point3>,
    tolerance: Tolerance,
}
impl Input {
    pub fn new(center: Point3, tolerance: Tolerance) -> Self {
        Self {
            center,
            start: None,
            frame: None,
            orientation_height: None,
            length: false,
            clockwise: false,
            cursor_angle: None,
            cursor_x: 0.,
            direction_locked: false,
            length_start: None,
            tolerance,
        }
    }
    pub fn center(self) -> Point3 {
        self.center
    }
    pub fn frame(self) -> Option<Frame3> {
        self.frame
    }
    pub fn requests_orientation(self) -> bool {
        self.start.is_some() && self.frame.is_none()
    }
    pub fn requests_length(self) -> bool {
        self.length
    }
    pub fn clockwise(self) -> bool {
        self.clockwise
    }
    /// A crossing of the start ray changes sweep direction. Both samples must
    /// be on the start's side of the center: a jump across the opposite half
    /// of the circle preserves direction. The first cursor seeds the side.
    pub fn observe_cursor(&mut self, point: Point3) -> Result<(), GeometryError> {
        let Some(frame) = self.frame else {
            return Ok(());
        };
        if self.length || self.direction_locked {
            return Ok(());
        }
        let radial = self.center.vector_to(point)?;
        let x = radial.dot(frame.x_axis().as_vector())?;
        let y = radial.dot(frame.y_axis().as_vector())?;
        if x.hypot(y) <= self.tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "tilted arc cursor direction",
            });
        }
        let angle = y.atan2(x);
        if let Some(previous) = self.cursor_angle {
            if self.cursor_x > 0. && x > 0. && (previous < 0.) != (angle < 0.) {
                self.clockwise = angle < 0.;
            }
        } else {
            self.clockwise = angle < 0.;
        }
        self.cursor_angle = Some(angle);
        self.cursor_x = x;
        Ok(())
    }
    pub fn anchor(self) -> Point3 {
        self.length_start.or(self.start).unwrap_or(self.center)
    }
    pub fn prompt(self) -> &'static str {
        if self.start.is_none() {
            "start of tilted arc"
        } else if self.frame.is_none() && self.orientation_height.is_some() {
            "choose tilt rectangle"
        } else if self.frame.is_none() {
            "arc orientation"
        } else if !self.length {
            "endpoint or signed angle; Length"
        } else if self.length_start.is_none() {
            if self.clockwise {
                "signed arc length or first distance point; Direction=Clockwise"
            } else {
                "signed arc length or first distance point; Direction=Counterclockwise"
            }
        } else {
            "second distance point"
        }
    }
    pub fn option(&mut self, word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        if self.frame.is_some()
            && let Some((key, value)) = word.split_once('=')
            && key.eq_ignore_ascii_case("Direction")
        {
            if value.eq_ignore_ascii_case("Clockwise") {
                self.clockwise = true;
                self.direction_locked = true;
                return true;
            }
            if value.eq_ignore_ascii_case("Counterclockwise") {
                self.clockwise = false;
                self.direction_locked = true;
                return true;
            }
        }
        if self.frame.is_some() && !self.length && word.eq_ignore_ascii_case("Length") {
            self.length = true;
            self.clockwise = false;
            return true;
        }
        false
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<CircularArc3>, CommandError> {
        if let Some(start) = self.start {
            if let Some(frame) = self.frame {
                if self.length {
                    if let Some(first) = self.length_start {
                        return self.number(first.distance_to(point)?);
                    }
                    self.length_start = Some(point);
                    return Ok(None);
                }
                if self.cursor_angle.is_some() {
                    self.observe_cursor(point)?;
                }
                return Ok(Some(CircularArc3::try_from_center_start_end_on_plane(
                    self.center,
                    start,
                    point,
                    if self.clockwise {
                        frame.z_axis().opposite()
                    } else {
                        frame.z_axis()
                    },
                    self.tolerance,
                )?));
            }
            let frame = Frame3::try_from_points(self.center, start, point, self.tolerance)?;
            self.frame = Some(frame);
        } else {
            if self.center.distance_to(point)? <= self.tolerance.absolute() {
                return Err(CommandError::Usage("Arc start must differ from its center"));
            }
            self.start = Some(point);
        }
        Ok(None)
    }
    pub fn number(&mut self, value: Real) -> Result<Option<CircularArc3>, CommandError> {
        if self.requests_orientation() {
            if !value.is_finite() || value <= self.tolerance.absolute() {
                return Err(CommandError::Usage(
                    "Enter a positive tilt rectangle height",
                ));
            }
            self.orientation_height = Some(value);
            return Ok(None);
        }
        let frame = self.frame.ok_or(CommandError::Usage(
            "Specify the tilted arc orientation first",
        ))?;
        if !value.is_finite() || value == 0. {
            return Err(CommandError::Usage(
                "Enter a nonzero finite arc angle or length",
            ));
        }
        let radius = self.center.distance_to(self.start.unwrap())?;
        let sweep = if self.length {
            value / radius * if self.clockwise { -1. } else { 1. }
        } else {
            value.to_radians()
        };
        let circle = Circle3::try_from_frame(
            self.center,
            radius,
            frame.x_axis(),
            if sweep < 0. {
                frame.z_axis().opposite()
            } else {
                frame.z_axis()
            },
            self.tolerance,
        )?;
        Ok(Some(CircularArc3::try_from_circle_sweep(
            circle,
            sweep.abs().min(std::f64::consts::TAU),
        )?))
    }
    /// A rectangle shows the two radial directions defining the new plane.
    pub fn orientation_guide(self, point: Point3) -> Option<[Point3; 4]> {
        if !self.requests_orientation() {
            return None;
        }
        let frame =
            Frame3::try_from_points(self.center, self.start?, point, self.tolerance).ok()?;
        let height = self.orientation_height.unwrap_or(
            self.center
                .vector_to(point)
                .ok()?
                .dot(frame.y_axis().as_vector())
                .ok()?,
        );
        let offset = frame.y_axis().as_vector().scaled(height).ok()?;
        Some([
            self.center,
            self.start?,
            self.start?.translated(offset).ok()?,
            self.center.translated(offset).ok()?,
        ])
    }
    pub fn preview(self, point: Point3) -> Option<CircularArc3> {
        let mut candidate = self;
        if self.requests_orientation() {
            candidate.point(point).ok()?;
            candidate.number(90.).ok().flatten()
        } else {
            candidate.observe_cursor(point).ok()?;
            candidate.point(point).ok().flatten()
        }
    }
}

pub fn from_arguments(
    center: Point3,
    args: &[&str],
    tolerance: Tolerance,
) -> Result<CircularArc3, CommandError> {
    let mut input = Input::new(center, tolerance);
    let mut used = 0;
    while used < args.len() {
        let word = args[used].trim_start_matches('_');
        let (count, result) = if input.option(word) {
            (1, None)
        } else if let Some((key, value)) = word.split_once('=') {
            if key.eq_ignore_ascii_case("Height") && input.requests_orientation() {
                (1, input.number(crate::parse_finite_real(value)?)?)
            } else if key.eq_ignore_ascii_case("Length")
                && input.frame().is_some()
                && !input.requests_length()
            {
                input.option("Length");
                (1, input.number(crate::parse_finite_real(value)?)?)
            } else {
                return Err(CommandError::Usage(
                    "Arc Center center Tilted start orientation endpoint|angle|Length=distance",
                ));
            }
        } else if (input.frame().is_some() && args.len() - used == 1 && !word.contains(','))
            || (input.requests_orientation()
                && args.len() - used > 1
                && args[used + 1].contains(',')
                && word.parse::<Real>().is_ok())
        {
            (1, input.number(crate::parse_finite_real(word)?)?)
        } else {
            let (point, count) = crate::parse_point(&args[used..])?;
            (count, input.point(point)?)
        };
        used += count;
        if let Some(arc) = result {
            crate::require_consumed(
                args,
                used,
                "Arc Center center Tilted start orientation endpoint|angle|Length=distance",
            )?;
            return Ok(arc);
        }
    }
    Err(CommandError::Usage("Tilted Arc construction is incomplete"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    #[test]
    fn tilted_arc_cursor_crosses_the_start_ray_and_keeps_far_side_jumps_stable() {
        let mut i = Input::new(p(0., 0., 0.), Tolerance::DEFAULT);
        i.point(p(4., 0., 0.)).unwrap();
        i.point(p(0., 3., 0.)).unwrap();
        i.observe_cursor(p(-10., -3., 0.)).unwrap();
        assert!(i.clockwise());
        for q in [
            p(6., 0.01, 0.),
            p(-3., 3., 0.),
            p(-3., -3., 0.),
            p(3., -3., 0.),
        ] {
            i.observe_cursor(q).unwrap();
            assert!(i.clockwise());
        }
        i.observe_cursor(p(3., 3., 0.)).unwrap();
        assert!(!i.clockwise());
        let before = i;
        assert!(i.observe_cursor(p(0., 0., 0.)).is_err());
        assert_eq!(i, before);
        let preview = i.preview(p(0., 6., 0.)).unwrap();
        let arc = i.point(p(0., 6., 0.)).unwrap().unwrap();
        assert_eq!(arc, preview);
        assert!((arc.sweep_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }
    #[test]
    fn tilted_arc_orientation_projects_axial_components_and_preserves_spatial_radius() {
        let mut i = Input::new(p(2., -1., 5.), Tolerance::DEFAULT);
        i.point(p(6., -1., 5.)).unwrap();
        i.point(p(13., 2., 8.)).unwrap();
        let a = i.number(90.).unwrap().unwrap();
        assert!((a.radius() - 4.).abs() < 1e-12);
        assert!(
            a.end()
                .unwrap()
                .distance_to(p(2., -1. + 8f64.sqrt(), 5. + 8f64.sqrt()))
                .unwrap()
                < 1e-12
        );
        assert!((a.sweep_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let b = i.number(-90.).unwrap().unwrap();
        assert!(
            b.end()
                .unwrap()
                .distance_to(p(2., -1. - 8f64.sqrt(), 5. - 8f64.sqrt()))
                .unwrap()
                < 1e-12
        );
    }
    #[test]
    fn tilted_arc_failed_getters_are_atomic_and_distance_points_measure_in_world_space() {
        let mut i = Input::new(p(0., 0., 0.), Tolerance::DEFAULT);
        let seed = i;
        assert!(i.point(p(0., 0., 0.)).is_err());
        assert_eq!(i, seed);
        i.point(p(4., 0., 0.)).unwrap();
        let seed = i;
        assert!(i.point(p(7., 0., 0.)).is_err());
        assert_eq!(i, seed);
        assert!(i.number(Real::NAN).is_err());
        assert_eq!(i, seed);
        let guide = i.orientation_guide(p(13., 2., 2.)).unwrap();
        assert!(guide[3].distance_to(p(0., 2., 2.)).unwrap() < 1e-12);
        i.point(p(0., 3., 3.)).unwrap();
        i.option("Length");
        i.point(p(7., 8., 9.)).unwrap();
        let seed = i;
        assert!(i.point(p(7., 8., 9.)).is_err());
        assert_eq!(i, seed);
        let arc = i.point(p(10., 12., 9.)).unwrap().unwrap();
        assert!((arc.radius() * arc.sweep_radians() - 5.).abs() < 1e-12);
    }
}
