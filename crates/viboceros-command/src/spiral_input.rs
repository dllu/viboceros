//! Edit-free axial Helix/Spiral getters shared by the GUI and oracle.
use crate::{
    AxialPrimitiveRadius, CommandError, axial_primitive_frame, axial_primitive_radius_value,
};
use viboceros_geometry::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Helix,
    Spiral,
}
impl Kind {
    pub fn parse(word: &str) -> Option<Self> {
        match word
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "helix" => Some(Self::Helix),
            "spiral" => Some(Self::Spiral),
            _ => None,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Helix => "Helix",
            Self::Spiral => "Spiral",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Turns,
    Pitch,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    Free,
    Vertical,
    Flat,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub turns: Real,
    pub pitch: Real,
    pub reverse_twist: bool,
    pub diameter: bool,
    pub start_radius: Real,
    pub end_radius: Real,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Turns,
            turns: 10.,
            pitch: 1.,
            reverse_twist: false,
            diameter: false,
            start_radius: 1.,
            end_radius: 0.,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Start,
    End,
    StartRadius,
    EndRadius,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Numeric {
    Turns,
    Pitch,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    kind: Kind,
    options: Options,
    axis: Axis,
    plane: Frame3,
    tolerance: Tolerance,
    start: Option<Point3>,
    end: Option<Point3>,
    radial: Option<Point3>,
    end_radius: Option<Real>,
    locked_radius: Option<Real>,
    locked_height: Option<Real>,
    complete: bool,
}
fn invalid(context: &'static str) -> CommandError {
    GeometryError::Degenerate { context }.into()
}
impl Input {
    pub fn new(kind: Kind, options: Options, plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            kind,
            options,
            axis: Axis::Free,
            plane,
            tolerance,
            start: None,
            end: None,
            radial: None,
            end_radius: None,
            locked_radius: None,
            locked_height: None,
            complete: false,
        }
    }
    pub fn kind(self) -> Kind {
        self.kind
    }
    pub fn options(self) -> Options {
        self.options
    }
    pub fn axis(self) -> Axis {
        self.axis
    }
    pub fn plane(self) -> Frame3 {
        self.plane
    }
    pub fn slot(self) -> Slot {
        if self.complete {
            Slot::Complete
        } else if self.start.is_none() {
            Slot::Start
        } else if self.end.is_none() && self.axis != Axis::Flat {
            Slot::End
        } else if self.radial.is_none() {
            Slot::StartRadius
        } else {
            Slot::EndRadius
        }
    }
    pub fn anchor(self) -> Option<Point3> {
        if self.slot() == Slot::EndRadius {
            self.end.or(self.start)
        } else {
            self.start
        }
    }
    pub fn markers(self) -> Vec<Point3> {
        [self.start, self.end, self.radial]
            .into_iter()
            .flatten()
            .collect()
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.start.is_none() {
            self.plane = plane;
        }
    }
    pub fn mouse_plane(self) -> Frame3 {
        match self.slot() {
            Slot::StartRadius | Slot::EndRadius => {
                if let (Some(start), Some(end)) = (self.start, self.end.or(self.start)) {
                    let normal = if self.axis == Axis::Flat {
                        self.plane.z_axis().as_vector()
                    } else {
                        match start.vector_to(end) {
                            Ok(v) => v,
                            Err(_) => return self.plane,
                        }
                    };
                    return Frame3::try_from_normal(
                        self.anchor().unwrap_or(start),
                        normal,
                        self.tolerance,
                    )
                    .unwrap_or(self.plane);
                }
                self.plane
            }
            _ => self.plane,
        }
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        if self.complete {
            return Err(invalid("spiral input is complete"));
        }
        let word = word.trim_start_matches('_');
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v.trim_start_matches('_'))));
        let mut next = *self;
        match key.to_ascii_lowercase().as_str() {
            "vertical" | "flat" if value.is_none() => {
                if self.start.is_some()
                    || key.eq_ignore_ascii_case("Flat") && self.kind != Kind::Spiral
                {
                    return Err(invalid("axis option is unavailable after the first point"));
                }
                next.axis = if key.eq_ignore_ascii_case("Flat") {
                    next.options.mode = Mode::Turns;
                    Axis::Flat
                } else {
                    Axis::Vertical
                };
            }
            "mode"
                if self.axis == Axis::Flat
                    && (value.is_none()
                        || value.is_some_and(|v| v.eq_ignore_ascii_case("Pitch"))) =>
            {
                return Err(invalid("a flat spiral has no pitch"));
            }
            "mode" => {
                next.options.mode = match value.map(str::to_ascii_lowercase).as_deref() {
                    Some("turns") => Mode::Turns,
                    Some("pitch") => Mode::Pitch,
                    None => {
                        if next.options.mode == Mode::Turns {
                            Mode::Pitch
                        } else {
                            Mode::Turns
                        }
                    }
                    _ => return Err(invalid("mode requires Turns or Pitch")),
                }
            }
            "turns" | "pitch" => {
                let value = value
                    .and_then(|v| v.parse::<Real>().ok())
                    .ok_or_else(|| invalid("spiral option requires a positive number"))?;
                next.numeric(
                    if key.eq_ignore_ascii_case("Turns") {
                        Numeric::Turns
                    } else {
                        Numeric::Pitch
                    },
                    value,
                )?;
            }
            "reversetwist" => {
                next.options.reverse_twist = if let Some(v) = value {
                    crate::parse_yes_no(v)
                        .ok_or_else(|| invalid("ReverseTwist requires Yes or No"))?
                } else {
                    !next.options.reverse_twist
                }
            }
            "diameter" | "radius" if value.is_none() => {
                next.options.diameter = key.eq_ignore_ascii_case("Diameter")
            }
            _ => return Ok(false),
        }
        *self = next;
        Ok(true)
    }
    pub fn numeric(&mut self, key: Numeric, value: Real) -> Result<(), CommandError> {
        if self.complete {
            return Err(invalid("spiral input is complete"));
        }
        if self.axis == Axis::Flat && key == Numeric::Pitch {
            return Err(invalid("a flat spiral has no pitch"));
        }
        if !value.is_finite() || value <= 0. {
            return Err(invalid("spiral advance must be positive and finite"));
        }
        let mut next = *self;
        match key {
            Numeric::Turns => {
                next.options.turns = value;
                next.options.mode = Mode::Turns;
            }
            Numeric::Pitch => {
                next.options.pitch = value;
                next.options.mode = Mode::Pitch;
            }
        }
        next.update_advance()?;
        *self = next;
        Ok(())
    }
    fn update_advance(&mut self) -> Result<(), CommandError> {
        if self.axis == Axis::Flat {
            return Ok(());
        }
        if let (Some(a), Some(b)) = (self.start, self.end) {
            let height = a.distance_to(b)?;
            match self.options.mode {
                Mode::Turns => self.options.pitch = height / self.options.turns,
                Mode::Pitch => self.options.turns = height / self.options.pitch,
            }
            if !self.options.turns.is_finite()
                || self.options.turns <= 0.
                || !self.options.pitch.is_finite()
                || self.options.pitch <= 0.
            {
                return Err(invalid("spiral advance is out of range"));
            }
        }
        Ok(())
    }
    pub fn number(&mut self, value: Real) -> Result<bool, CommandError> {
        if !value.is_finite() {
            return Err(invalid("spiral input must be finite"));
        }
        let mut next = *self;
        match next.slot() {
            Slot::End if next.axis == Axis::Vertical => {
                if value == 0. {
                    return Err(invalid("axis height must be nonzero"));
                }
                next.locked_height = Some(value.abs());
            }
            Slot::StartRadius => {
                if value <= 0. {
                    return Err(invalid("radius must be positive"));
                }
                next.locked_radius = Some(value * if next.options.diameter { 0.5 } else { 1. });
            }
            Slot::EndRadius => {
                if value < 0. {
                    return Err(invalid("end radius must be nonnegative"));
                }
                next.end_radius = Some(value * if next.options.diameter { 0.5 } else { 1. });
                next.complete = true;
                next.curve()?;
            }
            _ => return Err(invalid("pick a spiral point")),
        }
        if next.complete {
            next.options.end_radius = next.end_radius.unwrap();
        }
        *self = next;
        Ok(self.complete)
    }
    pub fn point(&mut self, point: Point3) -> Result<bool, CommandError> {
        let mut next = *self;
        match self.slot() {
            Slot::Start => next.start = Some(point),
            Slot::End => {
                let start = next.start.unwrap();
                let end = if next.axis == Axis::Vertical {
                    let height = start
                        .vector_to(point)?
                        .dot(next.plane.z_axis().as_vector())?;
                    if height.abs() <= next.tolerance.absolute() {
                        return Err(invalid("pick above or below the axis start"));
                    }
                    let height = next.locked_height.map_or(height, |h| h * height.signum());
                    start.translated(next.plane.z_axis().as_vector().scaled(height)?)?
                } else {
                    point
                };
                if start.distance_to(end)? <= next.tolerance.absolute() {
                    return Err(invalid("spiral axis is too short"));
                }
                next.end = Some(end);
                next.update_advance()?;
            }
            Slot::StartRadius => {
                let (frame, radius) = next.radial_frame(point)?;
                let radius = next
                    .locked_radius
                    .unwrap_or(radius * if next.options.diameter { 0.5 } else { 1. });
                if radius <= next.tolerance.absolute() {
                    return Err(invalid("radius is too small"));
                }
                next.options.start_radius = radius;
                next.radial = Some(
                    next.start
                        .unwrap()
                        .translated(frame.x_axis().as_vector().scaled(radius)?)?,
                );
                if next.kind == Kind::Helix {
                    next.end_radius = Some(radius);
                    next.complete = true;
                    next.curve()?;
                }
            }
            Slot::EndRadius => {
                let (frame, _) = next.radial_frame(next.radial.unwrap())?;
                let radius = axial_primitive_radius_value(
                    next.end.or(next.start).unwrap(),
                    AxialPrimitiveRadius::Point(point),
                    frame,
                )?;
                next.end_radius = Some(radius * if next.options.diameter { 0.5 } else { 1. });
                next.complete = true;
                next.curve()?;
            }
            Slot::Complete => return Err(invalid("spiral input is complete")),
        }
        if next.complete {
            next.options.end_radius = next.end_radius.unwrap();
        }
        *self = next;
        Ok(self.complete)
    }
    /// Accept the remembered radius in the current hovered direction.
    pub fn default_radius(&mut self, cursor: Point3) -> Result<bool, CommandError> {
        if self.slot() != Slot::StartRadius {
            return Err(invalid("radius getter required"));
        }
        let mut next = *self;
        next.locked_radius = Some(next.locked_radius.unwrap_or(next.options.start_radius));
        let complete = next.point(cursor)?;
        *self = next;
        Ok(complete)
    }
    pub fn enter(&mut self) -> Result<bool, CommandError> {
        if self.slot() == Slot::EndRadius {
            self.number(self.options.end_radius * if self.options.diameter { 2. } else { 1. })
        } else {
            Err(invalid("pick a point to set the spiral direction"))
        }
    }
    fn radial_frame(self, point: Point3) -> Result<(Frame3, Real), CommandError> {
        let start = self
            .start
            .ok_or_else(|| invalid("axis start is required"))?;
        let normal = if self.axis == Axis::Flat {
            self.plane.z_axis().as_vector()
        } else {
            start.vector_to(self.end.ok_or_else(|| invalid("axis end is required"))?)?
        };
        axial_primitive_frame(
            start,
            AxialPrimitiveRadius::Point(point),
            normal,
            self.tolerance,
        )
    }
    pub fn curve(self) -> Result<NurbsCurve, CommandError> {
        let (frame, radius) = self.radial_frame(
            self.radial
                .ok_or_else(|| invalid("radius point is required"))?,
        )?;
        let height = if self.axis == Axis::Flat {
            0.
        } else {
            self.start.unwrap().distance_to(self.end.unwrap())?
        };
        let turns = self.options.turns * if self.options.reverse_twist { -1. } else { 1. };
        Ok(NurbsCurve::try_spiral(
            frame,
            height,
            turns,
            [
                radius,
                self.end_radius
                    .ok_or_else(|| invalid("end radius is required"))?,
            ],
        )?)
    }
    pub fn radius_value(self) -> Real {
        self.locked_radius.unwrap_or(self.options.start_radius)
    }
    pub fn preview(self, cursor: Point3) -> Option<NurbsCurve> {
        let mut next = self;
        let complete = next.point(cursor).ok()?;
        if !complete {
            if self.slot() != Slot::StartRadius || self.kind != Kind::Spiral {
                return None;
            }
            next.enter().ok()?;
        }
        next.curve().ok()
    }
    pub fn command(self) -> Result<String, CommandError> {
        if !self.complete {
            return Err(invalid("spiral input is incomplete"));
        }
        fn p(p: Point3) -> String {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        }
        let mut words = vec![self.kind.name().to_owned()];
        if self.axis == Axis::Flat {
            words.push("Flat".into());
        }
        words.push(p(self.start.unwrap()));
        if self.axis != Axis::Flat {
            words.push(p(self.end.unwrap()));
        }
        words.push(p(self.radial.unwrap()));
        if self.kind == Kind::Spiral {
            words.push(self.end_radius.unwrap().to_string());
        }
        if self.axis == Axis::Flat {
            let [x, y, z] = self.plane.z_axis().as_vector().to_array();
            words.push(format!("Axis={x},{y},{z}"));
        }
        match self.options.mode {
            Mode::Turns => words.push(format!("Mode=Turns Turns={}", self.options.turns)),
            Mode::Pitch if self.axis != Axis::Flat => {
                words.push(format!("Mode=Pitch Pitch={}", self.options.pitch))
            }
            Mode::Pitch => words.push(format!("Turns={}", self.options.turns)),
        }
        words.push(format!(
            "ReverseTwist={}",
            if self.options.reverse_twist {
                "Yes"
            } else {
                "No"
            }
        ));
        Ok(words.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandContext, CommandRegistry};
    use viboceros_document::{Document, Geometry};
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn input(kind: Kind) -> Input {
        Input::new(
            kind,
            Options::default(),
            CommandContext::default().construction_plane,
            Tolerance::DEFAULT,
        )
    }
    #[test]
    fn spiral_input_radius_locks_preview_and_script_share_one_curve() {
        let mut i = input(Kind::Spiral);
        i.point(p(0., 0., 0.)).unwrap();
        i.point(p(0., 0., 6.)).unwrap();
        i.option("Turns=2").unwrap();
        i.number(2.).unwrap();
        let before = i;
        let preview = i.preview(p(0., 7., 0.)).unwrap();
        assert_eq!(before, i);
        assert_eq!(preview.control_points()[0].point(), p(0., 2., 0.));
        assert!(!i.point(p(0., 7., 0.)).unwrap());
        let preview = i.preview(p(4., 0., 6.)).unwrap();
        assert!(!i.complete);
        assert!(i.number(4.).unwrap());
        assert_eq!(preview, i.curve().unwrap());
        let commands = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        commands.execute(&mut doc, &i.command().unwrap()).unwrap();
        let Geometry::NurbsCurve(c) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(c, &preview);
        assert_eq!(doc.undo_label(), Some("Spiral"));
        assert_eq!(doc.objects().len(), 1);
        doc.undo().unwrap();
        assert_eq!(doc.objects().len(), 0);
    }
    #[test]
    fn spiral_input_invalid_picks_and_numbers_do_not_advance_or_mutate() {
        let mut i = input(Kind::Helix);
        assert!(i.option("Flat").is_err());
        i.point(p(1., 2., 3.)).unwrap();
        let before = i;
        assert!(i.option("Vertical").is_err());
        assert_eq!(i, before);
        assert!(i.point(p(1., 2., 3.)).is_err());
        assert_eq!(i, before);
        i.point(p(1., 2., 9.)).unwrap();
        let before = i;
        for v in [0., -1., Real::NAN, Real::INFINITY] {
            assert!(i.number(v).is_err());
            assert_eq!(i, before);
            assert!(i.numeric(Numeric::Turns, v).is_err());
            assert_eq!(i, before);
        }
        assert!(i.point(p(1., 2., 5.)).is_err());
        assert_eq!(i, before);
        assert!(i.numeric(Numeric::Pitch, Real::MIN_POSITIVE).is_err());
        assert_eq!(i, before);
        i.point(p(3., 2., 3.)).unwrap();
        assert!(i.complete);
        let complete = i;
        assert!(i.point(p(0., 0., 0.)).is_err());
        assert!(i.option("ReverseTwist").is_err());
        assert_eq!(complete, i);
    }
    #[test]
    fn spiral_input_vertical_height_flat_plane_diameter_and_defaults() {
        let mut i = input(Kind::Helix);
        i.option("Vertical").unwrap();
        i.point(p(1., 2., 3.)).unwrap();
        i.number(8.).unwrap();
        assert_eq!(i.slot(), Slot::End);
        i.point(p(9., 5., 2.)).unwrap();
        assert_eq!(i.end, Some(p(1., 2., -5.)));
        i.option("Diameter").unwrap();
        i.number(6.).unwrap();
        i.point(p(1., 9., 3.)).unwrap();
        assert_eq!(
            i.curve().unwrap().control_points()[0].point(),
            p(1., 5., 3.)
        );
        let plane = Frame3::try_from_directions(
            p(4., 5., 6.),
            viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
            viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut flat = Input::new(Kind::Spiral, Options::default(), plane, Tolerance::DEFAULT);
        flat.option("Flat").unwrap();
        flat.point(p(1., 2., 3.)).unwrap();
        assert!(flat.option("Mode=Pitch").is_err());
        flat.default_radius(p(6., 2., 3.)).unwrap();
        assert_eq!(flat.slot(), Slot::EndRadius);
        flat.enter().unwrap();
        let curve = flat.curve().unwrap();
        assert!(
            curve
                .control_points()
                .iter()
                .all(|p| (p.point().y() - 2.).abs() < 1e-12)
        );
        assert_eq!(
            curve.control_points().first().unwrap().point(),
            p(2., 2., 3.)
        );
        assert_eq!(
            curve.control_points().last().unwrap().point(),
            p(1., 2., 3.)
        );
    }
    #[test]
    fn spiral_input_preferences_are_registry_local_and_outside_history() {
        let commands = CommandRegistry::with_builtins();
        let other = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let mut i = input(Kind::Helix);
        i.point(p(0., 0., 0.)).unwrap();
        i.point(p(0., 0., 10.)).unwrap();
        i.option("Turns=2.5").unwrap();
        i.option("ReverseTwist").unwrap();
        i.point(p(2., 0., 0.)).unwrap();
        commands.execute(&mut doc, &i.command().unwrap()).unwrap();
        commands.remember_spiral_options(Kind::Helix, i.options());
        let options = commands.spiral_options(Kind::Helix);
        doc.undo().unwrap();
        assert_eq!(commands.spiral_options(Kind::Helix), options);
        assert_ne!(commands.spiral_options(Kind::Spiral), options);
        assert_eq!(other.spiral_options(Kind::Helix), Options::default());
    }
}
