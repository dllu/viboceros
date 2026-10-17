//! Conic construction is independent of input transport and document history.
use crate::{
    Command, CommandContext, CommandError, remembered::Remembered,
    translation::DestinationConstraint,
};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{
    CurveRef, Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, UnitVector3, Vector3,
};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub rho: Option<Real>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Tangent,
    Perpendicular,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Start,
    End,
    Apex,
    Shape,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    pub controls: [Point3; 3],
    pub rho: Real,
}
impl Definition {
    pub fn curve(self) -> Result<NurbsCurve, GeometryError> {
        let [start, apex, end] = self.controls;
        NurbsCurve::try_conic(start, apex, end, self.rho)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    plane: Frame3,
    points: [Option<Point3>; 3],
    lines: [Option<DestinationConstraint>; 2],
    request: Option<Direction>,
    apex_first: bool,
    vertical: bool,
    options: Options,
    tolerance: Tolerance,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
impl Input {
    pub fn new(plane: Frame3, options: Options, tolerance: Tolerance) -> Self {
        Self {
            plane,
            points: [None; 3],
            lines: [None; 2],
            request: None,
            apex_first: false,
            vertical: false,
            options,
            tolerance,
        }
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.points[0].is_none() {
            self.plane = plane;
        }
    }
    pub fn options(self) -> Options {
        self.options
    }
    pub fn direction_request(self) -> Option<Direction> {
        self.request
    }
    pub fn slot(self) -> Slot {
        if self.points[0].is_none() {
            Slot::Start
        } else if self.apex_first && self.points[1].is_none() {
            Slot::Apex
        } else if self.points[2].is_none() {
            Slot::End
        } else if self.points[1].is_none() {
            Slot::Apex
        } else {
            Slot::Shape
        }
    }
    pub fn anchor(self) -> Option<Point3> {
        match self.slot() {
            Slot::Start => None,
            Slot::End => self.points[1].or(self.points[0]),
            Slot::Apex => self.points[2].or(self.points[0]),
            Slot::Shape => self.points[1],
        }
    }
    pub fn markers(self) -> Vec<Point3> {
        self.points.into_iter().flatten().collect()
    }
    pub fn option(&mut self, word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        if matches!(self.slot(), Slot::Start | Slot::End) && self.points[1].is_none() {
            let kind = if word.eq_ignore_ascii_case("Tangent") {
                Some(Direction::Tangent)
            } else if word.eq_ignore_ascii_case("Perpendicular") {
                Some(Direction::Perpendicular)
            } else {
                None
            };
            if let Some(kind) = kind {
                self.request = Some(kind);
                return true;
            }
        }
        if word.eq_ignore_ascii_case("Apex") && self.slot() == Slot::End && self.points[1].is_none()
        {
            self.apex_first = true;
            return true;
        }
        if word.eq_ignore_ascii_case("Vertical")
            && self.slot() == Slot::End
            && self.points[1].is_some()
        {
            self.vertical = !self.vertical;
            return true;
        }
        false
    }
    pub fn point_constraint(self) -> Option<DestinationConstraint> {
        match self.slot() {
            Slot::End if self.vertical => Some(DestinationConstraint {
                anchor: self.points[1]?,
                direction: Some(self.plane.z_axis()),
                distance: None,
            }),
            Slot::Apex => self.lines[0].or(self.lines[1]),
            _ => None,
        }
    }
    pub fn mouse_plane(self) -> Option<Frame3> {
        if self.slot() == Slot::Shape {
            let [start, apex, end] = self.triangle().ok()?;
            return Frame3::try_from_points(start, end, apex, Tolerance::NUMERICAL_VALIDATION).ok();
        }
        Some(
            self.plane
                .with_origin(self.points[0].unwrap_or(self.plane.origin())),
        )
    }
    fn triangle(self) -> Result<[Point3; 3], GeometryError> {
        let [Some(start), Some(apex), Some(end)] = self.points else {
            return Err(invalid("conic needs start, end and apex"));
        };
        Ok([start, apex, end])
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, GeometryError> {
        if self.request.is_some() {
            return Err(invalid("select a conic reference curve"));
        }
        if self.slot() == Slot::Shape {
            let [start, apex, end] = self.triangle()?;
            let curve = NurbsCurve::try_conic_through_point(
                start,
                apex,
                end,
                point,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
            let weight = curve.control_points()[1].weight();
            let rho = weight / (1. + weight);
            return self.number(rho);
        }
        let mut next = *self;
        let point = next
            .point_constraint()
            .map_or(Ok(point), |c| c.resolve(point))?;
        match next.slot() {
            Slot::Start => next.points[0] = Some(point),
            Slot::End => next.points[2] = Some(point),
            Slot::Apex => next.points[1] = Some(point),
            Slot::Shape => unreachable!(),
        }
        next.infer_apex()?;
        *self = next;
        Ok(None)
    }
    fn infer_apex(&mut self) -> Result<(), GeometryError> {
        if self.points[1].is_some() || self.points[2].is_none() {
            return Ok(());
        }
        if let [Some(first), Some(second)] = self.lines {
            let a = first.direction.unwrap().as_vector();
            let b = second.direction.unwrap().as_vector();
            let frame =
                Frame3::try_from_directions(first.anchor, a, b, Tolerance::NUMERICAL_VALIDATION)?;
            let [x, y, z] = frame.coordinates_of(second.anchor)?;
            if z.abs() > self.tolerance.absolute() {
                return Err(invalid("conic endpoint directions do not intersect"));
            }
            let dx = b.dot(frame.x_axis().as_vector())?;
            let dy = b.dot(frame.y_axis().as_vector())?;
            self.points[1] = Some(first.anchor.translated(a.scaled(x - y * dx / dy)?)?);
        }
        Ok(())
    }
    pub fn endpoint(
        &mut self,
        point: Point3,
        tangent: Vector3,
        normal: UnitVector3,
    ) -> Result<(), GeometryError> {
        let kind = self.request.ok_or(invalid("conic endpoint constraint"))?;
        let index = match self.slot() {
            Slot::Start => 0,
            Slot::End => 1,
            _ => return Err(invalid("conic endpoint constraint")),
        };
        let tangent = tangent.normalized_nonzero()?.as_vector();
        let direction = match kind {
            Direction::Tangent => Some(tangent.normalized_nonzero()?),
            Direction::Perpendicular => {
                tangent.cross(normal.as_vector())?.normalized_nonzero().ok()
            }
        };
        let mut next = *self;
        next.request = None;
        next.lines[index] = direction.map(|direction| DestinationConstraint {
            anchor: point,
            direction: Some(direction),
            distance: None,
        });
        next.point(point)?;
        *self = next;
        Ok(())
    }
    pub fn number(&mut self, rho: Real) -> Result<Option<Definition>, GeometryError> {
        if self.slot() != Slot::Shape {
            return Err(invalid("conic rho requires three controls"));
        }
        if !rho.is_finite() || rho <= 0. || rho >= 1. {
            return Err(invalid("conic rho must be between zero and one"));
        }
        let definition = Definition {
            controls: self.triangle()?,
            rho,
        };
        definition.curve()?;
        self.options.rho = Some(rho);
        Ok(Some(definition))
    }
    pub fn enter(&mut self) -> Result<Option<Definition>, GeometryError> {
        self.number(
            self.options
                .rho
                .ok_or(invalid("conic rho has no default"))?,
        )
    }
    pub fn default_curve(self) -> Option<NurbsCurve> {
        Definition {
            controls: self.triangle().ok()?,
            rho: self.options.rho.unwrap_or(0.5),
        }
        .curve()
        .ok()
    }
    pub fn preview(self, point: Point3) -> Option<NurbsCurve> {
        let mut next = self;
        if let Some(definition) = next.point(point).ok()? {
            return definition.curve().ok();
        }
        (next.slot() == Slot::Shape)
            .then(|| next.default_curve())
            .flatten()
    }
}
pub const USAGE: &str = "Conic start end apex rho-or-shape-point | Conic start Apex apex [Vertical] end rho | Conic [Tangent|Perpendicular [SubCrv] Curve=id|Edge=id,index] ... [Hover=point Direction=Locked|Free]";
#[derive(Default)]
pub(super) struct ConicCommand(Remembered<Options>);
impl Command for ConicCommand {
    fn name(&self) -> &'static str {
        "Conic"
    }
    fn conic_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_conic_options(&self, value: Options) -> bool {
        self.0.set(value);
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
        let (definition, options) =
            from_arguments(doc, args, context.construction_plane, self.0.get())?;
        let id = doc.add_geometry(Geometry::NurbsCurve(definition.curve()?))?;
        self.0.set(options);
        Ok(format!("Added exact NURBS conic {id}"))
    }
}

mod reference;
pub use reference::{ReferenceInput, ReferenceStage};
/// Replay authored inputs using the same construction state as the UI.
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    options: Options,
) -> Result<(Definition, Options), CommandError> {
    let mut input = Input::new(plane, options, doc.tolerance());
    let mut reference: Option<ReferenceInput> = None;
    let mut subcurve = false;
    let mut result = None;
    let leading = args
        .first()
        .is_some_and(|w| w.trim_start_matches('_').eq_ignore_ascii_case("Apex"));
    let explicit_default = args
        .first()
        .is_some_and(|w| w.trim_start_matches('_').eq_ignore_ascii_case("Default"));
    input.apex_first = leading;
    let mut i = usize::from(leading || explicit_default);
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let word = args[i].trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Apex")
            && i > usize::from(leading || explicit_default)
            && (leading || explicit_default)
        {
            return Err(CommandError::Usage(USAGE));
        }
        if input.option(word) {
            if input.direction_request().is_some() {
                reference = None;
            }
            i += 1;
            continue;
        }
        if word.eq_ignore_ascii_case("SubCrv")
            && input.direction_request().is_some()
            && reference.is_none()
        {
            subcurve = true;
            i += 1;
            continue;
        }
        if let Some(r) = &mut reference {
            if let Some((key, value)) = word.split_once('=')
                && key.eq_ignore_ascii_case("Hover")
            {
                let (point, _) = crate::parse_point(&[value])?;
                r.hover_point(point)?;
                i += 1;
                continue;
            }
            if let Some(locked) = crate::subcurve_input::direction_option(word) {
                r.direction(locked.map_err(CommandError::Usage)?)?;
                i += 1;
                continue;
            }
        }
        if word.eq_ignore_ascii_case("FullCurve")
            && let Some(reference) = &mut reference
        {
            reference.full_curve();
            i += 1;
            continue;
        }
        if word.eq_ignore_ascii_case("Enter") && reference.is_none() && subcurve {
            subcurve = false;
            i += 1;
            continue;
        }
        if input.direction_request().is_some() && reference.is_none() {
            let (key, value) = word.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let (id, edge) = if key.eq_ignore_ascii_case("Curve") {
                (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
            } else if key.eq_ignore_ascii_case("Edge") {
                let (id, edge) =
                    crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                (id, Some(edge))
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            reference = Some(ReferenceInput::capture(doc, id, edge, subcurve)?);
            subcurve = false;
            i += 1;
            continue;
        }
        if word.eq_ignore_ascii_case("Enter") {
            if reference
                .as_ref()
                .is_some_and(|r| r.stage() != ReferenceStage::Endpoint)
            {
                reference = None;
            } else if let Some(reference) = &mut reference {
                reference.full_curve();
            } else {
                result = input.enter()?;
            }
            i += 1;
            continue;
        }
        if reference
            .as_ref()
            .is_some_and(|r| r.stage() == ReferenceStage::TrimEnd)
            && !word.contains(',')
        {
            reference.as_mut().unwrap().length(
                word.parse().map_err(|_| CommandError::Usage(USAGE))?,
                doc.tolerance(),
            )?;
            if reference
                .as_ref()
                .is_some_and(|r| r.stage() == ReferenceStage::SelectCurve)
            {
                reference = None;
            }
            i += 1;
            continue;
        }
        if input.slot() == Slot::Shape && !word.contains(',') && args.len() - i == 1 {
            result = input.number(word.parse().map_err(|_| CommandError::Usage(USAGE))?)?;
            i += 1;
            continue;
        }
        let (point, count) = crate::parse_point(&args[i..])?;
        i += count;
        if let Some(r) = &mut reference {
            if r.stage() != ReferenceStage::Endpoint {
                r.trim_point(point, doc.tolerance())?;
                if r.stage() == ReferenceStage::SelectCurve {
                    reference = None;
                }
            } else {
                let (point, tangent) = r.sample(point)?;
                input.endpoint(point, tangent, plane.z_axis())?;
                reference = None;
            }
        } else {
            result = input.point(point)?;
        }
    }
    result
        .map(|definition| (definition, input.options()))
        .ok_or(CommandError::Usage(USAGE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn input() -> Input {
        Input::new(
            CommandContext::default().construction_plane,
            Options::default(),
            Tolerance::DEFAULT,
        )
    }
    #[test]
    fn conic_preview_is_pure_and_rho_has_no_invented_default() {
        let mut i = input();
        i.point(p(0., 0., 0.)).unwrap();
        i.point(p(10., 0., 0.)).unwrap();
        let before = i;
        let preview = i.preview(p(4., 6., 0.)).unwrap();
        assert_eq!(i, before);
        i.point(p(4., 6., 0.)).unwrap();
        assert!(i.enter().is_err());
        let before = i;
        for rho in [0., -0.1, 1., Real::NAN, Real::INFINITY] {
            assert!(i.number(rho).is_err());
            assert_eq!(i, before);
        }
        let definition = i.number(0.5).unwrap().unwrap();
        assert_eq!(definition.curve().unwrap(), preview);
        assert_eq!(i.options().rho, Some(0.5));
        assert_eq!(i.enter().unwrap(), Some(definition));
    }
    #[test]
    fn conic_constraints_use_cplane_perpendicular_lines_and_intersect_both_endpoints() {
        let mut i = input();
        let normal = i.plane.z_axis();
        i.option("Perpendicular");
        i.endpoint(p(2., 0., 0.), Vector3::try_new(1., 0., 0.).unwrap(), normal)
            .unwrap();
        i.point(p(10., 8., 7.)).unwrap();
        i.point(p(5., 3., 5.)).unwrap();
        assert_eq!(
            i.number(0.5).unwrap().unwrap().controls,
            [p(2., 0., 0.), p(2., 3., 0.), p(10., 8., 7.)]
        );
        let mut i = input();
        i.option("Tangent");
        i.endpoint(p(2., 0., 0.), Vector3::try_new(1., 0., 0.).unwrap(), normal)
            .unwrap();
        i.option("Tangent");
        i.endpoint(
            p(10., 5., 0.),
            Vector3::try_new(0., 1., 0.).unwrap(),
            normal,
        )
        .unwrap();
        assert_eq!(i.slot(), Slot::Shape);
        assert_eq!(i.number(0.5).unwrap().unwrap().controls[1], p(10., 0., 0.));
        for direction in [
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
        ] {
            let mut i = input();
            i.option("Tangent");
            i.endpoint(p(2., 0., 0.), Vector3::try_new(1., 0., 0.).unwrap(), normal)
                .unwrap();
            i.option("Tangent");
            let before = i;
            assert!(i.endpoint(p(10., 5., 2.), direction, normal).is_err());
            assert_eq!(i, before);
        }
    }
    #[test]
    fn conic_shape_points_reuse_exact_kernel_and_vertical_uses_apex_line() {
        let mut i = input();
        i.point(p(1., 2., 3.)).unwrap();
        i.option("Apex");
        i.point(p(4., 6., 8.)).unwrap();
        assert!(i.option("Vertical"));
        let before = i;
        let preview = i.preview(p(11., 9., 4.)).unwrap();
        assert_eq!(i, before);
        i.point(p(11., 9., 4.)).unwrap();
        let d = i.number(0.5).unwrap().unwrap();
        assert_eq!(d.controls[2], p(4., 6., 4.));
        assert_eq!(preview, d.curve().unwrap());
        let mut i = input();
        for a in [p(0., 0., 0.), p(10., 0., 0.), p(4., 6., 0.)] {
            i.point(a).unwrap();
        }
        let mut j = i;
        assert_eq!(
            i.point(p(4., 2., 3.)).unwrap(),
            j.point(p(4., 2., 0.)).unwrap()
        );
        let mut i = input();
        for a in [p(0., 0., 0.), p(10., 0., 0.), p(4., 6., 0.)] {
            i.point(a).unwrap();
        }
        let before = i;
        assert!(i.point(p(-2., 2., 0.)).is_err());
        assert_eq!(i, before);
    }
    #[test]
    fn conic_scripted_reference_and_failed_getters_preserve_source_selection_and_history() {
        let mut doc = Document::default();
        let commands = CommandRegistry::with_builtins();
        let guide = doc
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    p(0., 0., 0.),
                    p(10., 0., 0.),
                    doc.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let original = doc.object(guide).unwrap().geometry().clone();
        doc.clear_history().unwrap();
        for suffix in [
            "0,0,0 10,0,0 4,6,0 0",
            "Tangent Curve=99999 2,0,0 10,8,0 5,3,0 0.5",
            "0,0,0 10,0,0 4,6,0 -2,2,0",
        ] {
            let before = format!("{doc:?}");
            assert!(
                commands
                    .execute(&mut doc, &format!("Conic {suffix}"))
                    .is_err()
            );
            assert_eq!(before, format!("{doc:?}"));
            assert_eq!(doc.undo_label(), None);
        }
        commands
            .execute(
                &mut doc,
                &format!("Conic Tangent SubCrv Curve={guide} 2,0,0 6,0,0 9,2,0 10,8,0 5,3,0 0.5"),
            )
            .unwrap();
        let curve = doc
            .objects()
            .find(|o| o.id() != guide)
            .unwrap()
            .geometry()
            .nurbs_curve_representation()
            .unwrap()
            .unwrap();
        assert_eq!(curve.control_points()[0].point(), p(6., 0., 0.));
        assert_eq!(curve.control_points()[1].point(), p(5., 0., 0.));
        assert_eq!(doc.object(guide).unwrap().geometry(), &original);
        assert_eq!(doc.undo().unwrap().as_deref(), Some("Conic"));
        assert_eq!(doc.objects().count(), 1);
        assert!(doc.undo().unwrap().is_none());
        doc.redo().unwrap();
        assert_eq!(doc.objects().count(), 2);
        assert_eq!(commands.conic_options().rho, Some(0.5));
    }
}
