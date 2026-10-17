//! Circle AroundCurve reuses the ordinary edit-free circle size getter.
use crate::{
    circle_input::{CircleInput, CircleSizeMode},
    curve_reference::CapturedCurve,
};
use viboceros_document::{Document, ObjectId};
use viboceros_geometry::{Circle3, Frame3, GeometryError, Point3, Tolerance};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub radius: f64,
    pub diameter: bool,
    pub deformation: crate::circle_deform_input::Deformation,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            radius: 1.,
            diameter: false,
            deformation: Default::default(),
        }
    }
}
impl Preferences {
    pub fn mode(self) -> CircleSizeMode {
        if self.diameter {
            CircleSizeMode::Diameter
        } else {
            CircleSizeMode::Radius
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    getter: CircleInput,
    frame: Frame3,
    tolerance: Tolerance,
    retained_mode: CircleSizeMode,
    preferences: Preferences,
}
impl Input {
    pub fn new(frame: Frame3, preferences: Preferences, tolerance: Tolerance) -> Self {
        Self {
            getter: CircleInput::at_center(frame, frame.origin())
                .with_size_mode(preferences.mode()),
            frame,
            tolerance,
            retained_mode: preferences.mode(),
            preferences,
        }
    }
    pub fn center(self) -> Point3 {
        self.frame.origin()
    }
    pub fn frame(self) -> Frame3 {
        self.frame
    }
    pub fn size_mode(self) -> CircleSizeMode {
        self.getter.size_mode()
    }
    pub fn option(&mut self, word: &str) -> bool {
        let Some(mode) = CircleSizeMode::parse(word) else {
            return false;
        };
        if !self.getter.option(word) {
            return false;
        }
        if matches!(mode, CircleSizeMode::Radius | CircleSizeMode::Diameter) {
            self.retained_mode = mode;
        }
        true
    }
    pub fn resolve_point(self, point: Point3) -> Result<Point3, GeometryError> {
        let [x, y, _] = self.frame.coordinates_of(point)?;
        self.frame.point_at([x, y, 0.])
    }
    fn circle(
        self,
        definition: Option<crate::circle_input::CircleDefinition>,
    ) -> Result<Circle3, GeometryError> {
        let d = definition.ok_or(GeometryError::Degenerate {
            context: "Circle AroundCurve requires a size",
        })?;
        Circle3::try_from_frame(
            d.frame.origin(),
            d.radius,
            d.frame.x_axis(),
            d.frame.z_axis(),
            self.tolerance,
        )
    }
    pub fn point(&mut self, point: Point3) -> Result<Circle3, GeometryError> {
        let point = self.resolve_point(point)?;
        let d = self.getter.point(point)?;
        self.circle(d)
    }
    pub fn number(&mut self, value: f64) -> Result<Circle3, GeometryError> {
        let d = self.getter.number(value)?;
        self.circle(d)
    }
    pub fn default_size(&mut self, preferences: Preferences) -> Result<Circle3, GeometryError> {
        match self.size_mode() {
            CircleSizeMode::Radius => self.number(preferences.radius),
            CircleSizeMode::Diameter => self.number(2. * preferences.radius),
            _ => Err(GeometryError::Degenerate {
                context: "this Circle size getter has no default",
            }),
        }
    }
    pub fn preview(self, point: Point3) -> Option<Circle3> {
        let mut input = self;
        input.point(point).ok()
    }
    pub fn preferences(self, circle: Circle3) -> Preferences {
        Preferences {
            radius: circle.radius(),
            diameter: self.retained_mode == CircleSizeMode::Diameter,
            ..self.preferences
        }
    }
    pub fn command(
        self,
        circle: Circle3,
        id: ObjectId,
        edge: Option<usize>,
        postselected: bool,
    ) -> Result<String, GeometryError> {
        let reference = edge.map_or_else(
            || {
                format!(
                    "{}={id}",
                    if postselected { "PickedCurve" } else { "Curve" }
                )
            },
            |i| format!("Edge={id},{i}"),
        );
        let p = |p: Point3| {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        };
        Ok(format!(
            "Circle AroundCurve {reference} {} {} {}",
            p(circle.center()),
            self.retained_mode.label(),
            p(circle.quadrants()?[0])
        ))
    }
}
pub struct Definition {
    pub circle: Circle3,
    pub source: ObjectId,
    pub edge: Option<usize>,
    pub postselected: bool,
    pub preferences: Preferences,
}
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    preferences: Preferences,
) -> Result<Definition, crate::CommandError> {
    let usage = || {
        crate::CommandError::Usage(
            "Circle AroundCurve Curve=id|Edge=id,index center radius|size-point",
        )
    };
    let (key, value) = args
        .first()
        .and_then(|s| s.split_once('='))
        .ok_or_else(usage)?;
    let (id, edge) = if key.eq_ignore_ascii_case("Curve") || key.eq_ignore_ascii_case("PickedCurve")
    {
        (value.parse().map_err(|_| usage())?, None)
    } else if key.eq_ignore_ascii_case("Edge") {
        let (id, edge) = crate::curve_reference::parse_edge(value).ok_or_else(usage)?;
        (id, Some(edge))
    } else {
        return Err(usage());
    };
    let source = CapturedCurve::capture(doc, id, edge)?;
    let (center, count) = crate::parse_point(&args[1..])?;
    let frame = crate::curve_reference::perpendicular_frame(source.curve_ref(), center)?;
    let mut input = Input::new(frame, preferences, doc.tolerance());
    let mut i = 1 + count;
    let mut result = None;
    while i < args.len() {
        if result.is_some() {
            return Err(usage());
        }
        let word = args[i];
        if input.option(word) {
            i += 1;
            continue;
        }
        if word.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
            result = Some(input.default_size(preferences)?);
            i += 1;
            continue;
        }
        if let Some((key, value)) = word.split_once('=') {
            if !input.option(key) {
                return Err(usage());
            }
            result = Some(input.number(crate::parse_finite_real(value)?)?);
            i += 1;
            continue;
        }
        if word.contains(',') || args.len() - i >= 3 {
            let (point, count) = crate::parse_point(&args[i..])?;
            result = Some(input.point(point)?);
            i += count;
        } else {
            result = Some(input.number(crate::parse_finite_real(word)?)?);
            i += 1;
        }
    }
    let circle = result.ok_or_else(usage)?;
    Ok(Definition {
        circle,
        source: id,
        edge,
        postselected: edge.is_none()
            && (key.eq_ignore_ascii_case("PickedCurve") || !doc.is_selected(id)),
        preferences: input.preferences(circle),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_document::{Geometry, SelectionMode};
    use viboceros_geometry::LineSegment;
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    #[test]
    fn circle_around_projects_pick_and_size_modes_reuse_the_pure_getter() {
        let tol = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            p(0., 0., 5.),
            viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
            tol,
        )
        .unwrap();
        let mut input = Input::new(frame, Preferences::default(), tol);
        let before = input;
        for value in [0., -3., f64::NAN, f64::INFINITY] {
            assert!(input.number(value).is_err());
            assert_eq!(input, before);
        }
        assert!(input.point(p(0., 0., 9.)).is_err());
        assert_eq!(input, before);
        let preview = input.preview(p(3., 0., 9.)).unwrap();
        assert_eq!(input, before);
        assert_eq!(input.point(p(3., 0., 9.)).unwrap(), preview);
        assert_eq!(preview.radius(), 3.);
        for (mode, value) in [
            ("Diameter", 6.),
            ("Circumference", 6. * std::f64::consts::PI),
            ("Area", 9. * std::f64::consts::PI),
        ] {
            let mut input = before;
            assert!(input.option(mode));
            let c = input.number(value).unwrap();
            assert!((c.radius() - 3.).abs() < 1e-12);
        }
        let mut diameter = before;
        diameter.option("Diameter");
        assert_eq!(diameter.point(p(0., 6., 9.)).unwrap().radius(), 6.);
        let mut circumference = before;
        circumference.option("Circumference");
        let c = circumference
            .point(p(0., 6. * std::f64::consts::PI, 9.))
            .unwrap();
        assert!((c.radius() - 3.).abs() < 1e-12);
        assert_eq!(c.quadrants().unwrap()[0], p(3., 0., 5.));
    }
    #[test]
    fn circle_around_complete_scripts_are_atomic_preserve_sources_and_share_size_preferences() {
        for preselect in [false, true] {
            let mut doc = Document::default();
            let commands = CommandRegistry::with_builtins();
            let id = doc
                .add_geometry(Geometry::Line(
                    LineSegment::try_new(p(0., 0., 0.), p(0., 0., 10.), doc.tolerance()).unwrap(),
                ))
                .unwrap();
            let source = doc.object(id).unwrap().clone();
            if preselect {
                doc.select_object(id, SelectionMode::Replace).unwrap();
            }
            doc.clear_history().unwrap();
            commands
                .execute(
                    &mut doc,
                    &format!("Circle AroundCurve Curve={id} 2,3,5 3,0,9"),
                )
                .unwrap();
            let Geometry::Circle(c) = doc.objects().find(|o| o.id() != id).unwrap().geometry()
            else {
                panic!()
            };
            assert_eq!(c.center(), p(0., 0., 5.));
            assert_eq!(c.radius(), 3.);
            for replay in [false, true] {
                assert_eq!(
                    if replay {
                        doc.redo().unwrap()
                    } else {
                        doc.undo().unwrap()
                    }
                    .as_deref(),
                    Some("Circle")
                );
                assert_eq!(doc.is_selected(id), preselect);
                assert_eq!(doc.object(id).unwrap(), &source);
                assert!(
                    source
                        .geometry_snapshot()
                        .shares_storage_with(doc.object(id).unwrap().geometry_snapshot())
                );
            }
            let saved = doc.objects().cloned().collect::<Vec<_>>();
            let preferences = commands.circle_preferences();
            for script in [
                format!("Circle AroundCurve Curve={id} 0,0,5 0"),
                format!("Circle AroundCurve Edge={id},0 0,0,5 3"),
                format!("Circle AroundCurve Curve={id} 0,0,5 3 99"),
            ] {
                assert!(commands.execute(&mut doc, &script).is_err());
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), saved);
                assert_eq!(commands.circle_preferences(), preferences);
            }
            commands
                .execute(&mut doc, "Circle 0,0,0 Diameter=8")
                .unwrap();
            assert!(commands.circle_preferences().diameter);
            commands
                .execute(&mut doc, &format!("Circle AroundCurve Curve={id} 0 0 5 4"))
                .unwrap();
            assert_eq!(commands.circle_preferences().radius, 2.);
            assert_eq!(
                CommandRegistry::with_builtins().circle_preferences(),
                Preferences::default()
            );
        }
    }
}
