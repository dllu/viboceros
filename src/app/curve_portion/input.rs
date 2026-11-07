//! Typed points and unit-aware lengths share the same directed-interval rules.
use super::*;
impl Editor {
    pub(in crate::app) fn input(
        &mut self,
        text: &str,
        doc: &Document,
        plane: Frame3,
        last_point: Option<Point3>,
    ) -> Result<Action, String> {
        let word = text.trim_start_matches(['_', '-']);
        if text.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("Cancel")
        {
            return Ok(Action::Discard);
        }
        if self.reference.is_none() {
            if let Some((id, kind)) = text
                .split_whitespace()
                .next()
                .and_then(viboceros_command::line::normal::parse_binding)
            {
                let edge = match kind {
                    viboceros_command::line::normal::Component::Object => None,
                    viboceros_command::line::normal::Component::Edge(e) => Some(e),
                    _ => return Err("Select a curve or edge".into()),
                };
                return Ok(Action::Capture(id, edge));
            }
            return text
                .parse::<ObjectId>()
                .map(|id| Action::Capture(id, None))
                .map_err(|_| "Select curve to temporarily shorten".into());
        }
        let source = self.reference.as_ref().unwrap();
        let c = source.curve_ref();
        if let Some(direction) = subcurve_input::direction_option(text) {
            let locked = direction.map_err(str::to_owned)?;
            self.locked = if locked {
                let start = self
                    .start
                    .ok_or("Pick the start before locking direction")?;
                let hover = self
                    .hover
                    .ok_or("Move along the curve before locking direction")?;
                Some(subcurve_input::cursor_forward(c, start, hover).map_err(|e| e.to_string())?)
            } else {
                None
            };
            return Ok(Action::Pending);
        }
        if let Some(start) = self.start
            && let Some(quantity) =
                viboceros_drafting::PointInput::parse_length_with_units(text, doc.units())
        {
            let length = quantity.map_err(|e| e.to_string())?.abs();
            if length > c.length(doc.tolerance()).map_err(|e| e.to_string())? {
                return Err("Curve length exceeds the whole source curve".into());
            }
            if length == 0. {
                return Ok(Action::Discard);
            }
            let required = self
                .locked
                .map(|forward| {
                    subcurve_input::locked_numeric_requires_confirmation(
                        c,
                        start,
                        length,
                        forward,
                        doc.tolerance(),
                    )
                })
                .transpose()
                .map_err(|e| e.to_string())?;
            self.length = Some(length);
            if required == Some(true) {
                self.locked = None;
            }
            return if required == Some(false) {
                self.advance(start, doc.tolerance())
            } else {
                Ok(Action::Pending)
            };
        }
        let point = viboceros_drafting::PointInput::parse_with_units(text, doc.units())
            .and_then(Result::ok)
            .and_then(|p| p.resolve(plane, last_point).ok())
            .ok_or("Pick a point on the temporary source curve")?;
        let t = c
            .closest_parameter(point, doc.tolerance())
            .map_err(|e| e.to_string())?;
        self.advance(t, doc.tolerance())
    }
}
