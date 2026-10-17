//! Shared, edit-free Parabola and Parabola3Pt input and construction.
use crate::{
    Command, CommandError, ParabolicConstruction, ParabolicPositionals,
    parabolic_frame_and_dimensions, remembered::Remembered,
};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, GeometryError, NurbsCurve, Point3, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Axis,
    ThreePoint,
}
impl Kind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Axis => "Parabola",
            Self::ThreePoint => "Parabola3Pt",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim_start_matches(['_', '-']);
        [Self::Axis, Self::ThreePoint]
            .into_iter()
            .find(|kind| kind.name().eq_ignore_ascii_case(name))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Construction {
    #[default]
    Focus,
    Vertex,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Focus,
    #[default]
    ThroughPoint,
    Vertex,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickOrder {
    #[default]
    Default,
    EndsFirst,
    EndsLast,
}
/// Each command instance owns its applicable preferences, outside undo history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub construction: Construction,
    pub mode: Mode,
    pub pick_order: PickOrder,
    pub half: bool,
    pub mark_focus: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Focus,
    Vertex,
    AxisDirection,
    Start,
    End,
    ThroughPoint,
    OpeningDirection,
    Complete,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub curve: NurbsCurve,
    pub focus: Point3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    kind: Kind,
    options: Options,
    points: [Option<Point3>; 4],
    count: usize,
    plane: Frame3,
    tolerance: Tolerance,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
impl Input {
    pub fn new(kind: Kind, options: Options, plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            kind,
            options,
            points: [None; 4],
            count: 0,
            plane,
            tolerance,
        }
    }
    pub fn kind(self) -> Kind {
        self.kind
    }
    pub fn options(self) -> Options {
        self.options
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.count == 0 {
            self.plane = plane;
        }
    }
    pub fn markers(self) -> Vec<Point3> {
        self.points.into_iter().flatten().collect()
    }
    fn ordered(self) -> [usize; 3] {
        match self.options.pick_order {
            PickOrder::Default => [0, 1, 2],
            PickOrder::EndsFirst => [0, 2, 1],
            PickOrder::EndsLast => [1, 0, 2],
        }
    }
    pub fn slot(self) -> Slot {
        if self.kind == Kind::Axis {
            return match self.count {
                0 if self.options.construction == Construction::Focus => Slot::Focus,
                0 => Slot::Vertex,
                1 if self.options.construction == Construction::Focus => Slot::AxisDirection,
                1 => Slot::Focus,
                2 => Slot::End,
                _ => Slot::Complete,
            };
        }
        if self.count == 3 && self.options.mode == Mode::ThroughPoint {
            return Slot::OpeningDirection;
        }
        if self.count >= 3 {
            return Slot::Complete;
        }
        let [start, special, _] = self.ordered();
        if self.count == start {
            Slot::Start
        } else if self.count != special {
            Slot::End
        } else {
            match self.options.mode {
                Mode::Focus => Slot::Focus,
                Mode::Vertex => Slot::Vertex,
                Mode::ThroughPoint => Slot::ThroughPoint,
            }
        }
    }
    pub fn point_count(self) -> usize {
        self.count
    }
    pub fn anchor(self) -> Option<Point3> {
        self.count.checked_sub(1).and_then(|i| self.points[i])
    }
    pub fn mouse_plane(self) -> Frame3 {
        if self.slot() == Slot::OpeningDirection {
            let [s, p, e] = self.ordered().map(|i| self.points[i].unwrap());
            if let Ok(plane) = Frame3::try_from_points(p, e, s, self.tolerance) {
                return plane.with_origin(self.points[2].unwrap());
            }
        }
        self.plane
            .with_origin(self.anchor().unwrap_or(self.plane.origin()))
    }
    /// Options which change point roles are offered before the affected getter.
    pub fn option(&mut self, word: &str) -> Result<bool, GeometryError> {
        let word = word.trim_start_matches(['_', '-']);
        if self.kind == Kind::Axis && self.count == 0 {
            if word.eq_ignore_ascii_case("Focus") {
                self.options.construction = Construction::Focus;
                return Ok(true);
            }
            if word.eq_ignore_ascii_case("Vertex") {
                self.options.construction = Construction::Vertex;
                return Ok(true);
            }
        }
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v.trim_start_matches('_'))));
        let boolean = |old: bool| match value {
            None => Ok(!old),
            Some(v) if v.eq_ignore_ascii_case("Yes") => Ok(true),
            Some(v) if v.eq_ignore_ascii_case("No") => Ok(false),
            _ => Err(invalid("parabola option requires Yes or No")),
        };
        if name.eq_ignore_ascii_case("MarkFocus") {
            self.options.mark_focus = boolean(self.options.mark_focus)?;
        } else if self.kind == Kind::Axis && name.eq_ignore_ascii_case("Half") {
            self.options.half = boolean(self.options.half)?;
        } else if self.kind == Kind::ThreePoint
            && self.count < 3
            && name.eq_ignore_ascii_case("Mode")
        {
            self.options.mode = match value {
                Some(v) if v.eq_ignore_ascii_case("Focus") => Mode::Focus,
                Some(v) if v.eq_ignore_ascii_case("ThroughPoint") => Mode::ThroughPoint,
                Some(v) if v.eq_ignore_ascii_case("Vertex") => Mode::Vertex,
                _ => {
                    return Err(invalid(
                        "parabola mode requires Focus, ThroughPoint or Vertex",
                    ));
                }
            };
        } else if self.kind == Kind::ThreePoint
            && self.count == 0
            && name.eq_ignore_ascii_case("PickOrder")
        {
            self.options.pick_order = match value {
                Some(v) if v.eq_ignore_ascii_case("Default") => PickOrder::Default,
                Some(v) if v.eq_ignore_ascii_case("EndsFirst") => PickOrder::EndsFirst,
                Some(v) if v.eq_ignore_ascii_case("EndsLast") => PickOrder::EndsLast,
                _ => {
                    return Err(invalid(
                        "parabola pick order requires Default, EndsFirst or EndsLast",
                    ));
                }
            };
        } else {
            return Ok(false);
        }
        Ok(true)
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, GeometryError> {
        if self.slot() == Slot::Complete {
            return Err(invalid("parabola input is complete"));
        }
        let mut next = *self;
        next.points[next.count] = Some(point);
        next.count += 1;
        if self.kind == Kind::Axis && next.count == 2 {
            next.points[0]
                .unwrap()
                .vector_to(point)?
                .normalized(self.tolerance)?;
        }
        if self.kind == Kind::ThreePoint
            && next.count == 3
            && self.options.mode == Mode::ThroughPoint
        {
            let [s, p, e] = next.ordered().map(|i| next.points[i].unwrap());
            Frame3::try_from_points(s, p, e, self.tolerance)?;
        }
        let result = if next.slot() == Slot::Complete {
            Some(next.definition()?)
        } else {
            None
        };
        *self = next;
        Ok(result)
    }
    pub fn preview(self, cursor: Point3) -> Option<Definition> {
        let mut next = self;
        next.point(cursor).ok().flatten()
    }
    fn definition(self) -> Result<Definition, GeometryError> {
        let (curve, focus) = if self.kind == Kind::Axis {
            let positionals = ParabolicPositionals {
                construction: match self.options.construction {
                    Construction::Focus => ParabolicConstruction::Focus,
                    Construction::Vertex => ParabolicConstruction::Vertex,
                },
                first: self.points[0].unwrap(),
                second: self.points[1].unwrap(),
                end: self.points[2].unwrap(),
                option_start: 0,
            };
            let (frame, radius, height, focus, _) =
                parabolic_frame_and_dimensions(positionals, self.tolerance)?;
            (
                NurbsCurve::try_parabola(frame, radius, height, self.options.half)?,
                focus,
            )
        } else {
            let [start, special, end] = self.ordered().map(|i| self.points[i].unwrap());
            let curve = match self.options.mode {
                Mode::Focus => {
                    NurbsCurve::try_parabola_from_focus(special, start, end, self.tolerance)?
                }
                Mode::Vertex => {
                    NurbsCurve::try_parabola_from_vertex(special, start, end, self.tolerance)?
                }
                Mode::ThroughPoint => NurbsCurve::try_parabola_through_point(
                    start,
                    special,
                    end,
                    self.points[2].unwrap().vector_to(self.points[3].unwrap())?,
                    self.tolerance,
                )?,
            };
            let focus = curve.try_parabola_focus(self.tolerance)?;
            (curve, focus)
        };
        Ok(Definition { curve, focus })
    }
    pub fn command(self) -> String {
        let mut words = vec![self.kind.name().to_owned()];
        match self.kind {
            Kind::Axis => {
                words.push(format!("{:?}", self.options.construction));
                words.push(format!(
                    "Half={}",
                    if self.options.half { "Yes" } else { "No" }
                ));
            }
            Kind::ThreePoint => {
                words.push(format!("Mode={:?}", self.options.mode));
                words.push(format!("PickOrder={:?}", self.options.pick_order));
            }
        }
        words.push(format!(
            "MarkFocus={}",
            if self.options.mark_focus { "Yes" } else { "No" }
        ));
        words.extend(self.points.into_iter().flatten().map(|p| {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        }));
        words.join(" ")
    }
}

