//! Construction-plane commands and viewport-local history, independent of cameras
//! and document undo. A parsed edit is fully validated before it can be applied.
use std::collections::VecDeque;
use viboceros_document::{Geometry, ObjectId};
use viboceros_drafting::{PointInput, PointInputError};
use viboceros_geometry::{
    AffineTransform3, CurveRef, CurveSegment3, Frame3, GeometryError, LineSegment, NurbsSurface,
    Point3, PolyCurve3, Tolerance, TriangleMesh, Vector3,
};

const HISTORY_LIMIT: usize = 50;
pub const USAGE: &str = "CPlane [point | All[=Yes|No] point | View | World Top|Bottom|Front|Back|Right|Left | 3Point origin (x-point y-point | Vertical x-point | ZAxis z-point) | Elevation distance | Through [All[=Yes|No]] point | Rotate axis-start axis-end (degrees | reference-point target-point) | Object [object-id [Face=index]] | Undo | Redo]";

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
    Object,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneAction {
    Set(Frame3),
    SetAllOrigin(Point3),
    SetThroughAll(Point3),
    AlignToView,
    Object(ObjectId),
    ObjectFace(ObjectId, usize),
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
    #[error(
        "CPlane Object requires a line, polyline, polycurve, NURBS curve, conic, surface, or single-face polysurface; a mesh requires Face=index"
    )]
    UnsupportedObject,
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
        [name] if keyword(name, "Object") => PlaneAction::Prompt(PlanePromptKind::Object),
        [name, id] if keyword(name, "Object") => PlaneAction::Object(
            id.parse::<ObjectId>()
                .map_err(|_| PlaneCommandError::Usage)?,
        ),
        [name, id, face] if keyword(name, "Object") => {
            let (option, index) = face.split_once('=').ok_or(PlaneCommandError::Usage)?;
            if !keyword(option, "Face") {
                return Err(PlaneCommandError::Usage);
            }
            PlaneAction::ObjectFace(
                id.parse::<ObjectId>()
                    .map_err(|_| PlaneCommandError::Usage)?,
                index
                    .parse::<usize>()
                    .map_err(|_| PlaneCommandError::Usage)?,
            )
        }
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

fn surface_mid_frame(
    surface: &NurbsSurface,
    reversed: bool,
    tolerance: Tolerance,
) -> Result<Frame3, PlaneCommandError> {
    let u = surface.domain_u();
    let v = surface.domain_v();
    let midpoint = |a: f64, b: f64| 0.5 * a + 0.5 * b;
    let (origin, x, y) = surface.evaluate_with_derivatives(
        midpoint(*u.start(), *u.end()),
        midpoint(*v.start(), *v.end()),
    )?;
    let y = if reversed { y.scaled(-1.0)? } else { y };
    Ok(Frame3::try_from_directions(origin, x, y, tolerance)?)
}

pub fn frame_from_object(
    geometry: &Geometry,
    tolerance: Tolerance,
) -> Result<Frame3, PlaneCommandError> {
    Ok(match geometry {
        Geometry::Line(line) => line_object_frame(*line, tolerance)?,
        Geometry::Circle(circle) => Frame3::try_from_directions(
            circle.center(),
            circle.x_axis().as_vector(),
            circle.y_axis().as_vector(),
            tolerance,
        )?,
        Geometry::Arc(arc) => Frame3::try_from_directions(
            arc.center(),
            arc.x_axis().as_vector(),
            arc.y_axis().as_vector(),
            tolerance,
        )?,
        Geometry::Ellipse(ellipse) => Frame3::try_from_directions(
            ellipse.point_at_angle(0.0)?,
            ellipse.y_axis().as_vector(),
            ellipse.x_axis().as_vector().scaled(-1.0)?,
            tolerance,
        )?,
        Geometry::Polyline(_) | Geometry::NurbsCurve(_) => {
            curve_object_frame(geometry.curve_ref().unwrap(), tolerance)?
        }
        Geometry::PolyCurve(polycurve) => polycurve_object_frame(polycurve, tolerance)?,
        Geometry::NurbsSurface(surface) => surface_mid_frame(surface, false, tolerance)?,
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            surface_mid_frame(face.surface(), face.is_reversed(), tolerance)?
        }
        _ => return Err(PlaneCommandError::UnsupportedObject),
    })
}

