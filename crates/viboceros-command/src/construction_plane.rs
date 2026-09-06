//! Construction-plane commands and viewport-local history, independent of cameras
//! and document undo. A parsed edit is fully validated before it can be applied.
use std::collections::VecDeque;
use viboceros_drafting::{PointInput, PointInputError};
use viboceros_geometry::{AffineTransform3, Frame3, GeometryError, Point3, Tolerance, Vector3};

const HISTORY_LIMIT: usize = 50;
pub const USAGE: &str = "CPlane [point | World Top|Bottom|Front|Back|Right|Left | 3Point origin x-point y-point | Elevation distance | Through point | Rotate axis-start axis-end degrees | Undo | Redo]";

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
    ThreePoint,
    Elevation,
    Through,
    Rotate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneAction {
    Set(Frame3),
    Undo,
    Redo,
    Prompt(PlanePromptKind),
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

pub fn parse(
    input: &str,
    plane: Frame3,
    previous: Option<Point3>,
    tolerance: Tolerance,
) -> Option<Result<PlaneAction, PlaneCommandError>> {
    let mut tokens = input.split_whitespace();
    if !tokens
        .next()?
        .trim_start_matches(['\'', '_', '-'])
        .eq_ignore_ascii_case("CPlane")
    {
        return None;
    }
    Some(parse_arguments(
        &tokens.collect::<Vec<_>>(),
        plane,
        previous,
        tolerance,
    ))
}

fn parse_arguments(
    args: &[&str],
    plane: Frame3,
    mut previous: Option<Point3>,
    tolerance: Tolerance,
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
        [] => PlaneAction::Prompt(PlanePromptKind::Origin),
        [name] if keyword(name, "Undo") => PlaneAction::Undo,
        [name] if keyword(name, "Redo") => PlaneAction::Redo,
        [name] if keyword(name, "3Point") => PlaneAction::Prompt(PlanePromptKind::ThreePoint),
        [name] if keyword(name, "Elevation") => PlaneAction::Prompt(PlanePromptKind::Elevation),
        [name] if keyword(name, "Through") => PlaneAction::Prompt(PlanePromptKind::Through),
        [name] if keyword(name, "Rotate") => PlaneAction::Prompt(PlanePromptKind::Rotate),
        [name, view] if keyword(name, "World") => PlaneAction::Set(
            WorldPlane::ALL
                .into_iter()
                .find(|p| keyword(view, p.label()))
                .ok_or(PlaneCommandError::Usage)?
                .frame(),
        ),
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
            PlaneAction::Set(through(plane, point(value)?)?)
        }
        [name, a, b, angle] if keyword(name, "Rotate") => PlaneAction::Set(rotated(
            plane,
            point(a)?,
            point(b)?,
            number(angle)?,
            tolerance,
        )?),
        [value] => PlaneAction::Set(plane.with_origin(point(value)?)),
        _ => return Err(PlaneCommandError::Usage),
    })
}

pub fn elevated(plane: Frame3, distance: f64) -> Result<Frame3, GeometryError> {
    Ok(plane.with_origin(plane.point_at([0.0, 0.0, distance])?))
}

pub fn through(plane: Frame3, point: Point3) -> Result<Frame3, GeometryError> {
    elevated(plane, plane.coordinates_of(point)?[2])
}

pub fn rotated(
    plane: Frame3,
    start: Point3,
    end: Point3,
    degrees: f64,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let axis = start.vector_to(end)?.normalized(tolerance)?;
    let transform = AffineTransform3::try_rotation(start, axis, degrees.to_radians())?;
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
