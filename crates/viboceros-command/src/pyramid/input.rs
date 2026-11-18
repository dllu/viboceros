//! Staged base and apex input. Failed candidates leave the current getter open.
use super::{top::Top, *};
use crate::{CommandError, option_name_eq, translation::DestinationConstraint};
use viboceros_geometry::{Frame3, MAX_REGULAR_POLYGON_SIDES, Real};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    First,
    Radial(Point3, Option<Frame3>),
    EdgeEnd(Point3, bool),
    Inner(Base),
    Height(Base),
    Top(Top),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum OptionValue {
    Sides,
    Mode,
    Direction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    kind: Kind,
    stage: Stage,
    mode: BaseMode,
    preferences: Preferences,
    option_value: Option<OptionValue>,
    plane: Frame3,
    tolerance: Tolerance,
    free_cursor: Option<Point3>,
}

impl Input {
    pub fn new(kind: Kind, plane: Frame3, preferences: Preferences, tolerance: Tolerance) -> Self {
        Self {
            kind,
            stage: Stage::First,
            mode: BaseMode::Inscribed,
            preferences,
            option_value: None,
            plane,
            tolerance,
            free_cursor: None,
        }
    }

    pub fn kind(self) -> Kind {
        self.kind
    }

    pub fn preferences(self) -> Preferences {
        self.preferences
    }

    pub fn restore_height(&mut self, height: Real) {
        self.preferences.height = height;
    }

    pub fn tolerance(self) -> Tolerance {
        self.tolerance
    }

