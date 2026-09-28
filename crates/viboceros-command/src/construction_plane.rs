//! Construction-plane commands and viewport-local history, independent of cameras
//! and document undo. A parsed edit is fully validated before it can be applied.
use std::collections::VecDeque;
use viboceros_drafting::{PointInput, PointInputError};
use viboceros_geometry::{AffineTransform3, Frame3, GeometryError, Point3, Tolerance, Vector3};

const HISTORY_LIMIT: usize = 50;
pub const USAGE: &str = "CPlane [point | All[=Yes|No] point | View | World Top|Bottom|Front|Back|Right|Left | 3Point origin (x-point y-point | Vertical x-point | ZAxis z-point) | Elevation distance | Through [All[=Yes|No]] point | Rotate axis-start axis-end (degrees | reference-point target-point) | Undo | Redo]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldPlane {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
}

impl WorldPlane {
    pub const ALL: [Self; 6] = [
        Self::Top,
        Self::Bottom,
        Self::Front,
        Self::Back,
        Self::Right,
        Self::Left,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Front => "Front",
            Self::Back => "Back",
            Self::Right => "Right",
            Self::Left => "Left",
        }
    }
    pub fn frame(self) -> Frame3 {
        let (x, y) = match self {
            Self::Top => ([1., 0., 0.], [0., 1., 0.]),
            Self::Bottom => ([1., 0., 0.], [0., -1., 0.]),
            Self::Front => ([1., 0., 0.], [0., 0., 1.]),
            Self::Back => ([-1., 0., 0.], [0., 0., 1.]),
            Self::Right => ([0., 1., 0.], [0., 0., 1.]),
            Self::Left => ([0., -1., 0.], [0., 0., 1.]),
        };
        Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_from(x).unwrap(),
            Vector3::try_from(y).unwrap(),
            Tolerance::DEFAULT,
        )
        .expect("orthonormal world frame")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanePromptKind {
    Origin,
    AllOrigin,
    ThreePoint,
    ThreePointVertical,
    ThreePointZAxis,
    Elevation,
    Through,
    ThroughAll,
    Rotate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneAction {
    Set(Frame3),
    SetAllOrigin(Point3),
    SetThroughAll(Point3),
    AlignToView,
    Undo,
    Redo,
    Prompt(PlanePromptKind),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlaneOptions {
    pub origin_all: bool,
    pub through_all: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedPlaneCommand {
    pub options: PlaneOptions,
    pub action: Result<PlaneAction, PlaneCommandError>,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum PlaneCommandError {
    #[error("Usage: {USAGE}")]
    Usage,
    #[error("CPlane requires a finite elevation or angle")]
    Number,
    #[error(transparent)]
    Point(#[from] PointInputError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

fn keyword(input: &str, expected: &str) -> bool {
    input.trim_start_matches('_').eq_ignore_ascii_case(expected)
}

#[derive(Clone, Copy)]
enum AllOption {
    Toggle,
    Set(bool),
}

impl AllOption {
    fn apply(self, current: bool) -> bool {
        match self {
            Self::Toggle => !current,
            Self::Set(value) => value,
        }
    }
}

fn all_option(input: &str) -> Option<AllOption> {
    if keyword(input, "All") {
        return Some(AllOption::Toggle);
    }
    let (name, value) = input.split_once('=')?;
    if !keyword(name, "All") {
        return None;
    }
    if keyword(value, "Yes") {
        Some(AllOption::Set(true))
    } else if keyword(value, "No") {
        Some(AllOption::Set(false))
    } else {
        None
    }
}

pub fn parse(
    input: &str,
    plane: Frame3,
    previous: Option<Point3>,
    tolerance: Tolerance,
) -> Option<Result<PlaneAction, PlaneCommandError>> {
    parse_with_options(input, plane, previous, tolerance, PlaneOptions::default())
        .map(|parsed| parsed.action)
}

pub fn parse_with_options(
    input: &str,
    plane: Frame3,
    previous: Option<Point3>,
    tolerance: Tolerance,
    options: PlaneOptions,
) -> Option<ParsedPlaneCommand> {
    let mut tokens = input.split_whitespace();
    if !tokens
        .next()?
        .trim_start_matches(['\'', '_', '-'])
        .eq_ignore_ascii_case("CPlane")
    {
        return None;
    }
    let args = tokens.collect::<Vec<_>>();
    let mut options = options;
    if let Some(option) = args.first().and_then(|name| all_option(name)) {
        options.origin_all = option.apply(options.origin_all);
    } else if args.first().is_some_and(|name| keyword(name, "Through"))
        && let Some(option) = args.get(1).and_then(|name| all_option(name))
    {
        options.through_all = option.apply(options.through_all);
    }
    Some(ParsedPlaneCommand {
        options,
        action: parse_arguments(&args, plane, previous, tolerance, options),
    })
}

fn parse_arguments(
    args: &[&str],
    plane: Frame3,
    mut previous: Option<Point3>,
    tolerance: Tolerance,
    options: PlaneOptions,
) -> Result<PlaneAction, PlaneCommandError> {
    let mut point = |text: &str| {
        let parsed = PointInput::parse(text).ok_or(PlaneCommandError::Usage)??;
        let p = parsed.resolve(plane, previous)?;
        previous = Some(p);
        Ok::<_, PlaneCommandError>(p)
    };
    let number = |text: &str| {
        text.parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .ok_or(PlaneCommandError::Number)
    };
    Ok(match args {
        [] => PlaneAction::Prompt(if options.origin_all {
            PlanePromptKind::AllOrigin
        } else {
            PlanePromptKind::Origin
        }),
        [name] if keyword(name, "Undo") => PlaneAction::Undo,
        [name] if keyword(name, "Redo") => PlaneAction::Redo,
        [name] if keyword(name, "View") => PlaneAction::AlignToView,
        [name] if keyword(name, "3Point") => PlaneAction::Prompt(PlanePromptKind::ThreePoint),
        [name] if keyword(name, "Elevation") => PlaneAction::Prompt(PlanePromptKind::Elevation),
        [name] if keyword(name, "Through") => PlaneAction::Prompt(if options.through_all {
            PlanePromptKind::ThroughAll
        } else {
            PlanePromptKind::Through
        }),
        [name] if all_option(name).is_some() => PlaneAction::Prompt(if options.origin_all {
            PlanePromptKind::AllOrigin
        } else {
            PlanePromptKind::Origin
        }),
        [name, option] if keyword(name, "Through") && all_option(option).is_some() => {
            PlaneAction::Prompt(if options.through_all {
                PlanePromptKind::ThroughAll
            } else {
                PlanePromptKind::Through
            })
        }
        [name] if keyword(name, "Rotate") => PlaneAction::Prompt(PlanePromptKind::Rotate),
        [name, view] if keyword(name, "World") => PlaneAction::Set(
            WorldPlane::ALL
                .into_iter()
                .find(|p| keyword(view, p.label()))
                .ok_or(PlaneCommandError::Usage)?
                .frame(),
        ),
        [name, a, option, b] if keyword(name, "3Point") && keyword(option, "Vertical") => {
            PlaneAction::Set(vertical(plane, point(a)?, point(b)?, tolerance)?)
        }
        [name, a, option, b] if keyword(name, "3Point") && keyword(option, "ZAxis") => {
            PlaneAction::Set(three_point_z_axis(point(a)?, point(b)?, tolerance)?)
        }
        [name, a, b, c] if keyword(name, "3Point") => PlaneAction::Set(Frame3::try_from_points(
            point(a)?,
            point(b)?,
            point(c)?,
            tolerance,
        )?),
        [name, value] if keyword(name, "Elevation") => {
            PlaneAction::Set(elevated(plane, number(value)?)?)
        }
        [name, value] if keyword(name, "Through") => {
            let target = point(value)?;
            if options.through_all {
                PlaneAction::SetThroughAll(target)
            } else {
                PlaneAction::Set(through(plane, target)?)
            }
        }
        [name, value] if all_option(name).is_some() => {
            let target = point(value)?;
            if options.origin_all {
                PlaneAction::SetAllOrigin(target)
            } else {
                PlaneAction::Set(plane.with_origin(target))
            }
        }
        [name, option, value] if keyword(name, "Through") && all_option(option).is_some() => {
            let target = point(value)?;
            if options.through_all {
                PlaneAction::SetThroughAll(target)
            } else {
                PlaneAction::Set(through(plane, target)?)
            }
        }
        [name, a, b, angle] if keyword(name, "Rotate") => PlaneAction::Set(rotated(
            plane,
            point(a)?,
            point(b)?,
            number(angle)?,
            tolerance,
        )?),
        [name, a, b, reference, target] if keyword(name, "Rotate") => {
            PlaneAction::Set(rotated_by_reference_points(
                plane,
                point(a)?,
                point(b)?,
                point(reference)?,
                point(target)?,
                tolerance,
            )?)
        }
        [value] => {
            let target = point(value)?;
            if options.origin_all {
                PlaneAction::SetAllOrigin(target)
            } else {
                PlaneAction::Set(plane.with_origin(target))
            }
        }
        _ => return Err(PlaneCommandError::Usage),
    })
}

pub fn elevated(plane: Frame3, distance: f64) -> Result<Frame3, GeometryError> {
    Ok(plane.with_origin(plane.point_at([0.0, 0.0, distance])?))
}

pub fn through(plane: Frame3, point: Point3) -> Result<Frame3, GeometryError> {
    elevated(plane, plane.coordinates_of(point)?[2])
}

pub fn vertical(
    plane: Frame3,
    origin: Point3,
    x_point: Point3,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let direction = origin.vector_to(x_point)?;
    let up = plane.z_axis().as_vector();
    let height = direction.dot(up)?;
    let direction = direction.to_array();
    let up_axis = up.to_array();
    let horizontal = Vector3::try_new(
        (-height).mul_add(up_axis[0], direction[0]),
        (-height).mul_add(up_axis[1], direction[1]),
        (-height).mul_add(up_axis[2], direction[2]),
    )?;
    Frame3::try_from_directions(origin, horizontal, up, tolerance)
}

pub fn three_point_z_axis(
    origin: Point3,
    z_point: Point3,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    Frame3::try_from_normal(origin, origin.vector_to(z_point)?, tolerance)
}

pub fn rotated(
    plane: Frame3,
    start: Point3,
    end: Point3,
    degrees: f64,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    rotated_radians(plane, start, end, degrees.to_radians(), tolerance)
}

pub fn rotated_by_reference_points(
    plane: Frame3,
    start: Point3,
    end: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let axis = start.vector_to(end)?.normalized(tolerance)?;
    let frame = Frame3::try_from_normal(start, axis.as_vector(), tolerance)?;
    let projected_unit = |point: Point3| -> Result<[f64; 2], GeometryError> {
        let [x, y, _] = frame.coordinates_of(point)?;
        let vector = Vector3::try_new(x, y, 0.0)?.normalized(tolerance)?;
        let [x, y, _] = vector.as_vector().to_array();
        Ok([x, y])
    };
    let [from_x, from_y] = projected_unit(reference)?;
    let [to_x, to_y] = projected_unit(target)?;
    let sine = from_x.mul_add(to_y, -(from_y * to_x));
    let cosine = from_x.mul_add(to_x, from_y * to_y);
    rotated_radians(plane, start, end, sine.atan2(cosine), tolerance)
}

fn rotated_radians(
    plane: Frame3,
    start: Point3,
    end: Point3,
    angle: f64,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let axis = start.vector_to(end)?.normalized(tolerance)?;
    let transform = AffineTransform3::try_rotation(start, axis, angle)?;
    Frame3::try_from_directions(
        transform.transform_point(plane.origin())?,
        transform.transform_vector(plane.x_axis().as_vector())?,
        transform.transform_vector(plane.y_axis().as_vector())?,
        tolerance,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstructionPlaneState {
    frame: Frame3,
    undo: VecDeque<Frame3>,
    redo: Vec<Frame3>,
}

impl ConstructionPlaneState {
    pub fn new(frame: Frame3) -> Self {
        Self {
            frame,
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }
    pub const fn frame(&self) -> Frame3 {
        self.frame
    }
    pub fn set(&mut self, frame: Frame3) -> bool {
        // Rhino records successful CPlane edits even when the frame is
        // unchanged. Such an edit also starts a new history branch.
        let changed = frame != self.frame;
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(self.frame);
        self.redo.clear();
        self.frame = frame;
        changed
    }
    pub fn undo(&mut self) -> bool {
        let Some(frame) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push(self.frame);
        self.frame = frame;
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(frame) = self.redo.pop() else {
            return false;
        };
        self.undo.push_back(self.frame);
        self.frame = frame;
        true
    }
}

#[cfg(test)]
mod tests;