fn line_object_frame(line: LineSegment, tolerance: Tolerance) -> Result<Frame3, PlaneCommandError> {
    // OpenNURBS ON_Line::InPlane prefers XY, YZ, then ZX before using the
    // line direction and its deterministic perpendicular.
    let direction = line.start().vector_to(line.end())?;
    let [x, y, z] = direction.to_array();
    let small = [x, y, z].map(|value| value.abs() <= tolerance.absolute());
    let (x_axis, y_axis) = if small[2] && (!small[0] || !small[1]) {
        (
            Vector3::try_new(1.0, 0.0, 0.0)?,
            Vector3::try_new(0.0, 1.0, 0.0)?,
        )
    } else if small[0] && (!small[1] || !small[2]) {
        (
            Vector3::try_new(0.0, 1.0, 0.0)?,
            Vector3::try_new(0.0, 0.0, 1.0)?,
        )
    } else if small[1] && (!small[2] || !small[0]) {
        (
            Vector3::try_new(0.0, 0.0, 1.0)?,
            Vector3::try_new(1.0, 0.0, 0.0)?,
        )
    } else {
        let perpendicular = Frame3::try_from_normal(line.start(), direction, tolerance)?;
        (direction, perpendicular.x_axis().as_vector())
    };
    Ok(Frame3::try_from_directions(
        line.start(),
        x_axis,
        y_axis,
        tolerance,
    )?)
}

fn curve_object_frame(
    curve: CurveRef<'_>,
    tolerance: Tolerance,
) -> Result<Frame3, PlaneCommandError> {
    let nurbs = curve.to_nurbs()?;
    if nurbs.is_linear(tolerance)? {
        return line_object_frame(
            LineSegment::try_new(curve.start_point()?, curve.end_point()?, tolerance)?,
            tolerance,
        );
    }
    let origin = curve.start_point()?;
    let tangent = curve
        .evaluate_with_tangent(*curve.domain().start())?
        .tangent()
        .as_vector();
    if !curve.is_planar(tolerance)? {
        let (_, _, curvature) = curve.evaluate_with_second_derivative(*curve.domain().start())?;
        let curvature_length = curvature.length()?;
        let y = if curvature_length > 0.0
            && tangent.cross(curvature)?.length()? > tolerance.angular() * curvature_length
        {
            curvature
        } else {
            Frame3::try_from_normal(origin, tangent, tolerance)?
                .x_axis()
                .as_vector()
        };
        return Ok(Frame3::try_from_directions(origin, tangent, y, tolerance)?);
    }
    let controls = nurbs.control_points();
    // ON_NurbsCurve::IsPlanar tests the largest triangle anchored at the
    // start point. Its orientation supplies Z; the start tangent supplies X.
    let stride = (controls.len() / 64).max(1);
    let mut best_area = 0.0;
    let mut normal = None;
    for first_index in (1..controls.len()).step_by(stride) {
        let first = origin.vector_to(controls[first_index].point())?;
        for second in ((first_index + stride)..controls.len()).step_by(stride) {
            let second = origin.vector_to(controls[second].point())?;
            let cross = first.cross(second)?;
            let area = cross.length()?;
            if area > best_area {
                best_area = area;
                normal = Some(cross);
            }
        }
    }
    let normal = normal.ok_or(PlaneCommandError::UnsupportedObject)?;
    Ok(Frame3::try_from_directions(
        origin,
        tangent,
        normal.cross(tangent)?,
        tolerance,
    )?)
}