    pub fn needs_guide(self) -> bool {
        self.stage == Stage::First && self.preferences.direction == Direction::AroundCurve
    }

    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.stage == Stage::First {
            self.plane = plane;
        }
    }

    pub fn set_guide(&mut self, frame: Frame3) -> Result<(), CommandError> {
        if !self.needs_guide() {
            return Err(self.kind.usage());
        }
        self.stage = Stage::Radial(frame.origin(), Some(frame));
        Ok(())
    }

    pub fn anchor(self) -> Option<Point3> {
        match self.stage {
            Stage::First => None,
            Stage::Radial(point, _) | Stage::EdgeEnd(point, _) => Some(point),
            Stage::Inner(base) | Stage::Height(base) => Some(base.frame.origin()),
            Stage::Top(top) => Some(top.center()),
        }
    }

    pub fn mouse_plane(self) -> Option<Frame3> {
        match self.stage {
            Stage::Radial(_, guide) => guide,
            _ => self.star_roll_plane(),
        }
    }

    pub fn star_roll_plane(self) -> Option<Frame3> {
        match self.stage {
            Stage::Inner(base) if self.preferences.direction == Direction::None => {
                Some(self.plane.with_origin(base.frame.origin()))
            }
            _ => None,
        }
    }

    pub fn cursor_plane(self, active_plane: Frame3) -> Option<Frame3> {
        self.star_roll_plane().or_else(|| {
            (self.kind == Kind::TruncatedPyramid
                && self.preferences.direction == Direction::None
                && matches!(self.stage, Stage::Height(_)))
            .then_some(active_plane)
        })
    }

    fn height_normal(self, base: Base) -> viboceros_geometry::UnitVector3 {
        match self.kind {
            Kind::Pyramid => base.height_normal(),
            Kind::TruncatedPyramid => base.frame.z_axis(),
        }
    }

    pub fn point_constraint(self) -> Option<DestinationConstraint> {
        match self.stage {
            Stage::Top(top) => Some(top.constraint()),
            Stage::Height(base) if self.preferences.direction != Direction::None => {
                Some(DestinationConstraint {
                    anchor: base.frame.origin(),
                    direction: Some(self.height_normal(base)),
                    distance: None,
                })
            }
            _ => None,
        }
    }

    pub fn observe_cursor(&mut self, point: Point3) {
        if let Stage::Inner(mut base) = self.stage
            && self.preferences.direction == Direction::None
        {
            base.observe_star_roll(point);
            self.stage = Stage::Inner(base);
        } else if matches!(self.stage, Stage::Height(_)) && Some(point) != self.anchor() {
            self.free_cursor = Some(point);
        }
    }

    pub fn wants_number(self) -> bool {
        self.option_value.is_none()
            && matches!(
                self.stage,
                Stage::Height(_) | Stage::Inner(_) | Stage::Top(_)
            )
    }

    pub fn prompt(self) -> String {
        let Preferences {
            sides,
            direction,
            solid,
            height,
        } = self.preferences;
        let noun = self.kind.noun();
        let center = match self.mode {
            BaseMode::Inscribed => format!("Center of inscribed {noun}"),
            BaseMode::Circumscribed => format!("Center of circumscribed {noun}"),
            BaseMode::Edge => "Start of edge".into(),
            BaseMode::Star => "Center of star".into(),
        };
        let base_mode = if self.mode == BaseMode::Circumscribed {
            "Circumscribed"
        } else {
            "Inscribed"
        };
        let direction = direction.label();
        match self.option_value {
            Some(OptionValue::Sides) => return format!("Number of sides <{sides}>"),
            Some(OptionValue::Mode) => {
                return format!("Mode <{base_mode}> (Inscribed, Circumscribed)");
            }
            Some(OptionValue::Direction) => {
                let choices = if self.mode == BaseMode::Edge {
                    "None, Vertical"
                } else {
                    "None, Vertical, AroundCurve"
                };
                return format!("Direction constraint <{direction}> ({choices})");
            }
            None => {}
        }
        match self.stage {
            Stage::First if self.needs_guide() => {
                format!("Select curve (DirectionConstraint={direction})")
            }
            Stage::First if matches!(self.mode, BaseMode::Edge | BaseMode::Star) => {
                format!("{center} (NumSides={sides}, DirectionConstraint={direction})")
            }
            Stage::First => {
                let solid = if solid { "Yes" } else { "No" };
                format!(
                    "{center} (NumSides={sides}, Mode={base_mode}, Edge, Star, DirectionConstraint={direction}, Solid={solid})"
                )
            }
            Stage::Radial(..) if self.mode == BaseMode::Star => {
                format!("Corner of star (NumSides={sides})")
            }
            Stage::Radial(..) => {
                let role = if self.mode == BaseMode::Circumscribed {
                    format!("Midpoint of {noun} edge")
                } else {
                    format!("Corner of {noun}")
                };
                format!("{role} (NumSides={sides}, Mode={base_mode})")
            }
            Stage::EdgeEnd(..) => format!("End of edge (NumSides={sides}, Flip)"),
            Stage::Inner(_) => {
                format!("Second star radius. Press Enter for automatic (NumSides={sides})")
            }
            Stage::Height(_) => format!("End of {noun} <{height:.3}>"),
            Stage::Top(_) => "Pick point".into(),
        }
    }

    pub fn option(&mut self, text: &str) -> Result<bool, CommandError> {
        let text = text.trim();
        if let Some(option) = self.option_value {
            if !text.is_empty() && !option_name_eq(text, "Enter") {
                self.set_option_value(option, text)?;
            }
            self.option_value = None;
            return Ok(true);
        }
        let (name, value) = text
            .split_once('=')
            .or_else(|| text.split_once(char::is_whitespace))
            .map_or((text, None), |(name, value)| (name, Some(value.trim())));
        if self.stage == Stage::First && value.is_none() {
            if option_name_eq(name, "Edge") {
                self.mode = BaseMode::Edge;
                if self.preferences.direction == Direction::AroundCurve {
                    self.preferences.direction = Direction::Vertical;
                }
                return Ok(true);
            }
            if option_name_eq(name, "Star") {
                self.mode = BaseMode::Star;
                return Ok(true);
            }
        }
        if let Stage::EdgeEnd(point, flipped) = self.stage
            && option_name_eq(name, "Flip")
        {
            self.stage = Stage::EdgeEnd(point, !flipped);
            return Ok(true);
        }
        let regular = matches!(self.mode, BaseMode::Inscribed | BaseMode::Circumscribed);
        let before_base = !matches!(self.stage, Stage::Height(_) | Stage::Top(_));
        let option = if before_base && option_name_eq(name, "NumSides") {
            OptionValue::Sides
        } else if regular
            && matches!(self.stage, Stage::First | Stage::Radial(..))
            && option_name_eq(name, "Mode")
        {
            OptionValue::Mode
        } else if self.stage == Stage::First && option_name_eq(name, "DirectionConstraint") {
            OptionValue::Direction
        } else if self.stage == Stage::First && regular && option_name_eq(name, "Solid") {
            self.preferences.solid = value
                .map(crate::parse_yes_no)
                .unwrap_or(Some(!self.preferences.solid))
                .ok_or(self.kind.usage())?;
            return Ok(true);
        } else {
            return Ok(false);
        };
        if let Some(value) = value {
            self.set_option_value(option, value)?;
        } else {
            self.option_value = Some(option);
        }
        Ok(true)
    }

    fn set_option_value(&mut self, option: OptionValue, value: &str) -> Result<(), CommandError> {
        let invalid = || self.kind.usage();
        match option {
            OptionValue::Sides => {
                let sides = value.parse::<usize>().map_err(|_| invalid())?;
                if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&sides) {
                    return Err(invalid());
                }
                self.preferences.sides = sides;
                if let Stage::Inner(mut base) = self.stage {
                    base.sides = sides;
                    self.stage = Stage::Inner(base);
                }
            }
            OptionValue::Mode => {
                self.mode = if option_name_eq(value, "Inscribed") {
                    BaseMode::Inscribed
                } else if option_name_eq(value, "Circumscribed") {
                    BaseMode::Circumscribed
                } else {
                    return Err(invalid());
                };
            }
            OptionValue::Direction => {
                let direction = Direction::parse(value).ok_or_else(invalid)?;
                if direction == Direction::AroundCurve && self.mode == BaseMode::Edge {
                    return Err(invalid());
                }
                self.preferences.direction = direction;
            }
        }
        Ok(())
    }

    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, CommandError> {
        if self.option_value.is_some() || self.needs_guide() {
            return Err(self.kind.usage());
        }
        if let Stage::Top(top) = self.stage {
            return top
                .definition(top.picked_size(point)?, self.preferences.solid)
                .map(Some);
        }
        if let Stage::Height(base) = self.stage {
            let target = self
                .point_constraint()
                .map_or(Ok(point), |axis| axis.resolve(point))?;
            let height = base.frame.origin().distance_to(target)?;
            return self.accept_height(base, target, height);
        }
        self.stage = match self.stage {
            Stage::First if self.mode == BaseMode::Edge => Stage::EdgeEnd(point, false),
            Stage::First => Stage::Radial(point, None),
            Stage::Radial(center, guide) => {
                let base = Base::radial(
                    guide.unwrap_or(self.plane),
                    center,
                    point,
                    self.preferences.sides,
                    self.mode,
                    guide.is_some(),
                    self.tolerance,
                )?;
                if self.mode == BaseMode::Star {
                    Stage::Inner(base)
                } else {
                    Stage::Height(base)
                }
            }
            Stage::EdgeEnd(start, flipped) => {
                let base = Base::edge(
                    self.plane,
                    start,
                    point,
                    self.preferences.sides,
                    flipped,
                    self.tolerance,
                )?;
                Stage::Height(base)
            }
            Stage::Inner(mut base) => {
                let radius = base.frame.origin().distance_to(point)?;
                if self.preferences.direction == Direction::None {
                    base.observe_star_roll(point);
                }
                Stage::Height(base.with_inner_radius(radius)?)
            }
            Stage::Height(_) | Stage::Top(_) => unreachable!("end getters handled above"),
        };
        Ok(None)
    }

    pub fn number(&mut self, value: Real) -> Result<Option<Definition>, CommandError> {
        match self.stage {
            Stage::Inner(base) => {
                self.stage = Stage::Height(base.with_inner_radius(value)?);
                Ok(None)
            }
            Stage::Height(base) => {
                let center = base.frame.origin();
                let base_normal = self.height_normal(base);
                let normal = if self.preferences.direction == Direction::None {
                    self.free_cursor
                        .map_or(Ok(base_normal), |point| center.direction_to(point))?
                } else {
                    base_normal
                };
                let target = center.translated(normal.as_vector().scaled(value)?)?;
                self.accept_height(base, target, value)
            }
            Stage::Top(top) => top.definition(value, self.preferences.solid).map(Some),
            _ => Err(self.kind.usage()),
        }
    }

    pub fn enter(&mut self) -> Result<Option<Definition>, CommandError> {
        if self.option_value.take().is_some() {
            return Ok(None);
        }
        match self.stage {
            Stage::Inner(base) => self.number(base.automatic_inner_radius()),
            Stage::Height(_) => self.number(self.preferences.height),
            _ => Err(CommandError::OperationDeclined),
        }
    }

    pub fn preview(self, cursor: Option<Point3>) -> Option<(Base, Option<End>)> {
        if self.option_value.is_some() || self.needs_guide() {
            return None;
        }
        let mut next = self;
        if let Some(point) = cursor {
            if self.kind == Kind::TruncatedPyramid && matches!(self.stage, Stage::Height(_)) {
                next.observe_cursor(point);
            }
            if let Ok(candidate) = next.point(point) {
                if let Some(definition) = candidate {
                    return Some((definition.base, Some(definition.end)));
                }
                if let Stage::Top(top) = next.stage {
                    return Some((top.base, Some(End::Apex(top.center()))));
                }
                if let Stage::Inner(base) | Stage::Height(base) = next.stage {
                    return Some((base, None));
                }
            }
        }
        match self.stage {
            Stage::Inner(base) | Stage::Height(base) => Some((base, None)),
            Stage::Top(top) => Some((top.base, Some(End::Apex(top.center())))),
            _ => None,
        }
    }

    fn accept_height(
        &mut self,
        mut base: Base,
        target: Point3,
        height: Real,
    ) -> Result<Option<Definition>, CommandError> {
        if self.preferences.direction == Direction::None {
            let basis_point = match self.kind {
                Kind::Pyramid => target,
                // The free frustum retains the preview orientation at the
                // height getter even when the accepted world point differs.
                Kind::TruncatedPyramid => self.free_cursor.unwrap_or(target),
            };
            base = base.perpendicular_to_apex(basis_point, self.kind, self.tolerance)?;
        }
        let definition = match self.kind {
            Kind::Pyramid => Some(Definition {
                base,
                end: End::Apex(target),
                solid: self.preferences.solid,
            }),
            Kind::TruncatedPyramid => {
                self.stage = Stage::Top(Top::new(base, target, self.tolerance)?);
                None
            }
        };
        self.preferences.height = height;
        Ok(definition)
    }
}
