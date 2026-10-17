//! Read-only source selection and tangent extension use existing arc constructors.
use crate::{CommandError, arc_tilted_input, curve_reference::CapturedCurve};
use viboceros_document::{Document, ObjectId};
use viboceros_geometry::{
    Circle3, CircularArc3, Frame3, GeometryError, Point3, Tolerance, UnitVector3,
};

#[derive(Clone, Debug)]
pub struct Input {
    source: Option<CapturedCurve>,
    endpoint: Option<(Point3, UnitVector3)>,
    center: bool,
    sweep: Option<arc_tilted_input::Input>,
    tolerance: Tolerance,
    tokens: Vec<String>,
}
impl Input {
    pub fn new(tolerance: Tolerance) -> Self {
        Self {
            source: None,
            endpoint: None,
            center: false,
            sweep: None,
            tolerance,
            tokens: vec!["Arc".into(), "Extension".into()],
        }
    }
    pub fn source_id(&self) -> Option<ObjectId> {
        self.source.as_ref().map(CapturedCurve::object_id)
    }
    pub fn wants_curve(&self) -> bool {
        self.source.is_none()
    }
    pub fn wants_center(&self) -> bool {
        self.source.is_some() && self.center && self.sweep.is_none()
    }
    pub fn sweep(&self) -> Option<arc_tilted_input::Input> {
        self.sweep
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.sweep
            .map(|s| s.anchor())
            .or(self.endpoint.map(|e| e.0))
    }
    pub fn center_plane(&self) -> Option<Frame3> {
        let (p, t) = self.endpoint?;
        Frame3::try_from_normal(p, t.as_vector(), self.tolerance).ok()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.source.as_ref().is_none_or(|s| s.current(doc))
    }
    pub fn prompt(&self) -> &'static str {
        if self.wants_curve() {
            "pick an open curve or surface edge near its end"
        } else if self.wants_center() {
            "center of arc in the endpoint normal plane"
        } else if self.sweep.is_some() {
            "endpoint or signed angle"
        } else {
            "endpoint; Center"
        }
    }
    pub fn curve(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Point3,
    ) -> Result<(), CommandError> {
        if !self.wants_curve() {
            return Err(CommandError::Usage("Arc Extension already has a source"));
        }
        let source = CapturedCurve::capture(doc, id, edge)?;
        let curve = source.curve_ref();
        if curve.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen.into());
        }
        let start = curve.start_point()?;
        let end = curve.end_point()?;
        let at_end = pick.distance_to(end)? < pick.distance_to(start)?;
        let parameter = if at_end {
            *curve.domain().end()
        } else {
            *curve.domain().start()
        };
        let sample = curve.evaluate_with_tangent(parameter)?;
        let tangent = if at_end {
            sample.tangent()
        } else {
            sample.tangent().opposite()
        };
        self.endpoint = Some((sample.point(), tangent));
        self.source = Some(source);
        self.tokens.push(if let Some(e) = edge {
            format!("Edge={id},{e}")
        } else {
            format!("Curve={id}")
        });
        self.tokens.push(point_token(pick));
        Ok(())
    }
    pub fn option(&mut self, word: &str) -> bool {
        let word = word.trim_start_matches(['_', '-']);
        let accepted = if let Some(s) = self.sweep.as_mut() {
            word.split_once('=')
                .is_some_and(|(key, _)| key.eq_ignore_ascii_case("Direction"))
                && s.option(word)
        } else if self.source.is_some() && !self.center && word.eq_ignore_ascii_case("Center") {
            self.center = true;
            true
        } else {
            false
        };
        if accepted {
            self.tokens.push(word.into());
        }
        accepted
    }
    pub fn observe_cursor(&mut self, point: Point3) -> Result<(), GeometryError> {
        if let Some(s) = self.sweep.as_mut() {
            s.observe_cursor(point)?;
        }
        Ok(())
    }
    pub fn resolve_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        if self.wants_center() {
            let plane = self.center_plane().unwrap();
            let [x, y] = plane.projected_coordinates_of(point)?;
            plane.point_at([x, y, 0.])
        } else {
            Ok(point)
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<CircularArc3>, CommandError> {
        let (start, tangent) = self
            .endpoint
            .ok_or(CommandError::Usage("Pick an Arc Extension source first"))?;
        let point = self.resolve_point(point)?;
        let result = if let Some(s) = self.sweep.as_mut() {
            s.point(point)?
        } else if self.center {
            let center = point;
            let radius = center.distance_to(start)?;
            let radial = center.vector_to(start)?.normalized(self.tolerance)?;
            let normal = radial
                .as_vector()
                .cross(tangent.as_vector())?
                .normalized(self.tolerance)?;
            let circle = Circle3::try_from_frame(center, radius, radial, normal, self.tolerance)?;
            self.sweep = Some(arc_tilted_input::Input::from_circle(
                circle,
                self.tolerance,
            )?);
            None
        } else {
            Some(CircularArc3::try_from_start_tangent_end(
                start,
                tangent,
                point,
                self.tolerance,
            )?)
        };
        if result.is_some()
            && let Some(s) = self.sweep
        {
            self.tokens.push(format!(
                "Direction={}",
                if s.clockwise() {
                    "Clockwise"
                } else {
                    "Counterclockwise"
                }
            ));
        }
        self.tokens.push(point_token(point));
        Ok(result)
    }
    pub fn number(&mut self, number: f64) -> Result<Option<CircularArc3>, CommandError> {
        let arc = self
            .sweep
            .as_mut()
            .ok_or(CommandError::Usage(
                "Choose Center and pick its point before entering an angle",
            ))?
            .number(number)?;
        self.tokens.push(number.to_string());
        Ok(arc)
    }
    pub fn command(&self) -> String {
        self.tokens.join(" ")
    }
    pub fn preview(&self, point: Point3) -> Option<CircularArc3> {
        if self.wants_curve() {
            return None;
        }
        let mut next = self.clone();
        if self.wants_center() {
            next.point(point).ok()?;
            return next.number(90.).ok().flatten();
        }
        next.point(point).ok().flatten()
    }
    pub fn argument(
        &mut self,
        doc: &Document,
        args: &[&str],
    ) -> Result<(usize, Option<CircularArc3>), CommandError> {
        let word = args
            .first()
            .ok_or(CommandError::Usage("Incomplete Arc Extension"))?
            .trim_start_matches('_');
        if self.option(word) {
            return Ok((1, None));
        }
        if let Some((key, value)) = word.split_once('=')
            && self.wants_curve()
            && (key.eq_ignore_ascii_case("Curve") || key.eq_ignore_ascii_case("Edge"))
        {
            let (id, edge) = if key.eq_ignore_ascii_case("Edge") {
                let (id, e) = crate::curve_reference::parse_edge(value)
                    .ok_or(CommandError::Usage("Edge=id,index"))?;
                (id, Some(e))
            } else {
                (
                    value.parse().map_err(|_| CommandError::Usage("Curve=id"))?,
                    None,
                )
            };
            let (pick, used) = crate::parse_point(&args[1..])?;
            self.curve(doc, id, edge, pick)?;
            return Ok((used + 1, None));
        }
        if self.sweep.is_some() && args.len() == 1 && !word.contains(',') {
            return Ok((1, self.number(crate::parse_finite_real(word)?)?));
        }
        let (p, used) = crate::parse_point(args)?;
        Ok((used, self.point(p)?))
    }
}
fn point_token(p: Point3) -> String {
    let [x, y, z] = p.to_array();
    format!("{x},{y},{z}")
}
pub fn from_arguments(doc: &Document, args: &[&str]) -> Result<CircularArc3, CommandError> {
    let mut input = Input::new(doc.tolerance());
    let mut used = 0;
    while used < args.len() {
        let (n, arc) = input.argument(doc, &args[used..])?;
        used += n;
        if let Some(arc) = arc {
            crate::require_consumed(
                args,
                used,
                "Arc Extension Curve=id|Edge=id,index pick endpoint | Center center endpoint|angle",
            )?;
            return Ok(arc);
        }
    }
    Err(CommandError::Usage("Incomplete Arc Extension"))
}