fn polycurve_object_frame(
    polycurve: &PolyCurve3,
    tolerance: Tolerance,
) -> Result<Frame3, PlaneCommandError> {
    if let [segment] = polycurve.segments() {
        return match segment {
            CurveSegment3::Line(line) => line_object_frame(*line, tolerance),
            CurveSegment3::Arc(arc) => frame_from_object(&Geometry::Arc(*arc), tolerance),
            CurveSegment3::Polyline(polyline) => {
                curve_object_frame(CurveRef::Polyline(polyline), tolerance)
            }
            CurveSegment3::NurbsCurve(curve) => {
                curve_object_frame(CurveRef::NurbsCurve(curve), tolerance)
            }
        };
    }
    let curve = CurveRef::PolyCurve(polycurve);
    if curve.to_nurbs()?.is_linear(tolerance)? {
        return line_object_frame(
            LineSegment::try_new(curve.start_point()?, curve.end_point()?, tolerance)?,
            tolerance,
        );
    }
    if !curve.is_planar(tolerance)? {
        return curve_object_frame(curve, tolerance);
    }
    let origin = curve.start_point()?;
    let tangent = curve
        .evaluate_with_tangent(*curve.domain().start())?
        .tangent()
        .as_vector();
    // ON_PolyCurve::GetTestPlane probes dyadic stations in this order. The
    // first direction away from the tangent establishes the plane's sign.
    let parallel_cosine = (std::f64::consts::PI / 180.0).cos();
    let mut best_parallel = None;
    let mut best_dot = 1.0;
    for denominator in (2..=16).step_by(2) {
        for numerator in (1..denominator).step_by(2) {
            let parameter = curve.parameter_at(numerator as f64 / denominator as f64)?;
            let offset = origin.vector_to(curve.evaluate(parameter)?)?;
            let length = offset.length()?;
            if length == 0.0 {
                continue;
            }
            let dot = (tangent.dot(offset)? / length).abs();
            if dot < parallel_cosine {
                return Ok(Frame3::try_from_directions(
                    origin, tangent, offset, tolerance,
                )?);
            }
            if dot < best_dot {
                best_parallel = Some(offset);
                best_dot = dot;
            }
        }
    }
    if let Some(offset) = best_parallel {
        return Ok(Frame3::try_from_directions(
            origin, tangent, offset, tolerance,
        )?);
    }
    for segment in polycurve.segments().iter().skip(1) {
        let domain = segment.domain();
        let midpoint = 0.5 * *domain.start() + 0.5 * *domain.end();
        let offset = origin.vector_to(segment.evaluate(midpoint)?)?;
        if offset.length()? > 0.0
            && tangent.cross(offset)?.length()? > tolerance.angular() * offset.length()?
        {
            return Ok(Frame3::try_from_directions(
                origin, tangent, offset, tolerance,
            )?);
        }
    }
    Err(PlaneCommandError::UnsupportedObject)
}

pub fn frame_from_mesh_face(
    mesh: &TriangleMesh,
    face_index: usize,
    tolerance: Tolerance,
) -> Result<Frame3, PlaneCommandError> {
    let face = mesh
        .faces()
        .get(face_index)
        .ok_or(GeometryError::MeshFaceIndexOutOfRange {
            face: face_index,
            face_count: mesh.face_count(),
        })?;
    let mut center = [0.0; 3];
    let divisor = face.vertex_count() as f64;
    for &index in face.indices() {
        let point = mesh.vertices()[index as usize].to_array();
        for (coordinate, value) in center.iter_mut().zip(point) {
            *coordinate += value / divisor;
        }
    }
    // Rhino's captured mesh-face CPlanes use the unit face normal after it has
    // been stored as f32, then normalize that stored direction for the frame.
    let normal = mesh.polygon_face_normal(face_index)?.as_vector().to_array();
    let stored_normal = Vector3::try_from(normal.map(|component| (component as f32) as f64))?;
    Ok(Frame3::try_from_normal(
        Point3::try_from(center)?,
        stored_normal,
        tolerance,
    )?)
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
