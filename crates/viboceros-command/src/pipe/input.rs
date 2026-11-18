//! Pipe's rail, radius, thickness and station getters.
use super::*;
use viboceros_geometry::Point3;

mod draft;
pub use draft::Draft;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RailMode {
    #[default]
    Single,
    Multiple,
    Chain,
}

#[derive(Clone, Copy, Debug)]
pub struct Preferences {
    radius: Real,
    second: Real,
    thickness: Real,
    by_thickness: bool,
    diameter: bool,
    thick: bool,
    output: Output,
    cap: PipeCap,
    blend: SweepBlend,
    fit_rail: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            radius: 1.,
            second: 2.,
            thickness: 1.,
            by_thickness: false,
            diameter: false,
            thick: false,
            output: Output::Surface,
            cap: PipeCap::Flat,
            blend: SweepBlend::Local,
            fit_rail: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Rail,
    Radius { fraction: Real, first: Option<Real> },
    Thickness { fraction: Real, first: Real },
    Station,
    Complete,
}

#[derive(Clone, Copy, Debug)]
enum Menu {
    Cap,
    Blend,
    Output,
}

#[derive(Clone, Debug)]
pub struct Input {
    stage: Stage,
    menu: Option<Menu>,
    rail_mode: RailMode,
    closed: bool,
    settings: Preferences,
    fixed_thickness: Option<Real>,
    sections: Vec<(Real, [Real; 2])>,
}

impl Input {
    pub fn new(settings: Preferences) -> Self {
        Self {
            stage: Stage::Rail,
            menu: None,
            rail_mode: RailMode::Single,
            closed: false,
            settings,
            fixed_thickness: None,
            sections: vec![],
        }
    }

    pub fn preferences(&self) -> Preferences {
        self.settings
    }

    fn options(&self) -> PipeOptions {
        PipeOptions {
            blend: self.settings.blend,
            fit_rail: self.settings.fit_rail,
            cap: self.settings.cap,
            wall_thickness: None,
        }
    }

    fn closed_subd(&self) -> bool {
        self.closed && self.settings.output == Output::SubD
    }

    fn active_thickness(&self) -> bool {
        self.settings.thick && !self.closed_subd()
    }

    fn subd_thick(&self) -> bool {
        self.settings.output == Output::SubD && self.active_thickness()
    }

    fn normalize_cap(&mut self) {
        if self.subd_thick() && self.settings.cap == PipeCap::Round {
            self.settings.cap = PipeCap::Flat;
        }
    }

    pub fn selecting(&self) -> bool {
        matches!(self.stage, Stage::Rail)
    }

    pub fn rail_mode(&self) -> RailMode {
        self.rail_mode
    }

    pub fn picking_station(&self) -> bool {
        self.menu.is_none() && matches!(self.stage, Stage::Station)
    }

    pub fn wants_number(&self) -> bool {
        self.menu.is_none() && matches!(self.stage, Stage::Radius { .. } | Stage::Thickness { .. })
    }

    pub fn start_sizes(&mut self, closed: bool) {
        self.closed = closed;
        self.stage = Stage::Radius {
            fraction: 0.,
            first: None,
        };
    }

    pub fn radius_plane(&self, rail: &Rail) -> Option<Frame3> {
        let Stage::Radius { fraction, .. } = self.stage else {
            return None;
        };
        if self.menu.is_some() {
            return None;
        }
        rail.frame(fraction).ok()
    }

