//! Base-circle, direction and signed-height getters.
use super::*;
use crate::{
    circle_input::{CircleDefinition, CircleInput, CircleSizeMode},
    translation::DestinationConstraint,
};
use viboceros_document::Document;
use viboceros_geometry::{Circle3, GeometryError, Point3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub kind: Kind,
    pub preferences: Preferences,
    pub circle: Option<CircleDefinition>,
    pub definition: Option<Definition>,
    base: CircleInput,
    guide_selected: bool,
    direction_menu: bool,
    cursor: Option<Point3>,
}

impl Input {
    pub fn new(kind: Kind, plane: Frame3, preferences: Preferences) -> Self {
        let mode = if preferences.diameter {
            CircleSizeMode::Diameter
        } else {
            CircleSizeMode::Radius
        };
        Self {
            kind,
            preferences,
            circle: None,
            definition: None,
            base: CircleInput::new(plane).with_size_mode(mode),
            guide_selected: false,
            direction_menu: false,
            cursor: None,
        }
    }

    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.circle.is_none() {
            self.base.set_initial_plane(plane);
        }
    }

    pub fn anchor(self) -> Option<Point3> {
        self.circle
            .map(|base| base.frame.origin())
            .or(self.base.anchor())
    }

    pub fn at_start(self) -> bool {
        self.circle.is_none() && self.base.at_start() && !self.direction_menu
    }

    pub fn needs_guide(self) -> bool {
        self.at_start()
            && self.preferences.direction == Direction::AroundCurve
            && !self.guide_selected
    }

    pub fn select_guide(&mut self) {
        self.guide_selected = true;
    }

    pub fn wants_number(self) -> bool {
        !self.direction_menu && (self.circle.is_some() || self.base.requests_size())
    }

    pub fn scalar_units(self) -> bool {
        self.circle.is_none() && self.base.size_mode() == CircleSizeMode::Area
    }

    pub fn snap_projection_plane(self) -> Option<Frame3> {
        if self.circle.is_none() && !self.direction_menu {
            self.base.snap_projection_plane()
        } else {
            None
        }
    }

    pub fn height_constraint(self) -> Option<DestinationConstraint> {
        let circle = self.circle?;
        if self.direction_menu || self.preferences.direction == Direction::None {
            return None;
        }
        Some(DestinationConstraint {
            anchor: circle.frame.origin(),
            direction: Some(circle.frame.z_axis()),
            distance: None,
        })
    }

    pub fn prompt(self) -> String {
        let name = self.kind.name().to_ascii_lowercase();
        let direction = self.preferences.direction.label();
        if self.direction_menu {
            let choices = if self.circle.is_some() {
                "None Vertical"
            } else {
                "None Vertical AroundCurve"
            };
            return format!("Direction constraint <{direction}> ({choices})");
        }
        if self.circle.is_some() {
            let mut prompt = format!("End of {name} <{}>", self.preferences.height);
            if self.preferences.direction != Direction::AroundCurve {
                prompt.push_str(&format!("; DirectionConstraint={direction}"));
            }
            if self.kind == Kind::Cylinder {
                prompt.push_str(if self.preferences.both_sides {
                    "; BothSides=Yes"
                } else {
                    "; BothSides=No"
                });
            }
            return prompt;
        }
        if self.base.at_start() {
            let solid = if self.preferences.solid { "Yes" } else { "No" };
            return format!(
                "Base of {name}; DirectionConstraint={direction}; Solid={solid}; 2Point, 3Point, Tangent, FitPoints",
            );
        }
        let mut prompt = self
            .base
            .prompt()
            .replace("Orientation, ", "")
            .replace("; Vertical", "");
        if self.base.is_two_point() && self.preferences.direction != Direction::AroundCurve {
            prompt.push_str(&format!("; DirectionConstraint={direction}"));
        }
        if self.base.requests_size() {
            match self.base.size_mode() {
                CircleSizeMode::Radius => {
                    prompt.push_str(&format!(" <{}>", self.preferences.radius))
                }
                CircleSizeMode::Diameter => {
                    prompt.push_str(&format!(" <{}>", 2. * self.preferences.radius))
                }
                _ => (),
            }
        }
        prompt
    }

    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim().trim_start_matches(['_', '-']);
        if self.direction_menu {
            return if option_name_eq(word, "Enter") {
                self.enter().map(|()| true)
            } else {
                self.set_direction(word).map(|()| true)
            };
        }
        let (name, value) = word
            .split_once('=')
            .or_else(|| word.split_once(char::is_whitespace))
            .map_or((word, None), |(k, v)| (k, Some(v.trim())));
        let can_toggle_direction = (self.base.is_two_point() || self.circle.is_some())
            && self.preferences.direction != Direction::AroundCurve;
        if option_name_eq(name, "DirectionConstraint") && (self.at_start() || can_toggle_direction)
        {
            if let Some(direction) = value {
                self.set_direction(direction)?;
            } else if can_toggle_direction {
                self.preferences.direction = if self.preferences.direction == Direction::None {
                    Direction::Vertical
                } else {
                    Direction::None
                };
            } else {
                self.direction_menu = true;
                self.guide_selected = false;
            }
            return Ok(true);
        }
        if option_name_eq(name, "Solid") && self.at_start() {
            self.preferences.solid = toggle(value, self.preferences.solid, self.kind)?;
            return Ok(true);
        }
        if option_name_eq(name, "BothSides") && self.kind == Kind::Cylinder && self.circle.is_some()
        {
            self.preferences.both_sides = toggle(value, self.preferences.both_sides, self.kind)?;
            return Ok(true);
        }
        if self.circle.is_none()
            && !["Vertical", "Orientation"]
                .iter()
                .any(|mode| option_name_eq(name, mode))
            && self.base.option(name)
        {
            if self.base.is_two_point() && self.preferences.direction == Direction::AroundCurve {
                self.preferences.direction = Direction::Vertical;
            }
            if self.base.has_size_mode() {
                match self.base.size_mode() {
                    CircleSizeMode::Radius => self.preferences.diameter = false,
                    CircleSizeMode::Diameter => self.preferences.diameter = true,
                    _ => (),
                }
            }
            if let Some(size) = value {
                self.number(crate::parse_finite_real(size)?)?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn set_direction(&mut self, text: &str) -> Result<(), CommandError> {
        let direction = Direction::parse(text).ok_or(CommandError::Usage(self.kind.usage()))?;
        if direction == Direction::AroundCurve && (self.circle.is_some() || !self.base.at_start()) {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        self.preferences.direction = direction;
        self.guide_selected = false;
        self.direction_menu = false;
        Ok(())
    }

    pub(super) fn around_frame(&mut self, frame: Frame3) {
        let mode = if self.preferences.diameter {
            CircleSizeMode::Diameter
        } else {
            CircleSizeMode::Radius
        };
        self.base = CircleInput::at_center(frame, frame.origin()).with_size_mode(mode);
    }

    fn accept_circle(&mut self, circle: CircleDefinition) {
        if self.base.remembers_radius() {
            self.preferences.radius = circle.radius;
        }
        self.circle = Some(circle);
        self.cursor = None;
    }

    pub fn reference_circle(
        &mut self,
        circle: Circle3,
        remember: bool,
    ) -> Result<(), CommandError> {
        if !self.at_start() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        let definition = CircleDefinition::from_circle(circle)?;
        if remember {
            self.accept_circle(definition);
        } else {
            self.circle = Some(definition);
        }
        Ok(())
    }

    pub fn fit_points(&mut self, doc: &Document) -> Result<(), CommandError> {
        let circle = crate::circle_fit_points::selected_circle(doc)?
            .ok_or(CommandError::OperationDeclined)?;
        self.reference_circle(circle, false)
    }

    pub fn observe_cursor(&mut self, point: Point3) {
        if let Some(circle) = self.circle
            && !self.direction_menu
        {
            self.cursor = (point != circle.frame.origin()).then_some(point);
        }
    }

    pub fn point(&mut self, mut point: Point3) -> Result<(), CommandError> {
        if self.direction_menu || self.needs_guide() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(circle) = self.circle {
            let offset = circle.frame.origin().vector_to(point)?;
            let height = if self.preferences.direction == Direction::None {
                offset.length()?
            } else {
                offset.dot(circle.frame.z_axis().as_vector())?
            };
            self.end(height, Some(point))?;
            self.preferences.height = height.abs();
        } else {
            if self.preferences.direction == Direction::AroundCurve && self.base.has_size_mode() {
                let frame = self.base.construction_plane();
                let [x, y] = frame.projected_coordinates_of(point)?;
                point = frame.point_at([x, y, 0.])?;
            }
            if let Some(circle) = self.base.point(point)? {
                self.accept_circle(circle);
            }
        }
        Ok(())
    }

    pub fn number(&mut self, value: f64) -> Result<(), CommandError> {
        if self.direction_menu || self.needs_guide() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if self.circle.is_some() {
            self.end(value, self.cursor)?;
        } else if let Some(circle) = self.base.number(value)? {
            self.accept_circle(circle);
        }
        Ok(())
    }

    pub fn enter(&mut self) -> Result<(), CommandError> {
        if self.direction_menu {
            self.direction_menu = false;
            return Ok(());
        }
        if self.circle.is_some() {
            return self.number(self.preferences.height);
        }
        if self.base.requests_size()
            && matches!(
                self.base.size_mode(),
                CircleSizeMode::Radius | CircleSizeMode::Diameter
            )
        {
            return self.number(
                self.preferences.radius
                    * if self.base.size_mode() == CircleSizeMode::Diameter {
                        2.
                    } else {
                        1.
                    },
            );
        }
        Err(CommandError::Usage(self.kind.usage()))
    }

    fn end(&mut self, height: f64, point: Option<Point3>) -> Result<(), CommandError> {
        if !height.is_finite() || height.abs() <= Tolerance::NUMERICAL_VALIDATION.absolute() {
            return Err(GeometryError::Degenerate {
                context: "cone or cylinder height",
            }
            .into());
        }
        let circle = self.circle.ok_or(CommandError::Usage(self.kind.usage()))?;
        let mut frame = circle.frame;
        if self.preferences.direction == Direction::None
            && let Some(point) = point
        {
            let normal = frame.origin().vector_to(point)?.normalized_nonzero()?;
            let basis = Frame3::try_from_unit_normal(frame.origin(), normal.opposite())?;
            frame = Frame3::try_from_unit_directions(
                basis.origin(),
                basis.x_axis(),
                basis.y_axis().opposite(),
                Tolerance::NUMERICAL_VALIDATION,
            )?;
        }
        self.preferences.height = height;
        self.definition = Some(Definition {
            kind: self.kind,
            frame,
            radius: circle.radius,
            height,
            solid: self.preferences.solid,
            both_sides: self.kind == Kind::Cylinder && self.preferences.both_sides,
        });
        Ok(())
    }

    pub fn preview_definition(mut self, point: Option<Point3>) -> Option<Definition> {
        match point {
            Some(point) => self.point(point),
            None => self.enter(),
        }
        .ok()?;
        self.definition
    }

    pub fn preview_circle(self, cursor: Point3) -> Option<CircleDefinition> {
        self.circle.or_else(|| self.base.preview(cursor))
    }
}

fn toggle(value: Option<&str>, previous: bool, kind: Kind) -> Result<bool, CommandError> {
    match value {
        None => Ok(!previous),
        Some(value) => crate::parse_yes_no(value.trim_start_matches('_'))
            .ok_or(CommandError::Usage(kind.usage())),
    }
}
