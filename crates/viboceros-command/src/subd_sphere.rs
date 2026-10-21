//! Subdivision-sphere styles layered over the shared sphere construction getter.
use crate::circle_input::CircleDefinition;
use crate::sphere_input::SphereInput;
use crate::{Command, CommandContext, CommandError, CommandRegistry, remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, SubD, SubDSphereStyle, Tolerance};
pub const USAGE: &str = "SubDSphere [Style=Quad|Triangle|UV Subdivisions=n | AroundFaces=n VerticalFaces=n] sphere-input";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Quad,
    Triangle,
    Uv,
}
impl Style {
    pub fn label(self) -> &'static str {
        match self {
            Self::Quad => "Quad",
            Self::Triangle => "Triangle",
            Self::Uv => "UV",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("Quad") {
            Some(Self::Quad)
        } else if ["Tri", "Triangle"]
            .iter()
            .any(|t| s.eq_ignore_ascii_case(t))
        {
            Some(Self::Triangle)
        } else if s.eq_ignore_ascii_case("UV") {
            Some(Self::Uv)
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub style: Style,
    pub quad_subdivisions: usize,
    pub triangle_subdivisions: usize,
    pub around_faces: usize,
    pub vertical_faces: usize,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            style: Style::Quad,
            quad_subdivisions: 1,
            triangle_subdivisions: 1,
            around_faces: 10,
            vertical_faces: 10,
        }
    }
}
impl Preferences {
    pub fn geometry_style(self) -> SubDSphereStyle {
        match self.style {
            Style::Quad => SubDSphereStyle::Quads {
                subdivisions: self.quad_subdivisions,
            },
            Style::Triangle => SubDSphereStyle::Triangles {
                subdivisions: self.triangle_subdivisions,
            },
            Style::Uv => SubDSphereStyle::Uv {
                around: self.around_faces,
                vertical: self.vertical_faces,
            },
        }
    }
    pub fn validate(self) -> Result<(), CommandError> {
        Ok(self.geometry_style().validate()?)
    }
    pub fn subdivisions(self) -> usize {
        if self.style == Style::Triangle {
            self.triangle_subdivisions
        } else {
            self.quad_subdivisions
        }
    }
}
pub fn geometry(
    d: CircleDefinition,
    p: Preferences,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    Ok(Geometry::SubD(SubD::try_sphere(
        d.frame,
        d.radius,
        p.geometry_style(),
        tolerance,
    )?))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Style,
    Subdivision,
    Around,
    Vertical,
}
impl Pending {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "style" => Some(Self::Style),
            "subdivisions" => Some(Self::Subdivision),
            "aroundfaces" => Some(Self::Around),
            "verticalfaces" => Some(Self::Vertical),
            _ => None,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Style => "Style",
            Self::Subdivision => "Subdivisions",
            Self::Around => "AroundFaces",
            Self::Vertical => "VerticalFaces",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub preferences: Preferences,
    pub definition: Option<CircleDefinition>,
    pub no_output: bool,
    pub getter: SphereInput,
    pending: Option<Pending>,
}
impl Input {
    pub fn new(plane: Frame3, preferences: Preferences) -> Self {
        Self {
            preferences,
            definition: None,
            no_output: false,
            getter: SphereInput::new(plane),
            pending: None,
        }
    }
    pub fn at_start(&self) -> bool {
        self.definition.is_none() && self.getter.at_start()
    }
    pub fn anchor(&self) -> Option<viboceros_geometry::Point3> {
        self.getter.anchor()
    }
    pub fn mouse_plane(&self) -> Option<Frame3> {
        self.getter.mouse_plane()
    }
    pub fn pending_value(&self) -> bool {
        self.pending.is_some()
    }
    pub fn wants_number(&self) -> bool {
        self.pending.is_some() || self.getter.requests_size()
    }
    pub fn scalar_units(&self) -> bool {
        self.pending.is_some()
            || self.getter.size_mode() == crate::circle_input::CircleSizeMode::Area
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        self.getter.set_initial_plane(plane);
    }
    pub fn prompt(&self) -> String {
        if let Some(key) = self.pending {
            return key.label().into();
        }
        let counts = if self.preferences.style == Style::Uv {
            format!(
                "AroundFaces={} VerticalFaces={}",
                self.preferences.around_faces, self.preferences.vertical_faces
            )
        } else {
            format!("Subdivisions={}", self.preferences.subdivisions())
        };
        format!(
            "{}; Style={} {counts}",
            self.getter
                .prompt()
                .replace("reference sphere", "sphere")
                .replace("Reference sphere", "Sphere"),
            self.preferences.style.label()
        )
    }
    fn setting(&mut self, key: Pending, value: &str) -> Result<(), CommandError> {
        let value = value.trim_start_matches('_');
        let mut p = self.preferences;
        match key {
            Pending::Style => p.style = Style::parse(value).ok_or(CommandError::Usage(USAGE))?,
            Pending::Subdivision => {
                let n = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                if p.style == Style::Quad && n <= 6 {
                    p.quad_subdivisions = n;
                } else if p.style == Style::Triangle && n <= 5 {
                    p.triangle_subdivisions = n;
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            Pending::Around => {
                let n = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                if !(3..=100_000).contains(&n) {
                    return Err(CommandError::Usage(USAGE));
                }
                p.around_faces = n;
            }
            Pending::Vertical => {
                let n = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                if !(2..=100_000).contains(&n) {
                    return Err(CommandError::Usage(USAGE));
                }
                p.vertical_faces = n;
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
            if !self.at_start()
                || matches!(key, Pending::Around | Pending::Vertical)
                    && self.preferences.style != Style::Uv
                || key == Pending::Subdivision && self.preferences.style == Style::Uv
            {
                return Err(CommandError::Usage(USAGE));
            }
            if let Some(value) = value {
                self.setting(key, value)?;
            } else {
                self.pending = Some(key);
            }
            return Ok(true);
        }
        if self.getter.option(name) {
            if let Some(value) = value {
                self.number(crate::parse_finite_real(value)?)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
    pub fn accept_reference(&mut self, d: CircleDefinition) -> Result<(), CommandError> {
        if !d.radius.is_finite() || d.radius <= 0. {
            return Err(CommandError::Usage(USAGE));
        }
        self.definition = Some(d);
        Ok(())
    }
    pub fn number(&mut self, value: f64) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return self.option(&value.to_string()).map(|_| ());
        }
        if let Some(d) = self.getter.number(value)? {
            self.accept_reference(d)?;
        }
        Ok(())
    }
    pub fn point(&mut self, point: viboceros_geometry::Point3) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if let Some(d) = self.getter.point(point)? {
            self.accept_reference(d)?;
        }
        Ok(())
    }
    pub fn enter(&mut self) -> Result<(), CommandError> {
        if self.pending.take().is_some() {
            return Ok(());
        }
        let value = self
            .getter
            .default_size()
            .filter(|_| {
                matches!(
                    self.getter.size_mode(),
                    crate::circle_input::CircleSizeMode::Radius
                        | crate::circle_input::CircleSizeMode::Diameter
                )
            })
            .ok_or(CommandError::Usage(USAGE))?;
        self.number(value)
    }
    pub fn preview(&self, point: viboceros_geometry::Point3) -> Option<CircleDefinition> {
        let mut copy = *self;
        copy.point(point).ok()?;
        copy.definition
    }
}
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    p: Preferences,
) -> Result<Input, CommandError> {
    let mut input = Input::new(plane, p);
    let mut i = 0;
    while i < args.len() {
        if input.definition.is_some() || input.no_output {
            return Err(CommandError::Usage(USAGE));
        }
        let word = args[i].trim_start_matches('_');
        if input.at_start() && word.eq_ignore_ascii_case("Tangent") {
            let mut tangent =
                crate::circle_tangent_input::Input::new(plane, doc.tolerance(), Default::default());
            i += 1;
            loop {
                let (n, c) = tangent.argument(doc, &args[i..])?;
                i += n;
                if let Some(c) = c {
                    input.accept_reference(CircleDefinition::from_circle(c)?)?;
                    break;
                }
            }
            continue;
        }
        if input.at_start() && word.eq_ignore_ascii_case("FitPoints") {
            let points = crate::point_fitting::PointFitKind::Sphere.selected_points(doc);
            if let Some(d) = SphereInput::fit_optional(&points)? {
                input.accept_reference(d)?;
            } else {
                input.no_output = true;
            }
            i += 1;
            continue;
        }
        if input.at_start() && word.eq_ignore_ascii_case("AroundCurve") {
            i += 1;
            let word = args.get(i).ok_or(CommandError::Usage(USAGE))?;
            let (key, value) = word
                .trim_start_matches(['_', '-'])
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            let (id, edge) = if key.eq_ignore_ascii_case("Curve") {
                (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
            } else if key.eq_ignore_ascii_case("Edge") {
                let (id, edge) =
                    crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                (id, Some(edge))
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            let curve = crate::curve_reference::CapturedCurve::capture(doc, id, edge)?;
            i += 1;
            let (center, n) = crate::parse_point(&args[i..])?;
            i += n;
            input.getter = SphereInput::around_frame(crate::curve_reference::perpendicular_frame(
                curve.curve_ref(),
                center,
            )?);
            continue;
        }
        if input.option(args[i])? {
            i += 1;
            continue;
        }
        if word.eq_ignore_ascii_case("Enter") {
            input.enter()?;
            i += 1;
            continue;
        }
        if word.contains(',') || !input.wants_number() {
            let (point, n) = if word == "0" && !input.wants_number() {
                (plane.origin(), 1)
            } else {
                crate::parse_point(&args[i..])?
            };
            input.point(point)?;
            i += n;
        } else {
            input.number(crate::parse_finite_real(word)?)?;
            i += 1;
        }
    }
    if input.definition.is_none() && !input.no_output {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(input)
}
#[derive(Default)]
pub(super) struct SubDSphereCommand(remembered::Remembered<Preferences>);
impl Command for SubDSphereCommand {
    fn name(&self) -> &'static str {
        "SubDSphere"
    }
    fn subd_sphere_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_subd_sphere_preferences(&self, p: Preferences) -> bool {
        self.0.set(p);
        true
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<crate::ObjectSelectionPrompt>, CommandError> {
        Ok(
            matches!(args,[s]if s.trim_start_matches(['_','-']).eq_ignore_ascii_case("FitPoints"))
                .then(selection_prompt),
        )
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let result = self.run_in_context(doc, args, context)?;
        let ids = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(ids)?;
        doc.clear_selection();
        Ok(result)
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let input = from_arguments(doc, args, context.construction_plane, self.0.get())?;
        if input.no_output {
            self.0.set(input.preferences);
            return Ok("No sphere fits the selected points".into());
        }
        let id = doc.add_geometry(geometry(
            input.definition.unwrap(),
            input.preferences,
            doc.tolerance(),
        )?)?;
        self.0.set(input.preferences);
        Ok(format!("Added SubD sphere {id}"))
    }
}
pub fn selection_prompt() -> crate::ObjectSelectionPrompt {
    crate::ObjectSelectionPrompt {
        command: "SubDSphere FitPoints",
        filter: crate::ObjectSelectionFilter::Points,
        options: vec![],
        menus: vec![],
        choices: vec![],
        workflow: crate::ObjectSelectionWorkflow::OptionsDuringSelection,
    }
}
impl CommandRegistry {
    pub fn subd_sphere_preferences(&self) -> Preferences {
        self.lookup
            .get(&crate::normalize_command_name("SubDSphere"))
            .and_then(|i| self.commands[*i].subd_sphere_preferences())
            .unwrap_or_default()
    }
    pub fn remember_subd_sphere_preferences(&self, p: Preferences) -> bool {
        self.lookup
            .get(&crate::normalize_command_name("SubDSphere"))
            .is_some_and(|i| self.commands[*i].remember_subd_sphere_preferences(p))
    }
}