pub(super) struct ParabolaCommand {
    kind: Kind,
    options: Remembered<Options>,
}
impl ParabolaCommand {
    pub(super) fn new(kind: Kind) -> Self {
        Self {
            kind,
            options: Remembered::default(),
        }
    }
}
impl Command for ParabolaCommand {
    fn name(&self) -> &'static str {
        self.kind.name()
    }
    fn parabola_options(&self) -> Option<Options> {
        Some(self.options.get())
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let usage = match self.kind {
            Kind::Axis => crate::PARABOLA_USAGE,
            Kind::ThreePoint => crate::PARABOLA_THREE_POINT_USAGE,
        };
        let mut input = Input::new(
            self.kind,
            self.options.get(),
            crate::CommandContext::default().construction_plane,
            document.tolerance(),
        );
        let mut seen = std::collections::HashSet::new();
        let mut positionals = Vec::new();
        for word in arguments {
            let name = word
                .trim_start_matches(['_', '-'])
                .split('=')
                .next()
                .unwrap()
                .to_ascii_lowercase();
            if input.option(word)? {
                if !seen.insert(name) {
                    return Err(CommandError::Usage(usage));
                }
            } else {
                positionals.push(*word);
            }
        }
        let mut index = 0;
        let mut definition = None;
        while index < positionals.len() {
            let (point, consumed) = crate::parse_point(&positionals[index..])?;
            definition = input.point(point)?;
            index += consumed;
        }
        let definition = definition.ok_or(CommandError::Usage(usage))?;
        if input.options.mark_focus {
            document.add_geometry(Geometry::Point(definition.focus))?;
        }
        let id = document.add_geometry(Geometry::NurbsCurve(definition.curve))?;
        self.options.set(input.options);
        let suffix = if input.options.mark_focus {
            format!(" and focus point at {:?}", definition.focus.to_array())
        } else {
            String::new()
        };
        Ok(format!(
            "Added {} NURBS parabola {id}{suffix}",
            if self.kind == Kind::ThreePoint {
                "three-point"
            } else if input.options.half {
                "half"
            } else {
                "full"
            }
        ))
    }
}
