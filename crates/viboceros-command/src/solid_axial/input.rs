//! Base-circle, direction, signed-height and end-circle getters.
use super::end_circle::EndCircle;
use super::mesh_topology::Count;
use super::tube_wall::TubeWall;
use super::*;
use crate::{
    circle_input::{CircleDefinition, CircleInput, CircleSizeMode},
    translation::DestinationConstraint,
};
use viboceros_document::Document;
use viboceros_geometry::{Circle3, GeometryError, MeshCapFaceStyle, Point3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub kind: Kind,
    pub preferences: Preferences,
    pub circle: Option<CircleDefinition>,
    pub definition: Option<Definition>,
    base: CircleInput,
    end_circle: Option<EndCircle>,
    tube_wall: Option<TubeWall>,
    tube_height: Option<f64>,
    guide_selected: bool,
    direction_menu: bool,
    mesh_count: Option<Count>,
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
            end_circle: None,
            tube_wall: None,
            tube_height: None,
            guide_selected: false,
            direction_menu: false,
            mesh_count: None,
            cursor: None,
        }
    }

    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.circle.is_none() {
            self.base.set_initial_plane(plane);
        }
    }

    pub fn anchor(self) -> Option<Point3> {
        self.end_plane()
            .map(Frame3::origin)
            .or(self.circle.map(|base| base.frame.origin()))
            .or(self.base.anchor())
    }

    pub fn at_start(self) -> bool {
        self.circle.is_none()
            && self.base.at_start()
            && !self.direction_menu
            && self.mesh_count.is_none()
    }

    pub fn needs_guide(self) -> bool {
        self.at_start()
            && self.preferences.direction == Direction::AroundCurve
            && !self.guide_selected
    }

    pub fn select_guide(&mut self) {
        self.guide_selected = true;
    }

    pub fn continues_base_on_guide(self) -> bool {
        self.guide_selected && self.circle.is_none() && self.base.at_start()
    }

    pub fn validate_mesh_layout(self) -> Result<(), CommandError> {
        let shape = self.kind.shape();
        let cap_count = shape.cap_count(self.preferences.solid, 0.);
        if self.kind.is_mesh()
            && self
                .preferences
                .topology
                .face_count(cap_count)
                .is_none_or(|count| count > shape.mesh_face_limit())
        {
            return Err(GeometryError::TooManyMeshFaces.into());
        }
        Ok(())
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
        if self.end_circle.is_some()
            || self.tube_wall.is_some_and(TubeWall::pending)
            || self.direction_menu
        {
            return None;
        }
        let (direction, distance) = if self.preferences.direction == Direction::None {
            (None, Some(self.tube_height?.abs()))
        } else {
            (Some(circle.frame.z_axis()), None)
        };
        Some(DestinationConstraint {
            anchor: circle.frame.origin(),
            direction,
            distance,
        })
    }

    pub fn end_plane(self) -> Option<Frame3> {
        self.end_circle.map(|end| end.frame)
    }

    pub fn mouse_plane(self) -> Option<Frame3> {
        self.end_plane()
            .or_else(|| (self.tube_wall == Some(TubeWall::Radius)).then_some(self.circle?.frame))
    }

    pub fn zero_is_point(self) -> bool {
        matches!(self.tube_wall, Some(TubeWall::Radius | TubeWall::Ready(_)))
    }

    fn end_mode(self) -> CircleSizeMode {
        if self.preferences.diameter {
            CircleSizeMode::Diameter
        } else {
            CircleSizeMode::Radius
        }
    }

    pub fn prompt(self) -> String {
        let name = self.kind.noun();
        let direction = self.preferences.direction.label();
        if let Some(count) = self.mesh_count {
            return format!(
                "{} <{}>",
                count.label(),
                count.get(self.preferences.topology)
            );
        }
        if self.direction_menu {
            let choices = if self.circle.is_some() {
                "None Vertical"
            } else {
                "None Vertical AroundCurve"
            };
            return format!("Direction constraint <{direction}> ({choices})");
        }
        if self.circle.is_some() {
            if let Some(wall) = self.tube_wall.filter(|wall| wall.pending()) {
                let thickness = self.preferences.wall_thickness;
                return match wall {
                    TubeWall::Thickness => format!("WallThickness <{thickness}>"),
                    _ => {
                        let mode = self.end_mode();
                        let size = self.preferences.end_size / mode.radius(1.);
                        format!("{} <{size}>; WallThickness={thickness}", mode.label())
                    }
                };
            }
            if self.kind.shape() == Shape::TruncatedCone {
                return if self.end_circle.is_some() {
                    let (mode, option) = if self.preferences.diameter {
                        ("Diameter", "Radius")
                    } else {
                        ("Radius", "Diameter")
                    };
                    format!("{mode} at top <{}>; {option}", self.preferences.end_size)
                } else {
                    format!("Top of {name} <{}>", self.preferences.height)
                };
            }
            let mut prompt = format!("End of {name} <{}>", self.preferences.height);
            if self.kind != Kind::Tube && self.preferences.direction != Direction::AroundCurve {
                prompt.push_str(&format!("; DirectionConstraint={direction}"));
            }
            if self.kind.shape() == Shape::Cylinder {
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
            let mut prompt = format!(
                "Base of {name}; DirectionConstraint={direction}; Solid={solid}; 2Point, 3Point, Tangent, FitPoints",
            );
            if self.kind.is_mesh() {
                let topology = self.preferences.topology;
                prompt.push_str(&format!(
                    "; VerticalFaces={}; AroundFaces={}",
                    topology.vertical_count, topology.around_count,
                ));
                if topology.offers_cap_style(self.preferences.solid) {
                    prompt.push_str(&format!("; CapFaceStyle={}", topology.cap_label()));
                }
            }
            return prompt;
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
        if let Some(count) = self.mesh_count {
            let word = word.trim();
            if !word.is_empty() && !option_name_eq(word, "Enter") {
                count.set(&mut self.preferences.topology, word, self.kind)?;
            }
            self.mesh_count = None;
            return Ok(true);
        }
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
        if self.tube_wall.is_some_and(TubeWall::pending) {
            if self.tube_wall == Some(TubeWall::Radius) && option_name_eq(name, "WallThickness") {
                if let Some(value) = value {
                    self.accept_tube_thickness(crate::parse_finite_real(value)?)?;
                } else {
                    self.tube_wall = Some(TubeWall::Thickness);
                }
                return Ok(true);
            }
            return Ok(false);
        }
        if self.end_circle.is_some() {
            if let Some(mode @ (CircleSizeMode::Radius | CircleSizeMode::Diameter)) =
                CircleSizeMode::parse(name)
            {
                let size = value.map(crate::parse_finite_real).transpose()?;
                self.preferences.diameter = mode == CircleSizeMode::Diameter;
                if let Some(size) = size {
                    self.number(size)?;
                }
                return Ok(true);
            }
            return Ok(false);
        }
        let can_toggle_direction = (self.base.is_two_point() && self.circle.is_none()
            || self.circle.is_some()
                && !matches!(self.kind, Kind::Tube)
                && self.kind.shape() != Shape::TruncatedCone)
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
        if option_name_eq(name, "BothSides")
            && self.kind.shape() == Shape::Cylinder
            && (self.circle.is_some() || self.kind.is_mesh() && self.at_start() && value.is_some())
        {
            self.preferences.both_sides = toggle(value, self.preferences.both_sides, self.kind)?;
            return Ok(true);
        }
        if self.kind.is_mesh() && self.at_start() {
            if let Some(count) = Count::parse(name) {
                if let Some(value) = value {
                    count.set(&mut self.preferences.topology, value, self.kind)?;
                } else {
                    self.mesh_count = Some(count);
                }
                return Ok(true);
            }
            let topology = &mut self.preferences.topology;
            if option_name_eq(name, "CapFaceStyle")
                && (value.is_some() || topology.offers_cap_style(self.preferences.solid))
            {
                topology.cap_style = match value {
                    Some(value) => crate::parse_mesh_cap_face_style(value.trim_start_matches('_'))
                        .ok_or(CommandError::Usage(self.kind.usage()))?,
                    None if topology.cap_style == MeshCapFaceStyle::Triangles => {
                        MeshCapFaceStyle::Quadrilaterals
                    }
                    None => MeshCapFaceStyle::Triangles,
                };
                return Ok(true);
            }
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
        self.set_circle(circle);
    }

    fn set_circle(&mut self, circle: CircleDefinition) {
        self.circle = Some(circle);
        self.tube_wall = (self.kind == Kind::Tube).then_some(TubeWall::Radius);
        self.cursor = None;
    }

    fn accept_tube_radius(&mut self, radius: f64) -> Result<(), CommandError> {
        let radius = TubeWall::check_radius(radius)?;
        self.tube_wall = Some(TubeWall::Ready(radius));
        self.preferences.end_size = radius;
        self.cursor = None;
        Ok(())
    }

    fn accept_tube_thickness(&mut self, thickness: f64) -> Result<(), CommandError> {
        let base = self.circle.ok_or(CommandError::Usage(self.kind.usage()))?;
        let thickness = if thickness == 0.0 {
            self.preferences.wall_thickness
        } else {
            thickness
        };
        self.accept_tube_radius(TubeWall::from_thickness(base.radius, thickness)?)?;
        self.preferences.wall_thickness = thickness;
        Ok(())
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
            self.set_circle(definition);
        }
        Ok(())
    }

    pub fn fit_points(&mut self, doc: &Document) -> Result<(), CommandError> {
        let circle = crate::circle_fit_points::selected_circle(doc)?
            .ok_or(CommandError::OperationDeclined)?;
        self.reference_circle(circle, false)
    }

    pub fn observe_cursor(&mut self, point: Point3) {
        if self.kind != Kind::Tube
            && self.end_circle.is_none()
            && let Some(circle) = self.circle
            && !self.direction_menu
        {
            self.cursor = (point != circle.frame.origin()).then_some(point);
        }
    }

    pub fn point(&mut self, mut point: Point3) -> Result<(), CommandError> {
        if self.direction_menu || self.mesh_count.is_some() || self.needs_guide() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(end) = self.end_circle {
            let definition = end.size(self.end_mode(), end.projected_size(point)?)?;
            self.preferences.end_size = definition.end_radius;
            self.definition = Some(definition);
        } else if let Some(circle) = self.circle {
            match self.tube_wall {
                Some(TubeWall::Radius) => {
                    return self.accept_tube_radius(TubeWall::projected_radius(circle, point)?);
                }
                Some(TubeWall::Thickness) => return Err(CommandError::Usage(self.kind.usage())),
                _ => {}
            }
            let offset = circle.frame.origin().vector_to(point)?;
            let height = if self.preferences.direction == Direction::None {
                self.tube_height.map_or_else(|| offset.length(), Ok)?
            } else {
                offset.dot(circle.frame.z_axis().as_vector())?
            };
            self.end(height, Some(point))?;
            let measured_height = if self.kind == Kind::MeshCylinder && self.preferences.both_sides
            {
                2. * height
            } else {
                height
            };
            if self.tube_height.is_none() {
                self.preferences.height = measured_height.abs();
            }
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
        if self.direction_menu || self.mesh_count.is_some() || self.needs_guide() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(end) = self.end_circle {
            let definition = end.size(self.end_mode(), value)?;
            self.preferences.end_size = value.max(0.);
            self.definition = Some(definition);
        } else if self.tube_wall == Some(TubeWall::Radius) {
            self.accept_tube_radius(self.end_mode().radius(value).abs())?;
        } else if self.tube_wall == Some(TubeWall::Thickness) {
            self.accept_tube_thickness(value)?;
        } else if self.tube_wall.is_some() && self.preferences.direction == Direction::None {
            validate_height(value)?;
            self.tube_height = Some(value);
            self.preferences.height = value;
        } else if self.circle.is_some() {
            self.end(value, None)?;
        } else if let Some(circle) = self.base.number(value)? {
            self.accept_circle(circle);
        }
        Ok(())
    }

    pub fn enter(&mut self) -> Result<(), CommandError> {
        if self.mesh_count.take().is_some() {
            return Ok(());
        }
        if self.direction_menu {
            self.direction_menu = false;
            return Ok(());
        }
        if self.end_circle.is_some() {
            return self.number(self.preferences.end_size);
        }
        match self.tube_wall {
            Some(TubeWall::Radius) => return self.accept_tube_radius(self.preferences.end_size),
            Some(TubeWall::Thickness) => {
                return self.accept_tube_thickness(self.preferences.wall_thickness);
            }
            _ => {}
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
        validate_height(height)?;
        let circle = self.circle.ok_or(CommandError::Usage(self.kind.usage()))?;
        let mut frame = circle.frame;
        let target = if self.preferences.direction == Direction::None {
            point.or(self.cursor)
        } else {
            None
        };
        if self.kind.shape() == Shape::TruncatedCone {
            if point.is_some() || target.is_some() {
                let normal =
                    target.map_or(Ok(frame.z_axis()), |p| frame.origin().direction_to(p))?;
                let signed = if height < 0. {
                    normal.opposite()
                } else {
                    normal
                };
                let basis = Frame3::try_from_unit_normal(frame.origin(), signed)?;
                frame = Frame3::try_from_unit_x_and_normal(
                    frame.origin(),
                    basis.x_axis(),
                    normal,
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
            }
        } else if let Some(target) = target {
            let normal = frame.origin().vector_to(target)?.normalized_nonzero()?;
            if self.kind == Kind::Tube && height < 0. {
                frame = Frame3::try_from_unit_normal(frame.origin(), normal)?;
            } else {
                let basis = Frame3::try_from_unit_normal(frame.origin(), normal.opposite())?;
                frame = Frame3::try_from_unit_directions(
                    basis.origin(),
                    basis.x_axis(),
                    basis.y_axis().opposite(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
            }
        }
        let definition = Definition {
            kind: self.kind,
            frame,
            radius: circle.radius,
            height,
            solid: self.preferences.solid,
            both_sides: self.kind.shape() == Shape::Cylinder && self.preferences.both_sides,
            end_radius: self.tube_wall.and_then(TubeWall::radius).unwrap_or(0.),
            topology: self.preferences.topology,
        };
        if self.kind.shape() == Shape::TruncatedCone {
            self.end_circle = Some(EndCircle::new(definition)?);
        } else {
            self.definition = Some(definition);
        }
        self.preferences.height = height;
        Ok(())
    }

    pub fn preview_definition(mut self, point: Option<Point3>) -> Option<Definition> {
        match point {
            Some(point) => self.point(point),
            None => self.enter(),
        }
        .ok()?;
        self.definition.or_else(|| {
            self.end_circle?
                .size(self.end_mode(), self.preferences.end_size)
                .ok()
        })
    }

    pub fn preview_circle(self, cursor: Point3) -> Option<CircleDefinition> {
        self.circle.or_else(|| self.base.preview(cursor))
    }

    pub fn preview_tube_circle(self, cursor: Option<Point3>) -> Option<CircleDefinition> {
        let base = self.circle?;
        let radius = match self.tube_wall? {
            TubeWall::Radius => cursor.map_or(Some(self.preferences.end_size), |point| {
                TubeWall::projected_radius(base, point).ok()
            })?,
            TubeWall::Thickness => {
                TubeWall::from_thickness(base.radius, self.preferences.wall_thickness).ok()?
            }
            TubeWall::Ready(radius) => radius,
        };
        Some(CircleDefinition {
            radius: TubeWall::check_radius(radius).ok()?,
            ..base
        })
    }
}

fn validate_height(height: f64) -> Result<(), GeometryError> {
    if !height.is_finite() || height.abs() <= Tolerance::NUMERICAL_VALIDATION.absolute() {
        return Err(GeometryError::Degenerate {
            context: "axial primitive height",
        });
    }
    Ok(())
}

fn toggle(value: Option<&str>, previous: bool, kind: Kind) -> Result<bool, CommandError> {
    match value {
        None => Ok(!previous),
        Some(value) => crate::parse_yes_no(value.trim_start_matches('_'))
            .ok_or(CommandError::Usage(kind.usage())),
    }
}