    fn radius_name(&self) -> &'static str {
        if self.settings.diameter {
            "diameter"
        } else {
            "radius"
        }
    }

    fn second_radius_name(&self) -> &'static str {
        if self.settings.diameter {
            "Radius"
        } else {
            "Diameter"
        }
    }

    pub fn prompt(&self) -> String {
        if let Some(menu) = self.menu {
            return match menu {
                Menu::Cap if self.subd_thick() => "Cap (None, Flat)".into(),
                Menu::Cap => "Cap (None, Flat, Round)".into(),
                Menu::Blend => "ShapeBlending (Local, Global)".into(),
                Menu::Output => "Output (Surface, SubD)".into(),
            };
        }
        let name = self.radius_name();
        let toggle = self.second_radius_name();
        let cap = match self.settings.cap {
            PipeCap::None => "None",
            PipeCap::Flat => "Flat",
            PipeCap::Round => "Round",
        };
        let blend = match self.settings.blend {
            SweepBlend::Local => "Local",
            _ => "Global",
        };
        let yes_no = |b| if b { "Yes" } else { "No" };
        match self.stage {
            Stage::Rail => match self.rail_mode {
                RailMode::Single => "Select rail (ChainEdges, Multiple)".into(),
                RailMode::Multiple => "Select rails. Press Enter when done".into(),
                RailMode::Chain => "Select connected edges. Press Enter when done".into(),
            },
            Stage::Thickness { .. } => {
                format!("Wall thickness <{}> (Radius)", self.settings.thickness)
            }
            Stage::Station => format!("Point for next {name}. Press Enter to finish"),
            Stage::Complete => "Create pipe".into(),
            Stage::Radius { fraction, first } => {
                let prefix = match (self.rail_mode, self.closed, fraction, first.is_some()) {
                    (RailMode::Multiple, _, _, false) => format!("Pipe {name}"),
                    (_, _, 0., false) if self.closed => format!("{name} for closed pipe"),
                    (_, _, 0., second) => {
                        format!("{}start {name}", if second { "Second " } else { "" })
                    }
                    (_, _, 1., second) => {
                        format!("{}end {name}", if second { "Second " } else { "" })
                    }
                    (_, _, _, true) => format!("Second {name}"),
                    (_, _, _, false) => name.to_owned(),
                };
                let units = self.default_size(first) * if self.settings.diameter { 2. } else { 1. };
                let fit = yes_no(self.settings.fit_rail);
                let output = self.settings.output.label();
                if self.closed_subd() {
                    return format!(
                        "{prefix} <{units}> ({toggle}, Output={output}, ShapeBlending={blend}, FitRail={fit})"
                    );
                }
                let thick = yes_no(self.settings.thick);
                let thickness = if first.is_some() {
                    ", WallThickness"
                } else {
                    ""
                };
                format!(
                    "{prefix} <{units}> ({toggle}, Output={output}, Thick={thick}, Cap={cap}, ShapeBlending={blend}, FitRail={fit}{thickness})"
                )
            }
        }
    }

    /// Apply an option and indicate whether it completed the last size.
    pub fn option(&mut self, text: &str) -> Result<Option<bool>, CommandError> {
        let text = text.trim().trim_start_matches(['_', '-']);
        if let Some(menu) = self.menu {
            if text.is_empty() || option_name_eq(text, "Enter") {
                self.menu = None;
                return Ok(Some(false));
            }
            self.choose(menu, text)?;
            self.menu = None;
            return Ok(Some(false));
        }
        let (name, value) = text
            .split_once('=')
            .or_else(|| text.split_once(char::is_whitespace))
            .map_or((text, None), |(name, value)| {
                (name.trim(), Some(value.trim()))
            });
        if self.selecting() && value.is_none() {
            if option_name_eq(name, "Multiple") {
                self.rail_mode = RailMode::Multiple;
                return Ok(Some(false));
            }
            if option_name_eq(name, "ChainEdges") {
                self.rail_mode = RailMode::Chain;
                return Ok(Some(false));
            }
        }
        let menu = ["Cap", "ShapeBlending", "Output"]
            .into_iter()
            .zip([Menu::Cap, Menu::Blend, Menu::Output])
            .find_map(|(label, menu)| option_name_eq(name, label).then_some(menu));
        if let Some(menu) = menu {
            if let Some(value) = value {
                self.choose(menu, value)?;
            } else {
                self.menu = Some(menu);
            }
            return Ok(Some(false));
        }
        if option_name_eq(name, "FitRail") {
            self.settings.fit_rail = toggle(value, self.settings.fit_rail)?;
        } else if option_name_eq(name, "Thick") && self.at_first_size() && !self.closed_subd() {
            self.settings.thick = toggle(value, self.settings.thick)?;
            self.normalize_cap();
            if !self.settings.thick {
                self.fixed_thickness = None;
            }
        } else if option_name_eq(name, "Radius") || option_name_eq(name, "Diameter") {
            if let Stage::Thickness { fraction, first } = self.stage {
                if !option_name_eq(name, "Radius") {
                    return Ok(None);
                }
                self.stage = Stage::Radius {
                    fraction,
                    first: Some(first),
                };
                self.fixed_thickness = None;
                self.settings.by_thickness = false;
            } else {
                self.settings.diameter = option_name_eq(name, "Diameter");
            }
            if let Some(value) = value {
                return self.number(parse_finite_real(value)?).map(Some);
            }
        } else if option_name_eq(name, "WallThickness") && !self.closed_subd() {
            if let Stage::Radius {
                fraction,
                first: Some(first),
            }
            | Stage::Thickness { fraction, first } = self.stage
            {
                if let Some(value) = value {
                    return self
                        .accept_thickness(fraction, first, parse_finite_real(value)?)
                        .map(Some);
                }
                self.settings.by_thickness = true;
                self.stage = Stage::Thickness { fraction, first };
            } else if self.at_first_size() {
                let thickness = value.ok_or(CommandError::Usage("Enter WallThickness=distance"))?;
                let thickness = parse_finite_real(thickness)?;
                if thickness == 0. {
                    return Err(CommandError::Usage(USAGE));
                }
                self.settings.thick = true;
                self.normalize_cap();
                self.settings.thickness = thickness;
                self.fixed_thickness = Some(thickness);
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }
        Ok(Some(false))
    }

    fn at_first_size(&self) -> bool {
        matches!(
            self.stage,
            Stage::Rail
                | Stage::Radius {
                    fraction: 0.,
                    first: None
                }
        )
    }

    fn choose(&mut self, menu: Menu, choice: &str) -> Result<(), CommandError> {
        match menu {
            Menu::Cap if option_name_eq(choice, "None") => self.settings.cap = PipeCap::None,
            Menu::Cap if option_name_eq(choice, "Flat") => self.settings.cap = PipeCap::Flat,
            Menu::Cap if option_name_eq(choice, "Round") && !self.subd_thick() => {
                self.settings.cap = PipeCap::Round
            }
            Menu::Blend if option_name_eq(choice, "Local") => {
                self.settings.blend = SweepBlend::Local
            }
            Menu::Blend if option_name_eq(choice, "Global") => {
                self.settings.blend = SweepBlend::Global
            }
            Menu::Output => {
                self.settings.output = Output::parse(choice)?;
                self.normalize_cap();
                if self.closed_subd()
                    && let Stage::Radius {
                        fraction,
                        first: Some(first),
                    }
                    | Stage::Thickness { fraction, first } = self.stage
                {
                    self.accept_pair(fraction, [first, 0.]);
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
        Ok(())
    }

    fn default_size(&self, first: Option<Real>) -> Real {
        if let Some(first) = first {
            if self.settings.second == first {
                first * 2.
            } else {
                self.settings.second
            }
        } else {
            self.settings.radius
        }
    }

    pub fn enter(&mut self) -> Result<bool, CommandError> {
        if self.menu.take().is_some() {
            return Ok(false);
        }
        match self.stage {
            Stage::Thickness { fraction, first } => {
                self.accept_thickness(fraction, first, self.settings.thickness)
            }
            Stage::Radius { fraction, first } => {
                self.accept_size(fraction, first, self.default_size(first))
            }
            Stage::Station => {
                self.stage = Stage::Complete;
                Ok(true)
            }
            _ => Err(CommandError::Usage(USAGE)),
        }
    }

    pub fn number(&mut self, length: Real) -> Result<bool, CommandError> {
        if self.menu.is_none() {
            match self.stage {
                Stage::Thickness { fraction, first } => {
                    return self.accept_thickness(fraction, first, length);
                }
                Stage::Radius { fraction, first } => {
                    let radius = if self.settings.diameter {
                        length * 0.5
                    } else {
                        length
                    };
                    return self.accept_size(fraction, first, radius);
                }
                _ => {}
            }
        }
        Err(CommandError::Usage(USAGE))
    }

    pub fn point(
        &mut self,
        rail: &Rail,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<bool, CommandError> {
        if self.menu.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        match self.stage {
            Stage::Radius { fraction, first } => {
                let radius = measured_radius(rail.frame(fraction)?, point)?;
                self.accept_size(fraction, first, radius)
            }
            Stage::Station => {
                let fraction = rail.fraction_at(point, tolerance)?;
                if fraction <= 0. || fraction >= 1. {
                    return Err(CommandError::Usage("Pick a point within the pipe rail"));
                }
                if self.sections.len() >= 256 {
                    return Err(CommandError::Usage(
                        "Pipe allows up to 254 intermediate stations",
                    ));
                }
                if self
                    .sections
                    .iter()
                    .any(|&(f, _)| (f - fraction).abs() <= 1e-12)
                {
                    return Err(CommandError::Usage(
                        "There is already a pipe radius at this station",
                    ));
                }
                self.stage = Stage::Radius {
                    fraction,
                    first: None,
                };
                Ok(false)
            }
            _ => Err(CommandError::Usage(USAGE)),
        }
    }

    fn accept_size(
        &mut self,
        fraction: Real,
        first: Option<Real>,
        radius: Real,
    ) -> Result<bool, CommandError> {
        check_radius(radius)?;
        if let Some(first) = first {
            if radius == first {
                return Err(CommandError::Usage("Pipe wall radii must be distinct"));
            }
            self.settings.second = radius;
            return Ok(self.accept_pair(fraction, [first, radius]));
        }
        if let Some(thickness) = self.fixed_thickness.filter(|_| !self.closed_subd()) {
            let other = radius + thickness;
            check_radius(other)?;
            if other == radius {
                return Err(CommandError::Usage("Pipe wall radii must be distinct"));
            }
            self.settings.radius = radius;
            return Ok(self.accept_pair(fraction, [radius, other]));
        }
        self.settings.radius = radius;
        if self.active_thickness() {
            self.stage = if self.settings.by_thickness {
                Stage::Thickness {
                    fraction,
                    first: radius,
                }
            } else {
                Stage::Radius {
                    fraction,
                    first: Some(radius),
                }
            };
            Ok(false)
        } else {
            Ok(self.accept_pair(fraction, [radius, 0.]))
        }
    }

    fn accept_thickness(
        &mut self,
        fraction: Real,
        first: Real,
        thickness: Real,
    ) -> Result<bool, CommandError> {
        let radius = first + thickness;
        check_radius(radius)?;
        if radius == first {
            return Err(CommandError::Usage("Pipe wall radii must be distinct"));
        }
        self.settings.thickness = thickness;
        self.settings.by_thickness = true;
        self.fixed_thickness = Some(thickness);
        Ok(self.accept_pair(fraction, [first, radius]))
    }

    fn accept_pair(&mut self, fraction: Real, radii: [Real; 2]) -> bool {
        let index = self.sections.partition_point(|&(f, _)| f < fraction);
        self.sections.insert(index, (fraction, radii));
        match (self.rail_mode, fraction, self.closed) {
            (RailMode::Multiple, _, _) => {
                self.stage = Stage::Complete;
                true
            }
            (_, 0., false) => {
                self.stage = Stage::Radius {
                    fraction: 1.,
                    first: None,
                };
                false
            }
            _ => {
                self.stage = Stage::Station;
                false
            }
        }
    }

    pub fn geometries(
        &self,
        rails: &[Rail],
        use_extrusions: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Geometry>, CommandError> {
        if !matches!(self.stage, Stage::Complete) || rails.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        let a = wall_profile(&self.sections, 0, self.closed);
        let b = self
            .active_thickness()
            .then(|| wall_profile(&self.sections, 1, self.closed));
        let options = self.options();
        let mut outputs = Vec::with_capacity(rails.len());
        for rail in rails {
            outputs.extend(match self.settings.output {
                Output::Surface => build::build_walls(
                    rail.geometry(),
                    &a,
                    b.as_ref(),
                    options,
                    use_extrusions,
                    tolerance,
                )?,
                Output::SubD => {
                    subd::build_walls(rail.curve(), &a, b.as_ref(), options, tolerance)?
                }
            });
        }
        Ok(outputs)
    }
}

fn wall_profile(sections: &[(Real, [Real; 2])], wall: usize, closed: bool) -> Profile {
    let first = sections[0].1[wall];
    let end = if closed {
        first
    } else {
        sections.last().unwrap().1[wall]
    };
    let stations = sections
        .iter()
        .filter(|(fraction, _)| 0. < *fraction && *fraction < 1.)
        .map(|&(fraction, radii)| (fraction, radii[wall]))
        .collect();
    Profile::new([first, end], stations)
}

fn measured_radius(frame: Frame3, point: Point3) -> Result<Real, GeometryError> {
    let [x, y] = frame.projected_coordinates_of(point)?;
    Ok(x.hypot(y))
}

fn check_radius(radius: Real) -> Result<(), CommandError> {
    if !radius.is_finite() || radius <= 0. {
        return Err(CommandError::Usage("Pipe radius must be positive"));
    }
    Ok(())
}

fn toggle(input: Option<&str>, previous: bool) -> Result<bool, CommandError> {
    input.map_or(Ok(!previous), |value| {
        parse_yes_no(value.trim_start_matches('_')).ok_or(CommandError::Usage(USAGE))
    })
}
