//! Edit-free Circle output settings layered over the shared construction getter.
use crate::{
    CommandError,
    circle_around_input::Preferences,
    circle_input::{CircleDefinition, CircleInput, CircleSizeMode},
};
use viboceros_document::Geometry;
use viboceros_geometry::{Circle3, Frame3, GeometryError, NurbsCurve, Point3, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deformation {
    pub degree: usize,
    pub point_count: usize,
}
impl Default for Deformation {
    fn default() -> Self {
        Self {
            degree: 5,
            point_count: 10,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub deformable: bool,
    pub deformation: Deformation,
}
impl Output {
    pub fn geometry(self, circle: Circle3) -> Result<Geometry, GeometryError> {
        if self.deformable {
            Ok(Geometry::NurbsCurve(circle.try_deformable(
                self.deformation.degree,
                self.deformation.point_count,
            )?))
        } else {
            Ok(Geometry::Circle(circle))
        }
    }
    pub fn curve(self, circle: Circle3) -> Result<NurbsCurve, GeometryError> {
        if self.deformable {
            circle.try_deformable(self.deformation.degree, self.deformation.point_count)
        } else {
            circle.to_nurbs()
        }
    }
    pub fn command_prefix(self) -> String {
        if !self.deformable {
            return "Circle".to_owned();
        }
        let d = self.deformation;
        if d.point_count < d.degree + 2 {
            // Native accepts these counts when Degree is raised after a lower
            // degree's PointCount getter. Preserve that legal option order.
            return format!(
                "Circle Deformable Degree=2 PointCount={} Degree={}",
                d.point_count, d.degree
            );
        }
        format!(
            "Circle Deformable Degree={} PointCount={}",
            d.degree, d.point_count
        )
    }
    pub fn wrap_command(self, circle_command: &str) -> String {
        format!(
            "{} {}",
            self.command_prefix(),
            circle_command
                .strip_prefix("Circle ")
                .expect("Circle construction")
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Degree,
    PointCount,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    getter: CircleInput,
    output: Output,
    pending: Option<Pending>,
    preferences: Preferences,
    tolerance: Tolerance,
}
impl Input {
    pub fn new(plane: Frame3, preferences: Preferences, tolerance: Tolerance) -> Self {
        Self {
            getter: CircleInput::new(plane).with_size_mode(preferences.mode()),
            output: Output {
                deformable: false,
                deformation: preferences.deformation,
            },
            pending: None,
            preferences,
            tolerance,
        }
    }
    pub fn output(self) -> Output {
        self.output
    }
    pub fn getter(self) -> CircleInput {
        self.getter
    }
    pub fn at_start(self) -> bool {
        self.getter.at_start() && self.pending.is_none()
    }
    pub fn integer_pending(self) -> bool {
        self.pending.is_some()
    }
    pub fn accepts_default(self) -> bool {
        self.integer_pending()
            || (self.getter.requests_size()
                && matches!(
                    self.getter.size_mode(),
                    CircleSizeMode::Radius | CircleSizeMode::Diameter
                ))
    }
    pub fn prompt(self) -> String {
        match self.pending {
            Some(Pending::Degree) => {
                format!("Deformable degree <{}>", self.output.deformation.degree)
            }
            Some(Pending::PointCount) => format!(
                "Deformable point count <{}>",
                self.output.deformation.point_count
            ),
            None if self.at_start() => format!(
                "{}; Degree={}, PointCount={}, AroundCurve, FitPoints",
                self.getter.prompt(),
                self.output.deformation.degree,
                self.output.deformation.point_count
            ),
            None => self.getter.prompt().to_owned(),
        }
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim().trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if self.pending.is_none() && self.getter.anchor().is_none() {
            if name.eq_ignore_ascii_case("Deformable") && !self.output.deformable && value.is_none()
            {
                self.output.deformable = true;
                return Ok(true);
            }
            let pending = if name.eq_ignore_ascii_case("Degree") {
                Some(Pending::Degree)
            } else if name.eq_ignore_ascii_case("PointCount") {
                Some(Pending::PointCount)
            } else {
                None
            };
            if self.output.deformable
                && let Some(pending) = pending
            {
                if let Some(value) = value {
                    let mut candidate = *self;
                    candidate.pending = Some(pending);
                    candidate.number(crate::parse_finite_real(value)?)?;
                    *self = candidate;
                } else {
                    self.pending = Some(pending);
                }
                return Ok(true);
            }
        }
        if self.pending.is_some() {
            return Ok(false);
        }
        if value.is_none() && self.getter.option(name) {
            if matches!(
                CircleSizeMode::parse(name),
                Some(CircleSizeMode::Radius | CircleSizeMode::Diameter)
            ) && self.getter.has_size_mode()
            {
                self.preferences.diameter = self.getter.size_mode() == CircleSizeMode::Diameter;
            }
            return Ok(true);
        }
        Ok(false)
    }
    fn circle(
        self,
        definition: Option<CircleDefinition>,
    ) -> Result<Option<Circle3>, GeometryError> {
        definition
            .map(|d| {
                Circle3::try_from_frame(
                    d.frame.origin(),
                    d.radius,
                    d.frame.x_axis(),
                    d.frame.z_axis(),
                    self.tolerance,
                )
            })
            .transpose()
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<Circle3>, CommandError> {
        if self.integer_pending() {
            return Err(CommandError::Usage("Enter an integer setting"));
        }
        let mut candidate = *self;
        let definition = candidate.getter.point(point)?;
        let circle = candidate.circle(definition)?;
        if let Some(circle) = circle {
            candidate.output.geometry(circle)?;
        }
        *self = candidate;
        Ok(circle)
    }
    pub fn number(&mut self, value: f64) -> Result<Option<Circle3>, CommandError> {
        if let Some(pending) = self.pending {
            let d = self.output.deformation;
            let (minimum, maximum) = match pending {
                Pending::Degree => (2, 11),
                Pending::PointCount => (d.degree + 2, 1000),
            };
            if !value.is_finite()
                || value.fract() != 0.
                || value < minimum as f64
                || value > maximum as f64
            {
                return Err(CommandError::Usage(
                    "Circle Degree requires 2..11; PointCount requires degree+2..1000",
                ));
            }
            let number = value as usize;
            match pending {
                Pending::Degree => self.output.deformation.degree = number,
                Pending::PointCount => self.output.deformation.point_count = number,
            }
            self.pending = None;
            return Ok(None);
        }
        let mut candidate = *self;
        let d = candidate.getter.number(value)?;
        let circle = candidate.circle(d)?;
        if let Some(circle) = circle {
            candidate.output.geometry(circle)?;
        }
        *self = candidate;
        Ok(circle)
    }
    pub fn default_value(&mut self) -> Result<Option<Circle3>, CommandError> {
        if self.pending.take().is_some() {
            return Ok(None);
        }
        if !self.accepts_default() {
            return Err(CommandError::Usage("Circle getter has no default"));
        }
        self.number(if self.getter.size_mode() == CircleSizeMode::Diameter {
            2. * self.preferences.radius
        } else {
            self.preferences.radius
        })
    }
    pub fn preview(self, point: Point3) -> Option<NurbsCurve> {
        let mut candidate = self;
        candidate
            .point(point)
            .ok()
            .flatten()
            .and_then(|c| self.output.curve(c).ok())
    }
    pub fn preferences(self, circle: Circle3) -> Preferences {
        Preferences {
            radius: if self.getter.remembers_radius() {
                circle.radius()
            } else {
                self.preferences.radius
            },
            deformation: self.output.deformation,
            ..self.preferences
        }
    }
}

pub enum Construction {
    Circle {
        circle: Circle3,
        preferences: Preferences,
    },
    Around(crate::circle_around_input::Definition),
    FitPoints,
    Tangent(crate::circle_tangent_input::Definition),
}
pub fn is_deformable_start(args: &[&str]) -> bool {
    args.first().is_some_and(|w| {
        w.trim_start_matches(['_', '-'])
            .split('=')
            .next()
            .is_some_and(|k| k.eq_ignore_ascii_case("Deformable"))
    })
}
pub fn output_prefix(
    args: &[&str],
    preferences: Preferences,
) -> Result<Option<(Output, usize)>, CommandError> {
    if !is_deformable_start(args) {
        return Ok(None);
    }
    let mut input = Input::new(
        crate::CommandContext::default().construction_plane,
        preferences,
        Tolerance::NUMERICAL_VALIDATION,
    );
    let mut i = 0;
    while i < args.len() {
        let key = args[i]
            .trim_start_matches(['_', '-'])
            .split('=')
            .next()
            .unwrap_or("");
        if ["Deformable", "Degree", "PointCount"]
            .iter()
            .any(|k| key.eq_ignore_ascii_case(k))
            && input.option(args[i])?
        {
            i += 1;
            if input.integer_pending() {
                let value = args
                    .get(i)
                    .ok_or(CommandError::Usage("Circle setting requires an integer"))?;
                input.number(crate::parse_finite_real(value)?)?;
                i += 1;
            }
        } else {
            break;
        }
    }
    Ok(Some((input.output, i)))
}
pub fn from_arguments(
    doc: &viboceros_document::Document,
    args: &[&str],
    plane: Frame3,
    preferences: Preferences,
) -> Result<(Output, Construction), CommandError> {
    let mut input = Input::new(plane, preferences, doc.tolerance());
    let mut i = 0;
    let mut completed = None;
    while i < args.len() {
        if completed.is_some() {
            return Err(CommandError::Usage("Trailing Circle input"));
        }
        let word = args[i].trim_start_matches(['_', '-']);
        if input.option(word)? {
            i += 1;
            continue;
        }
        if input.at_start() && word.eq_ignore_ascii_case("AroundCurve") {
            let prefs = Preferences {
                deformation: input.output.deformation,
                ..preferences
            };
            return Ok((
                input.output,
                Construction::Around(crate::circle_around_input::from_arguments(
                    doc,
                    &args[i + 1..],
                    prefs,
                )?),
            ));
        }
        if input.at_start() && word.eq_ignore_ascii_case("Tangent") {
            let prefs = Preferences {
                deformation: input.output.deformation,
                ..preferences
            };
            return Ok((
                input.output,
                Construction::Tangent(crate::circle_tangent_input::from_arguments(
                    doc,
                    &args[i + 1..],
                    plane,
                    prefs,
                )?),
            ));
        }
        if input.at_start() && word.eq_ignore_ascii_case("FitPoints") && i + 1 == args.len() {
            return Ok((input.output, Construction::FitPoints));
        }
        if word.eq_ignore_ascii_case("Enter") {
            completed = input.default_value()?;
            i += 1;
            continue;
        }
        if let Some((name, value)) = word.split_once('=') {
            if !input.option(name)? {
                return Err(CommandError::Usage("Circle size option"));
            }
            completed = input.number(crate::parse_finite_real(value)?)?;
            i += 1;
        } else if word.contains(',')
            || (!input.integer_pending() && !input.getter.requests_size() && args.len() - i >= 3)
        {
            let (point, count) = crate::parse_point(&args[i..])?;
            completed = input.point(point)?;
            i += count;
        } else {
            completed = input.number(crate::parse_finite_real(word)?)?;
            i += 1;
        }
    }
    let circle = completed.ok_or(CommandError::Usage("Incomplete Circle construction"))?;
    Ok((
        input.output,
        Construction::Circle {
            circle,
            preferences: input.preferences(circle),
        },
    ))
}
