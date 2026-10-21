//! Native subdivision cylinders layered over the shared circle getter.
use crate::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, GeometryError, Point3, SubD, SubDCylinderCap, Tolerance};
pub const USAGE: &str = "SubDCylinder [AroundFaces=n VerticalFaces=n Solid=Yes|No CapFaceStyle=Tri|Quad] circle-input [DirectionConstraint=Vertical|None BothSides=Yes|No] height-or-point";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub around_faces: usize,
    pub vertical_faces: usize,
    pub solid: bool,
    pub cap: SubDCylinderCap,
    pub direction: Direction,
    pub both_sides: bool,
    pub radius: f64,
    pub height: f64,
    pub diameter: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            around_faces: 8,
            vertical_faces: 4,
            solid: true,
            cap: SubDCylinderCap::Triangles,
            direction: Direction::Vertical,
            both_sides: false,
            radius: 1.,
            height: 1.,
            diameter: false,
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        let n = self.around_faces;
        let h = self.vertical_faces;
        let cap = if self.solid {
            self.cap
        } else {
            SubDCylinderCap::None
        };
        let triangles = cap == SubDCylinderCap::Triangles
            || cap == SubDCylinderCap::Quads && n > 4 && !n.is_multiple_of(2);
        let center = triangles && n > 3 || cap == SubDCylinderCap::Quads && n > 4;
        let caps = if cap == SubDCylinderCap::None {
            Some(0)
        } else if triangles && n > 3 {
            n.checked_mul(2)
        } else if n <= 4 {
            Some(2)
        } else {
            Some(n)
        };
        let vertices = h
            .checked_add(1)
            .and_then(|h| n.checked_mul(h))
            .and_then(|n| n.checked_add(if center { 2 } else { 0 }));
        let faces = n
            .checked_mul(h)
            .and_then(|f| caps.and_then(|c| f.checked_add(c)));
        if n < 3
            || h < 2
            || vertices.is_none_or(|v| v > 100_000)
            || faces.is_none_or(|v| v > 100_000)
            || self.cap == SubDCylinderCap::None
        {
            return Err(CommandError::Usage(
                "SubDCylinder requires at least 3 around faces, 2 vertical faces and at most 100000 control vertices/faces",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub circle: CircleDefinition,
    pub frame: Frame3,
    pub height: f64,
}
impl Definition {
    pub fn geometry(self, p: Preferences) -> Result<Geometry, CommandError> {
        p.validate()?;
        Ok(Geometry::SubD(SubD::try_cylinder(
            self.frame,
            self.circle.radius,
            self.height,
            [p.around_faces, p.vertical_faces],
            if p.solid {
                p.cap
            } else {
                SubDCylinderCap::None
            },
        )?))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Around,
    Vertical,
    Solid,
    Cap,
    Direction,
    Both,
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
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub preferences: Preferences,
    pub circle: Option<CircleDefinition>,
    pub definition: Option<Definition>,
    getter: CircleInput,
    pending: Option<Pending>,
    cursor: Option<Point3>,
}
impl Input {
    pub fn new(plane: Frame3, p: Preferences) -> Self {
        Self {
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
        }
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.circle.is_none() {
            self.getter.set_initial_plane(plane);
        }
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.circle
            .map(|c| c.frame.origin())
            .or(self.getter.anchor())
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
        format!(
            "{}; AroundFaces={} VerticalFaces={} Solid={} CapFaceStyle={}",
            if self.at_start() {
                "center of base circle; 2Point, 3Point"
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
        )
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
                Err(CommandError::Usage(USAGE))
            }
        };
        match key {
            Pending::Around => {
                p.around_faces = value.parse().map_err(|_| CommandError::Usage(USAGE))?;
                if !(3..=100_000).contains(&p.around_faces) {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            Pending::Vertical => {
                p.vertical_faces = value.parse().map_err(|_| CommandError::Usage(USAGE))?;
                if !(2..=100_000).contains(&p.vertical_faces) {
                    return Err(CommandError::Usage(USAGE));
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
                    return Err(CommandError::Usage(USAGE));
                }
            }
            Pending::Direction => {
                p.direction = if value.eq_ignore_ascii_case("Vertical") {
                    Direction::Vertical
                } else if value.eq_ignore_ascii_case("None") {
                    Direction::None
                } else {
                    return Err(CommandError::Usage(USAGE));
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
        if let Some(key) = Pending::parse(name) {
            let allowed = match key {
                Pending::Direction => true,
                Pending::Both => self.circle.is_some(),
                _ => self.at_start(),
            };
            if !allowed {
                return Err(CommandError::Usage(USAGE));
            }
            if let Some(value) = value {
                self.setting(key, value)?
            } else {
                self.pending = Some(key)
            }
            return Ok(true);
        }
        if self.circle.is_none() {
            if ["Tangent", "FitPoints"]
                .iter()
                .any(|m| name.eq_ignore_ascii_case(m))
            {
                return Err(CommandError::Usage(
                    "SubDCylinder Tangent and FitPoints are not implemented yet",
                ));
            }
            if ["Vertical", "Orientation"]
                .iter()
                .any(|m| name.eq_ignore_ascii_case(m))
            {
                return Err(CommandError::Usage(
                    "SubDCylinder does not use Circle's Vertical or Orientation modes",
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
            return Err(CommandError::Usage(USAGE));
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
        if self.circle.is_some() {
            return self.end(value, self.cursor);
        }
        if let Some(c) = self.getter.number(value)? {
            self.accepted_circle(c)
        }
        Ok(())
    }
    pub fn enter(&mut self) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return Err(CommandError::Usage(USAGE));
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
            Err(CommandError::Usage(USAGE))
        }
    }
    fn end(&mut self, value: f64, point: Option<Point3>) -> Result<(), CommandError> {
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD cylinder height",
            }
            .into());
        }
        if value.abs() <= 2.3283064365386963e-10 {
            return Err(GeometryError::Degenerate {
                context: "SubD cylinder height",
            }
            .into());
        }
        let circle = self.circle.ok_or(CommandError::Usage(USAGE))?;
        let mut frame = circle.frame;
        if self.preferences.direction == Direction::None
            && let Some(point) = point
        {
            let normal = circle
                .frame
                .origin()
                .vector_to(point)?
                .normalized_nonzero()?;
            let basis = Frame3::try_from_unit_normal(
                circle.frame.origin(),
                normal.as_vector().scaled(-1.)?.normalized_nonzero()?,
            )?;
            frame = Frame3::try_from_directions(
                basis.origin(),
                basis.x_axis().as_vector(),
                basis.y_axis().as_vector().scaled(-1.)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
        }
        if value < 0. {
            frame = Frame3::try_from_directions(
                frame.origin(),
                frame.x_axis().as_vector(),
                frame.y_axis().as_vector().scaled(-1.)?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
        }
        let height = if self.preferences.both_sides {
            let offset = frame.z_axis().as_vector().scaled(-value.abs())?;
            frame = frame.with_origin(frame.origin().translated(offset)?);
            2. * value.abs()
        } else {
            value.abs()
        };
        if !height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD cylinder height",
            }
            .into());
        }
        self.preferences.height = value;
        self.definition = Some(Definition {
            circle,
            frame,
            height,
        });
        Ok(())
    }
    pub fn observe_cursor(&mut self, p: Point3) {
        self.cursor = Some(p)
    }
    pub fn preview_circle(&self, p: Point3) -> Option<CircleDefinition> {
        self.circle.or_else(|| self.getter.preview(p))
    }
}

pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    preferences: Preferences,
) -> Result<Input, CommandError> {
    let mut input = Input::new(plane, preferences);
    let mut i = 0;
    while i < args.len() {
        if input.definition.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let word = args[i];
        if input.option(word)? {
            i += 1;
            continue;
        }
        if word.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
            input.enter()?;
            i += 1;
            continue;
        }
        if word.contains(',') || !input.wants_number() {
            let (p, n) = if word == "0" && !input.wants_number() {
                (plane.origin(), 1)
            } else {
                crate::parse_point(&args[i..])?
            };
            input.point(p)?;
            i += n;
        } else {
            input.number(crate::parse_finite_real(word)?)?;
            i += 1;
        }
    }
    let _ = doc;
    if input.definition.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(input)
}
#[derive(Default)]
pub(super) struct SubDCylinderCommand(remembered::Remembered<Preferences>);
impl Command for SubDCylinderCommand {
    fn name(&self) -> &'static str {
        "SubDCylinder"
    }
    fn subd_cylinder_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_subd_cylinder_preferences(&self, p: Preferences) -> bool {
        self.0.set(p);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let input = from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let id = doc.add_geometry(input.definition.unwrap().geometry(input.preferences)?)?;
        self.0.set(input.preferences);
        Ok(format!("Added SubD cylinder {id}"))
    }
}
impl CommandRegistry {
    pub fn subd_cylinder_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDCylinder"))
            .and_then(|i| self.commands[*i].subd_cylinder_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_cylinder_preferences(&self, p: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDCylinder"))
            .is_some_and(|i| self.commands[*i].remember_subd_cylinder_preferences(p))
    }
}
