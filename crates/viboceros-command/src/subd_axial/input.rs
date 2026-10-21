//! Shared staged circle, scalar and axis input.
use super::*;
mod axis;
mod options;
mod references;
use options::Pending;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub preferences: Preferences,
    pub kind: Kind,
    pub circle: Option<CircleDefinition>,
    pub definition: Option<Definition>,
    getter: CircleInput,
    pending: Option<Pending>,
    cursor: Option<Point3>,
    allow_angles: bool,
    end_circle: Option<super::end_circle::EndCircle>,
}
impl Input {
    pub fn new(plane: Frame3, p: Preferences) -> Self {
        Self::new_for(Kind::Cylinder, plane, p)
    }
    pub fn new_for(kind: Kind, plane: Frame3, p: Preferences) -> Self {
        Self {
            kind,
            preferences: p,
            circle: None,
            definition: None,
            getter: CircleInput::new(plane).with_size_mode(if p.diameter {
                CircleSizeMode::Diameter
            } else {
                CircleSizeMode::Radius
            }),
            pending: None,
            cursor: None,
            allow_angles: true,
            end_circle: None,
        }
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.circle.is_none() {
            self.getter.set_initial_plane(plane);
        }
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.end_circle.map(|e| e.frame().origin()).or(self
            .circle
            .map(|c| c.frame.origin())
            .or(self.getter.anchor()))
    }
    pub fn at_start(&self) -> bool {
        self.circle.is_none() && self.getter.anchor().is_none()
    }
    pub fn pending_value(&self) -> bool {
        self.pending.is_some()
    }
    pub fn uses_scalar_units(&self) -> bool {
        self.pending.is_some()
            || self.circle.is_none() && self.getter.size_mode() == CircleSizeMode::Area
    }
    pub fn wants_number(&self) -> bool {
        self.pending.is_some() || self.circle.is_some() || self.getter.requests_size()
    }
    pub fn mouse_plane(&self) -> Option<Frame3> {
        if let Some(end) = self.end_circle {
            return Some(end.frame());
        }
        if let Some(c) = self.circle {
            if self.preferences.direction == Direction::Vertical {
                Frame3::try_from_directions(
                    c.frame.origin(),
                    c.frame.x_axis().as_vector(),
                    c.frame.z_axis().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .ok()
            } else {
                None
            }
        } else {
            self.getter.mouse_plane()
        }
    }
    pub fn prompt(&self) -> String {
        if let Some(p) = self.pending {
            return p.label().to_string();
        }
        if let Some(end) = self.end_circle {
            return format!(
                "{} at top <{}>; {}",
                if end.diameter() { "Diameter" } else { "Radius" },
                self.preferences.end_size,
                if end.diameter() { "Radius" } else { "Diameter" }
            );
        }
        if self.circle.is_some() && self.kind == Kind::TruncatedCone {
            return format!("top of truncated cone <{}>", self.preferences.height);
        }
        if self.circle.is_some() && self.kind == Kind::Cone && self.allow_angles {
            return format!(
                "end of cone <{}>; DirectionConstraint={:?} OrBaseAngle={} OrApexAngle={}",
                self.preferences.height,
                self.preferences.direction,
                self.preferences.base_angle,
                self.preferences.apex_angle
            );
        }
        if self.circle.is_some() && self.kind == Kind::Cone {
            return format!(
                "end of cone <{}>; DirectionConstraint={:?}",
                self.preferences.height, self.preferences.direction
            );
        }
        if self.circle.is_some() {
            return format!(
                "end of cylinder <{}>; DirectionConstraint={:?} BothSides={}",
                self.preferences.height,
                self.preferences.direction,
                if self.preferences.both_sides {
                    "Yes"
                } else {
                    "No"
                }
            );
        }
        let prompt = format!(
            "{}; AroundFaces={} VerticalFaces={} Solid={} CapFaceStyle={}",
            if self.at_start() {
                "center of base circle; 2Point, 3Point, Tangent, FitPoints"
            } else {
                self.getter.prompt()
            },
            self.preferences.around_faces,
            self.preferences.vertical_faces,
            if self.preferences.solid { "Yes" } else { "No" },
            if self.preferences.cap == SubDCylinderCap::Quads {
                "Quad"
            } else {
                "Tri"
            }
        );
        if self.kind == Kind::TruncatedCone && self.at_start() {
            format!(
                "{prompt}; DirectionConstraint={:?}",
                self.preferences.direction
            )
        } else {
            prompt
        }
    }
    fn accepted_circle(&mut self, circle: CircleDefinition) {
        if self.getter.remembers_radius() {
            self.preferences.radius = circle.radius;
        }
        if self.getter.has_size_mode() {
            self.preferences.diameter = match self.getter.size_mode() {
                CircleSizeMode::Radius => false,
                CircleSizeMode::Diameter => true,
                _ => self.preferences.diameter,
            };
        }
        self.circle = Some(circle);
        self.cursor = None;
    }
    pub fn point(&mut self, p: Point3) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(end) = self.end_circle {
            let (d, size) = end.point(p)?;
            self.preferences.end_size = size;
            self.preferences.diameter = end.diameter();
            self.definition = Some(d);
            return Ok(());
        }
        if let Some(circle) = self.circle {
            let vector = circle.frame.origin().vector_to(p)?;
            let height = if self.preferences.direction == Direction::Vertical {
                vector.dot(circle.frame.z_axis().as_vector())?
            } else {
                vector.length()?
            };
            self.end(height, Some(p))
        } else {
            if let Some(c) = self.getter.point(p)? {
                self.accepted_circle(c)
            }
            Ok(())
        }
    }
    pub fn number(&mut self, value: f64) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return self.option(&value.to_string()).map(|_| ());
        }
        if let Some(end) = self.end_circle {
            let (d, size) = end.number(value)?;
            self.preferences.end_size = size;
            self.preferences.diameter = end.diameter();
            self.definition = Some(d);
            return Ok(());
        }
        if self.circle.is_some() {
            return self.end(value, self.cursor);
        }
        if let Some(c) = self.getter.number(value)? {
            self.accepted_circle(c)
        }
        Ok(())
    }
    pub fn enter(&mut self) -> Result<(), CommandError> {
        if let Some(key) = self.pending {
            return match key {
                Pending::BaseAngle => self.number(self.preferences.base_angle),
                Pending::ApexAngle => self.number(self.preferences.apex_angle),
                _ => Err(CommandError::Usage(self.kind.usage())),
            };
        }
        if self.end_circle.is_some() {
            return self.number(self.preferences.end_size);
        }
        if self.circle.is_some() {
            self.number(self.preferences.height)
        } else if self.getter.requests_size()
            && self.getter.has_size_mode()
            && matches!(
                self.getter.size_mode(),
                CircleSizeMode::Radius | CircleSizeMode::Diameter
            )
        {
            self.number(if self.getter.size_mode() == CircleSizeMode::Diameter {
                2. * self.preferences.radius
            } else {
                self.preferences.radius
            })
        } else {
            Err(CommandError::Usage(self.kind.usage()))
        }
    }
    pub fn preview_definition(&self, cursor: Option<Point3>) -> Option<Definition> {
        let mut copy = *self;
        if let Some(point) = cursor {
            copy.point(point).ok()?;
        }
        if copy.end_circle.is_some() && copy.definition.is_none() {
            copy.enter().ok()?;
        }
        copy.definition
    }
    pub fn observe_cursor(&mut self, p: Point3) {
        self.cursor = Some(p)
    }
    pub fn preview_circle(&self, p: Point3) -> Option<CircleDefinition> {
        self.circle.or_else(|| self.getter.preview(p))
    }
}
