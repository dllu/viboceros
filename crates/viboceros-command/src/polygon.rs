//! Polygon's point and size getters, shared by the viewport and scripts.
mod command;
pub(crate) mod rim;

pub(super) use command::PolygonCommand;

use crate::{CommandError, option_name_eq};
use viboceros_geometry::{
    Frame3, GeometryError, MAX_REGULAR_POLYGON_SIDES, Point3, Polyline3, Tolerance,
};

pub const USAGE: &str = "Polygon [NumSides=n] [Mode=Inscribed|Mode=Circumscribed|Edge|Star] [Vertical|AroundCurve Curve=id|Edge=id:index] points-or-size [Flip|Enter]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Inscribed,
    Circumscribed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    sides: usize,
    mode: Mode,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            sides: 5,
            mode: Mode::Inscribed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Getter {
    Radial,
    Edge,
    Star,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Alignment {
    Planar,
    Vertical,
    Guide,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Anchor,
    Size {
        anchor: Point3,
        distance: Option<f64>,
    },
    Inner {
        frame: Frame3,
        radius: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Input {
    plane: Frame3,
    tolerance: Tolerance,
    preferences: Preferences,
    getter: Getter,
    alignment: Alignment,
    stage: Stage,
    flipped: bool,
    pending_count: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Definition {
    frame: Frame3,
    radius: f64,
    inner: Option<f64>,
    sides: usize,
}

impl Definition {
    pub fn sides(self) -> usize {
        self.sides
    }

    pub fn polyline(self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        Polyline3::try_new(self.outline(self.sides)?, tolerance)?.try_chord_length_parameterized()
    }

    fn outline(self, limit: usize) -> Result<Vec<Point3>, GeometryError> {
        let mut points = rim::sampled(self.frame, self.radius, self.inner, self.sides, limit)?;
        points.push(points[0]);
        Ok(points)
    }
}

impl Input {
    pub fn new(plane: Frame3, preferences: Preferences, tolerance: Tolerance) -> Self {
        Self {
            plane,
            tolerance,
            preferences,
            getter: Getter::Radial,
            alignment: Alignment::Planar,
            stage: Stage::Anchor,
            flipped: false,
            pending_count: false,
        }
    }

    pub fn preferences(self) -> Preferences {
        self.preferences
    }

    pub fn sides(self) -> usize {
        self.preferences.sides
    }

    pub fn tolerance(self) -> Tolerance {
        self.tolerance
    }

    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.stage == Stage::Anchor && !self.needs_guide() {
            self.plane = plane;
        }
    }

    pub fn anchor(self) -> Option<Point3> {
        match self.stage {
            Stage::Anchor => None,
            Stage::Size { anchor, .. } => Some(anchor),
            Stage::Inner { frame, .. } => Some(frame.origin()),
        }
    }

    pub fn mouse_plane(self) -> Option<Frame3> {
        match self.stage {
            Stage::Inner { frame, .. } => Some(frame),
            Stage::Size { anchor, .. } => Some(self.plane.with_origin(anchor)),
            Stage::Anchor => None,
        }
    }

    pub fn needs_guide(self) -> bool {
        self.alignment == Alignment::Guide && self.stage == Stage::Anchor
    }

    pub fn set_guide(&mut self, frame: Frame3) {
        self.plane = frame;
        self.start_size(frame.origin());
    }

    pub fn pending_count(self) -> bool {
        self.pending_count
    }

    pub fn set_sides(&mut self, value: &str) -> Result<(), CommandError> {
        let sides = value
            .trim_start_matches('_')
            .parse()
            .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
        if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&sides) {
            return Err(GeometryError::InvalidRegularPolygonSides {
                actual: sides,
                maximum: MAX_REGULAR_POLYGON_SIDES,
            }
            .into());
        }
        self.preferences.sides = sides;
        self.pending_count = false;
        Ok(())
    }

    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        if self.pending_count {
            if word.is_empty() || option_name_eq(word, "Enter") {
                self.pending_count = false;
            } else {
                self.set_sides(word)?;
            }
            return Ok(true);
        }
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if option_name_eq(key, "NumSides") {
            if let Some(value) = value {
                self.set_sides(value)?;
            } else {
                self.pending_count = true;
            }
            return Ok(true);
        }
        if self.getter == Getter::Radial && option_name_eq(key, "Mode") {
            self.preferences.mode = match value {
                Some(word) if option_name_eq(word, "Inscribed") => Mode::Inscribed,
                Some(word) if option_name_eq(word, "Circumscribed") => Mode::Circumscribed,
                None if self.preferences.mode == Mode::Inscribed => Mode::Circumscribed,
                None => Mode::Inscribed,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            return Ok(true);
        }
        if value.is_some() {
            return Ok(false);
        }
        if self.getter == Getter::Edge && self.anchor().is_some() && option_name_eq(word, "Flip") {
            self.flipped = !self.flipped;
            return Ok(true);
        }
        if self.stage != Stage::Anchor || self.alignment != Alignment::Planar {
            return Ok(false);
        }
        let word = word.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Vertical") {
            self.alignment = Alignment::Vertical;
        } else if word.eq_ignore_ascii_case("AroundCurve") && self.getter != Getter::Edge {
            self.alignment = Alignment::Guide;
        } else if word.eq_ignore_ascii_case("Star") && self.getter == Getter::Radial {
            self.getter = Getter::Star;
        } else if word.eq_ignore_ascii_case("Edge") && self.getter == Getter::Radial {
            self.getter = Getter::Edge;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    pub fn prompt(self) -> String {
        let sides = self.sides();
        if self.pending_count {
            return format!("Number of sides <{sides}>");
        }
        let circumscribed = self.preferences.mode == Mode::Circumscribed;
        let point = match (self.stage, self.getter, circumscribed) {
            (Stage::Anchor, Getter::Edge, _) => "Start of edge",
            (Stage::Anchor, Getter::Star, _) => "Center of star",
            (Stage::Anchor, _, true) => "Center of circumscribed polygon",
            (Stage::Anchor, _, false) => "Center of inscribed polygon",
            (Stage::Size { .. }, Getter::Edge, _) => "End of edge or length",
            (Stage::Size { .. }, Getter::Star, _) => "Corner of star or outer radius",
            (Stage::Size { .. }, _, true) => "Midpoint of polygon edge or apothem",
            (Stage::Size { .. }, _, false) => "Corner of polygon or radius",
            (Stage::Inner { .. }, _, _) => "Second star radius. Press Enter for automatic",
        };
        let mut options = format!("NumSides={sides}");
        if self.getter == Getter::Radial {
            options.push_str(if circumscribed {
                " Mode=Circumscribed"
            } else {
                " Mode=Inscribed"
            });
        }
        if self.stage == Stage::Anchor && self.alignment == Alignment::Planar {
            if self.getter == Getter::Radial {
                options.push_str(" Edge Star");
            }
            options.push_str(" Vertical");
            if self.getter != Getter::Edge {
                options.push_str(" AroundCurve");
            }
        }
        if self.getter == Getter::Edge && self.anchor().is_some() {
            options.push_str(" Flip");
        }
        if self.needs_guide() {
            format!("{point} on curve ({options})")
        } else {
            format!("{point} ({options})")
        }
    }

    fn start_size(&mut self, anchor: Point3) {
        self.stage = Stage::Size {
            anchor,
            distance: None,
        };
    }

    pub fn takes_size(self) -> bool {
        !self.pending_count && self.anchor().is_some()
    }

    pub fn accepts_origin(self) -> bool {
        !matches!(self.stage, Stage::Inner { .. })
    }

    pub fn number(&mut self, value: f64) -> Result<Option<Definition>, GeometryError> {
        match self.stage {
            Stage::Inner { frame, radius } => {
                self.checked_size(value.abs())?;
                Ok(Some(self.definition(frame, radius, Some(value.abs()))))
            }
            Stage::Size { anchor, .. } => {
                // The outer getter interprets a negative entry as twice its magnitude.
                let size = if value < 0. { -2. * value } else { value };
                self.checked_size(size)?;
                self.stage = Stage::Size {
                    anchor,
                    distance: Some(size),
                };
                Ok(None)
            }
            Stage::Anchor => Ok(None),
        }
    }

    pub fn enter(&mut self, cursor: Option<Point3>) -> Result<Option<Definition>, GeometryError> {
        match self.stage {
            Stage::Inner { frame, radius } => Ok(Some(self.definition(
                frame,
                radius,
                Some(rim::automatic_inner_radius(radius, self.sides())),
            ))),
            Stage::Size {
                anchor,
                distance: Some(size),
            } => {
                let fallback = anchor.translated(self.plane.x_axis().as_vector().scaled(size)?)?;
                self.point(cursor.filter(|point| *point != anchor).unwrap_or(fallback))
            }
            _ => Ok(None),
        }
    }

    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, GeometryError> {
        match self.stage {
            Stage::Anchor => {
                self.start_size(point);
                Ok(None)
            }
            Stage::Size { anchor, distance } => {
                let (frame, radius) = self.outer_rim(anchor, point, distance)?;
                if self.getter == Getter::Star {
                    self.stage = Stage::Inner { frame, radius };
                    Ok(None)
                } else {
                    Ok(Some(self.definition(frame, radius, None)))
                }
            }
            Stage::Inner { frame, .. } => {
                let point = if self.alignment == Alignment::Guide {
                    project(frame, point)?
                } else {
                    point
                };
                self.number(frame.origin().distance_to(point)?)
            }
        }
    }

    pub fn preview(self, point: Point3, side_limit: usize) -> Option<Vec<Point3>> {
        if self.pending_count {
            return None;
        }
        let mut trial = self;
        let definition = trial.point(point).ok()?.or_else(|| {
            let Stage::Inner { frame, radius } = trial.stage else {
                return None;
            };
            Some(self.definition(
                frame,
                radius,
                Some(rim::automatic_inner_radius(radius, self.sides())),
            ))
        })?;
        definition.outline(side_limit).ok()
    }

    fn definition(self, frame: Frame3, radius: f64, inner: Option<f64>) -> Definition {
        Definition {
            frame,
            radius,
            inner,
            sides: self.sides(),
        }
    }

    fn checked_size(self, size: f64) -> Result<(), GeometryError> {
        if size.is_finite() && size > self.tolerance.absolute() {
            Ok(())
        } else {
            Err(GeometryError::Degenerate {
                context: "polygon size",
            })
        }
    }

    fn outer_rim(
        self,
        anchor: Point3,
        point: Point3,
        distance: Option<f64>,
    ) -> Result<(Frame3, f64), GeometryError> {
        let point = if self.alignment == Alignment::Guide {
            project(self.plane, point)?
        } else {
            point
        };
        let observed = anchor.distance_to(point)?;
        self.checked_size(observed)?;
        let along = anchor.direction_to(point)?;
        let size = distance.unwrap_or(observed);
        let end = anchor.translated(along.as_vector().scaled(size)?)?;
        let normal = if self.alignment == Alignment::Vertical {
            let [x, y, _] = self.plane.with_origin(anchor).coordinates_of(end)?;
            let horizontal = self.plane.with_origin(anchor).point_at([x, y, 0.])?;
            let across = anchor.direction_to(horizontal)?;
            let normal = across
                .as_vector()
                .cross(self.plane.z_axis().as_vector())?
                .normalized_nonzero()?;
            if self.getter == Getter::Edge {
                let midpoint = anchor.midpoint(end)?;
                let start = midpoint.translated(across.as_vector().scaled(-size * 0.5)?)?;
                let end = midpoint.translated(across.as_vector().scaled(size * 0.5)?)?;
                let edge =
                    Frame3::try_from_unit_x_and_normal(start, across, normal, self.tolerance)?;
                return rim::edge(edge, end, size, self.sides(), self.flipped, self.tolerance);
            }
            normal
        } else {
            self.plane.z_axis()
        };
        let frame = Frame3::try_from_unit_x_and_normal(anchor, along, normal, self.tolerance)?;
        if self.getter == Getter::Edge {
            rim::edge(frame, end, size, self.sides(), self.flipped, self.tolerance)
        } else {
            rim::radial(
                frame,
                size,
                self.sides(),
                self.getter == Getter::Radial && self.preferences.mode == Mode::Circumscribed,
                self.tolerance,
            )
        }
    }
}

fn project(plane: Frame3, point: Point3) -> Result<Point3, GeometryError> {
    let [x, y] = plane.projected_coordinates_of(point)?;
    plane.point_at([x, y, 0.])
}
