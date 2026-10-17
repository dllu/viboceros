//! Read-only tangent-circle construction shared by CLI and interactive input.
use crate::{CommandError, circle_around_input::Preferences, curve_reference::CapturedCurve};
use viboceros_document::{Document, ObjectId};
use viboceros_geometry::{
    Circle3, CircleTangentConstraint as Constraint, Frame3, GeometryError, Point3, Tolerance,
    try_circle_tangent,
};

#[derive(Clone, Debug)]
enum Pick {
    Point(Point3),
    Curve {
        source: CapturedCurve,
        parameter: f64,
        pick: Point3,
    },
}
#[derive(Clone, Debug)]
pub struct Input {
    picks: Vec<Pick>,
    point: bool,
    from_first: bool,
    radius: Option<f64>,
    radius_pending: bool,
    plane: Frame3,
    tolerance: Tolerance,
    preferences: Preferences,
}
impl Input {
    pub fn new(plane: Frame3, tolerance: Tolerance, preferences: Preferences) -> Self {
        Self {
            picks: Vec::new(),
            point: false,
            from_first: false,
            radius: None,
            radius_pending: false,
            plane,
            tolerance,
            preferences,
        }
    }
    pub fn wants_curve(&self) -> bool {
        !self.point && !self.radius_pending
    }
    pub fn wants_point(&self) -> bool {
        self.point
    }
    pub fn wants_number(&self) -> bool {
        self.radius_pending || self.picks.len() == 1
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.picks.last().map(|p| match p {
            Pick::Point(p) => *p,
            Pick::Curve { pick, .. } => *pick,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.picks.iter().all(|p| match p {
            Pick::Point(_) => true,
            Pick::Curve { source, .. } => source.current(doc),
        })
    }
    pub fn reset_sources(&mut self, tolerance: Tolerance) {
        self.tolerance = tolerance;
        self.picks.clear();
        self.point = false;
        self.from_first = false;
        self.radius_pending = false;
    }
    pub fn prompt(&self) -> String {
        if self.radius_pending {
            return format!("Radius <{}>", self.preferences.radius);
        }
        let stage = match self.picks.len() {
            0 => "First",
            1 => "Second",
            _ => "Third",
        };
        let mut prompt = format!(
            "{stage} {}",
            if self.point { "point" } else { "tangent curve" }
        );
        prompt.push_str("; Point");
        if self.picks.len() == 1 {
            prompt.push_str("; radius");
            if matches!(self.picks[0], Pick::Curve { .. }) && !self.from_first {
                prompt.push_str("; FromFirstPoint");
            }
        }
        if self.picks.len() == 2 {
            prompt.push_str("; Enter accepts two contacts");
            if matches!(
                self.picks.as_slice(),
                [Pick::Curve { .. }, Pick::Curve { .. }]
            ) {
                prompt.push_str("; Radius");
            }
        }
        prompt
    }

    pub fn option(&mut self, word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Point") && !self.radius_pending {
            self.point = true;
            return true;
        }
        if word.eq_ignore_ascii_case("FromFirstPoint")
            && matches!(self.picks.as_slice(), [Pick::Curve { .. }])
        {
            self.from_first = true;
            return true;
        }
        if word.eq_ignore_ascii_case("Radius")
            && matches!(
                self.picks.as_slice(),
                [Pick::Curve { .. }, Pick::Curve { .. }]
            )
        {
            self.radius_pending = true;
            return true;
        }
        false
    }
    fn should_finish(&self) -> bool {
        self.picks.len() == 3
            || (self.picks.len() == 2 && (self.radius.is_some() || self.from_first))
    }
    pub fn point(&mut self, p: Point3) -> Result<Option<Circle3>, CommandError> {
        if !self.point || self.picks.len() >= 3 {
            return Err(CommandError::Usage(
                "Choose Point before entering a tangent-circle point",
            ));
        }
        let mut next = self.clone();
        next.picks.push(Pick::Point(p));
        next.point = false;
        let result = if next.should_finish() {
            Some(next.finish()?)
        } else {
            None
        };
        *self = next;
        Ok(result)
    }
    pub fn curve(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Point3,
    ) -> Result<Option<Circle3>, CommandError> {
        if !self.wants_curve() || self.picks.len() >= 3 {
            return Err(CommandError::Usage(
                "Tangent Circle is not selecting a curve",
            ));
        }
        let source = CapturedCurve::capture(doc, id, edge)?;
        let parameter = source
            .curve_ref()
            .closest_parameter(pick, doc.tolerance())?;
        let mut next = self.clone();
        next.picks.push(Pick::Curve {
            source,
            parameter,
            pick,
        });
        let result = if next.should_finish() {
            Some(next.finish()?)
        } else {
            None
        };
        *self = next;
        Ok(result)
    }
    pub fn number(&mut self, r: f64) -> Result<Option<Circle3>, CommandError> {
        if !self.wants_number() || !r.is_finite() || r <= self.tolerance.absolute() {
            return Err(CommandError::Usage(
                "Enter a positive Circle tangent radius",
            ));
        }
        let mut next = self.clone();
        next.radius = Some(r);
        next.radius_pending = false;
        let result = if next.picks.len() == 2 {
            Some(next.finish()?)
        } else {
            None
        };
        *self = next;
        Ok(result)
    }
    pub fn accepts_default(&self) -> bool {
        self.radius_pending || self.picks.len() == 2
    }
    pub fn enter(&mut self) -> Result<Option<Circle3>, CommandError> {
        if self.radius_pending {
            self.default_number()
        } else if self.picks.len() == 2 {
            self.finish().map(Some)
        } else {
            Ok(None)
        }
    }
    pub fn default_number(&mut self) -> Result<Option<Circle3>, CommandError> {
        self.number(self.preferences.radius)
    }
    pub fn finish(&self) -> Result<Circle3, CommandError> {
        if let ([Pick::Point(a), Pick::Point(b)], Some(radius)) =
            (self.picks.as_slice(), self.radius)
        {
            // In the native radius-locked Point getter, the second point
            // supplies the center direction rather than a second contact.
            let frame = Circle3::try_from_diameter_on_plane(*a, *b, self.plane, self.tolerance)?;
            let center = a.translated(
                a.vector_to(*b)?
                    .normalized_nonzero()?
                    .as_vector()
                    .scaled(radius)?,
            )?;
            return Ok(Circle3::try_from_center_point(
                center,
                *a,
                frame.normal()?,
                self.tolerance,
            )?);
        }
        let constraints = self
            .picks
            .iter()
            .enumerate()
            .map(|(i, p)| match p {
                Pick::Point(p) => Constraint::Point(*p),
                Pick::Curve {
                    source, parameter, ..
                } => Constraint::Curve {
                    curve: source.curve_ref(),
                    parameter: *parameter,
                    fixed: i == 0 && self.from_first,
                },
            })
            .collect::<Vec<_>>();
        try_circle_tangent(&constraints, self.radius, self.plane, self.tolerance)?
            .map(|s| s.circle)
            .ok_or_else(|| {
                GeometryError::Degenerate {
                    context: "No tangent circle satisfies these picks",
                }
                .into()
            })
    }
    pub fn preview_current(&self) -> Option<Circle3> {
        self.finish().ok()
    }
    pub fn preview_curve(
        &self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        p: Point3,
    ) -> Option<Circle3> {
        if !self.wants_curve() || !self.current(doc) {
            return None;
        }
        let mut next = self.clone();
        next.curve(doc, id, edge, p).ok()?;
        next.finish().ok()
    }
    pub fn preview_point(&self, p: Point3) -> Option<Circle3> {
        let mut next = self.clone();
        if !next.point {
            return None;
        }
        next.picks.push(Pick::Point(p));
        next.finish().ok()
    }
    pub fn command(&self) -> String {
        let mut tokens = vec!["Circle Tangent".to_owned()];
        let point = |p: Point3| {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        };
        for (i, p) in self.picks.iter().enumerate() {
            match p {
                Pick::Point(p) => {
                    tokens.push("Point".into());
                    tokens.push(point(*p));
                }
                Pick::Curve { source, pick, .. } => {
                    tokens.push(source.source().1.map_or_else(
                        || format!("Curve={}", source.object_id()),
                        |e| format!("Edge={},{}", source.object_id(), e),
                    ));
                    tokens.push(point(*pick));
                }
            }
            if i == 0 {
                if self.from_first {
                    tokens.push("FromFirstPoint".into());
                }
                if let Some(r) = self.radius {
                    tokens.push(r.to_string());
                }
            }
        }
        if self.picks.len() == 2 && self.radius.is_none() && !self.from_first {
            tokens.push("Enter".into());
        }
        tokens.join(" ")
    }
    pub fn preferences(&self, circle: Circle3) -> Preferences {
        Preferences {
            radius: circle.radius(),
            ..self.preferences
        }
    }
}
pub struct Definition {
    pub circle: Circle3,
    pub preferences: Preferences,
}
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    preferences: Preferences,
) -> Result<Definition, CommandError> {
    let mut input = Input::new(plane, doc.tolerance(), preferences);
    let mut i = 0;
    let mut result = None;
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage("Trailing Circle Tangent input"));
        }
        let word = args[i].trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Enter") {
            result = Some(input.finish()?);
            i += 1;
            continue;
        }
        if input.option(word) {
            i += 1;
            continue;
        }
        if let Some((key, value)) = word.split_once('=') {
            if key.eq_ignore_ascii_case("Curve") || key.eq_ignore_ascii_case("Edge") {
                let (id, edge) = if key.eq_ignore_ascii_case("Curve") {
                    (
                        value.parse().map_err(|_| CommandError::Usage("Curve=id"))?,
                        None,
                    )
                } else {
                    let (id, e) = crate::curve_reference::parse_edge(value)
                        .ok_or(CommandError::Usage("Edge=id,index"))?;
                    (id, Some(e))
                };
                let (p, count) = crate::parse_point(&args[i + 1..])?;
                result = input.curve(doc, id, edge, p)?;
                i += 1 + count;
                continue;
            }
            if key.eq_ignore_ascii_case("Radius") {
                result = input.number(crate::parse_finite_real(value)?)?;
                i += 1;
                continue;
            }
        }
        if input.wants_point() && (word.contains(',') || args.len() - i >= 3) {
            let (p, count) = crate::parse_point(&args[i..])?;
            result = input.point(p)?;
            i += count;
        } else {
            result = input.number(crate::parse_finite_real(word)?)?;
            i += 1;
        }
    }
    let circle = match result {
        Some(c) => c,
        None => input.finish()?,
    };
    Ok(Definition {
        circle,
        preferences: input.preferences(circle),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_document::Geometry;
    #[test]
    fn circle_tangent_failed_commands_preserve_model_preferences_and_history() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let line = |a, b| {
            Geometry::Line(
                viboceros_geometry::LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap(),
            )
        };
        let a = doc.add_geometry(line(p(0., 0.), p(12., 0.))).unwrap();
        let b = doc.add_geometry(line(p(0., 6.), p(12., 6.))).unwrap();
        doc.clear_history().unwrap();
        let before = format!("{doc:?}");
        let preferences = registry.circle_preferences();
        for command in [
            format!("Circle Tangent Curve={a} 3,0,0 2 Curve={b} 4,6,0"),
            format!("Circle Tangent Curve={a} 3,0,0 -2 Curve={b} 4,6,0"),
            format!("Circle Tangent Curve={a} 3,0,0 FromFirstPoint Point 3,0,0"),
            format!("Circle Tangent Edge={a},0 3,0,0 Point 0,4,0 Enter"),
            "Circle Tangent Point 3,0,0 Point 0,4,0 Enter trailing".into(),
        ] {
            assert!(registry.execute(&mut doc, &command).is_err(), "{command}");
            assert_eq!(format!("{doc:?}"), before, "{command}");
            assert_eq!(registry.circle_preferences(), preferences);
            assert!(doc.undo().unwrap().is_none());
        }
    }
}
