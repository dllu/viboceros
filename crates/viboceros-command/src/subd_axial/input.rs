//! Shared staged circle, scalar and axis input.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Around,
    Vertical,
    Solid,
    Cap,
    Direction,
    Both,
    BaseAngle,
    ApexAngle,
}
impl Pending {
    fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "aroundfaces" => Some(Self::Around),
            "verticalfaces" => Some(Self::Vertical),
            "solid" => Some(Self::Solid),
            "capfacestyle" => Some(Self::Cap),
            "directionconstraint" => Some(Self::Direction),
            "bothsides" => Some(Self::Both),
            "orbaseangle" => Some(Self::BaseAngle),
            "orapexangle" => Some(Self::ApexAngle),
            _ => None,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Around => "AroundFaces",
            Self::Vertical => "VerticalFaces",
            Self::Solid => "Solid",
            Self::Cap => "CapFaceStyle",
            Self::Direction => "DirectionConstraint",
            Self::Both => "BothSides",
            Self::BaseAngle => "OrBaseAngle",
            Self::ApexAngle => "OrApexAngle",
        }
    }
}
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
    /// Accept an independently solved reference without adding a temporary circle.
    pub fn reference_circle(&mut self, circle: Circle3) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        self.circle = Some(CircleDefinition::from_circle(circle)?);
        self.preferences.radius = circle.radius();
        self.cursor = None;
        Ok(())
    }
    pub fn tangent_curve_circle(&mut self, circle: Circle3) -> Result<(), CommandError> {
        self.reference_circle(circle)?;
        self.allow_angles = false;
        Ok(())
    }
    /// Native round SubD primitives request height for a zero-radius point fit.
    /// Keep that base staged; a truncated cone can accept a nonzero end radius.
    /// Other primitives reject it at geometry admission without a model edit.
    pub fn fit_points(&mut self, document: &Document) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(circle) = crate::circle_fit_points::selected_circle(document)? {
            return self.reference_circle(circle);
        }
        let mut sums: [viboceros_geometry::FiniteSum; 3] =
            std::array::from_fn(|_| Default::default());
        for point in crate::circle_fit_points::selected_points(document) {
            for (sum, value) in sums.iter_mut().zip(point.to_array()) {
                sum.add(value)?;
            }
        }
        let center = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
        let frame = if self.kind == Kind::TruncatedCone {
            let normal = if let Some(point) =
                crate::circle_fit_points::selected_points(document).find(|&p| p != center)
            {
                Frame3::try_from_unit_normal(
                    center,
                    center.vector_to(point)?.normalized_nonzero()?,
                )?
                .y_axis()
            } else {
                viboceros_geometry::Vector3::try_new(1., 0., 0.)?.normalized_nonzero()?
            };
            Frame3::try_from_unit_normal(center, normal)?
        } else {
            self.getter.construction_plane().with_origin(center)
        };
        self.circle = Some(CircleDefinition { frame, radius: 0. });
        Ok(())
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
    fn setting(&mut self, key: Pending, value: &str) -> Result<(), CommandError> {
        let value = value.trim_start_matches('_');
        let mut p = self.preferences;
        let yes_no = || {
            if value.eq_ignore_ascii_case("Yes") {
                Ok(true)
            } else if value.eq_ignore_ascii_case("No") {
                Ok(false)
            } else {
                Err(CommandError::Usage(self.kind.usage()))
            }
        };
        match key {
            Pending::Around => {
                p.around_faces = value
                    .parse()
                    .map_err(|_| CommandError::Usage(self.kind.usage()))?;
                if !(3..=100_000).contains(&p.around_faces) {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
            }
            Pending::Vertical => {
                p.vertical_faces = value
                    .parse()
                    .map_err(|_| CommandError::Usage(self.kind.usage()))?;
                if !(2..=100_000).contains(&p.vertical_faces) {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
            }
            Pending::Solid => p.solid = yes_no()?,
            Pending::Both => p.both_sides = yes_no()?,
            Pending::Cap => {
                p.cap = if value.eq_ignore_ascii_case("Tri") {
                    SubDCylinderCap::Triangles
                } else if value.eq_ignore_ascii_case("Quad") {
                    SubDCylinderCap::Quads
                } else {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
            }
            Pending::BaseAngle | Pending::ApexAngle => {
                let degrees = crate::parse_finite_real(value)?;
                let base = key == Pending::BaseAngle;
                let bound = if base { 90. } else { 180. };
                if degrees == 0. || degrees.abs() >= bound {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
                if base {
                    p.base_angle = degrees;
                } else {
                    p.apex_angle = degrees;
                }
                let radius = self
                    .circle
                    .ok_or(CommandError::Usage(self.kind.usage()))?
                    .radius;
                let angle = (if base { degrees } else { degrees * 0.5 }).to_radians();
                let height = if base {
                    radius * angle.tan()
                } else {
                    radius / angle.tan()
                };
                self.preferences = p;
                self.pending = None;
                return self.end(height, self.cursor);
            }
            Pending::Direction => {
                p.direction = if value.eq_ignore_ascii_case("Vertical") {
                    Direction::Vertical
                } else if value.eq_ignore_ascii_case("None") {
                    Direction::None
                } else {
                    return Err(CommandError::Usage(self.kind.usage()));
                };
                p.both_sides = false;
            }
        }
        self.preferences = p;
        self.pending = None;
        Ok(())
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        if let Some(key) = self.pending {
            self.setting(key, word)?;
            return Ok(true);
        }
        let word = word.trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if let Some(end) = self.end_circle.as_mut() {
            if end.option(name) {
                self.preferences.diameter = end.diameter();
                if let Some(value) = value {
                    self.number(crate::parse_finite_real(value)?)?;
                }
                return Ok(true);
            }
            if Pending::parse(name).is_some() || CircleSizeMode::parse(name).is_some() {
                return Err(CommandError::Usage(self.kind.usage()));
            }
            return Ok(false);
        }
        if let Some(key) = Pending::parse(name) {
            let allowed = match key {
                Pending::Direction => self.kind != Kind::TruncatedCone || self.at_start(),
                Pending::Both => self.kind == Kind::Cylinder && self.circle.is_some(),
                Pending::BaseAngle | Pending::ApexAngle => {
                    self.kind == Kind::Cone && self.circle.is_some() && self.allow_angles
                }
                _ => self.at_start(),
            };
            if !allowed {
                return Err(CommandError::Usage(self.kind.usage()));
            }
            if let Some(value) = value {
                self.setting(key, value)?
            } else {
                self.pending = Some(key)
            }
            return Ok(true);
        }
        if self.circle.is_none() {
            if ["Vertical", "Orientation"]
                .iter()
                .any(|m| name.eq_ignore_ascii_case(m))
            {
                return Err(CommandError::Usage(
                    "These SubD primitives do not use Circle's Vertical or Orientation modes",
                ));
            }
            if self.getter.option(name) {
                if let Some(value) = value {
                    self.number(crate::parse_finite_real(value)?)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
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
    fn end(&mut self, value: f64, point: Option<Point3>) -> Result<(), CommandError> {
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD primitive height",
            }
            .into());
        }
        if value.abs() <= 2.3283064365386963e-10 {
            return Err(GeometryError::Degenerate {
                context: "SubD primitive height",
            }
            .into());
        }
        let circle = self.circle.ok_or(CommandError::Usage(self.kind.usage()))?;
        let mut frame = circle.frame;
        if self.preferences.direction == Direction::None
            && let Some(point) = point
        {
            let normal = circle
                .frame
                .origin()
                .vector_to(point)?
                .normalized_nonzero()?;
            frame = if self.kind == Kind::TruncatedCone {
                Frame3::try_from_unit_normal(circle.frame.origin(), normal)?
            } else {
                let basis = Frame3::try_from_unit_normal(
                    circle.frame.origin(),
                    normal.as_vector().scaled(-1.)?.normalized_nonzero()?,
                )?;
                Frame3::try_from_directions(
                    basis.origin(),
                    basis.x_axis().as_vector(),
                    basis.y_axis().as_vector().scaled(-1.)?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?
            };
        }
        if value < 0. {
            frame = Frame3::try_from_directions(
                frame.origin(),
                frame.x_axis().as_vector(),
                frame.y_axis().as_vector().scaled(-1.)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
        }
        let height = if self.kind == Kind::Cylinder && self.preferences.both_sides {
            let offset = frame.z_axis().as_vector().scaled(-value.abs())?;
            frame = frame.with_origin(frame.origin().translated(offset)?);
            2. * value.abs()
        } else {
            value.abs()
        };
        if !height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD primitive height",
            }
            .into());
        }
        self.preferences.height = value;
        let definition = Definition {
            kind: self.kind,
            circle,
            frame,
            height,
            end_radius: if self.kind == Kind::Cylinder {
                circle.radius
            } else {
                0.
            },
        };
        if self.kind == Kind::TruncatedCone {
            self.end_circle = Some(super::end_circle::EndCircle::new(
                definition,
                self.preferences,
            )?);
            self.cursor = None;
        } else {
            self.definition = Some(definition);
        }
        Ok(())
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
