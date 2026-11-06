use super::*;
impl Input {
    pub fn prompt(&self) -> &'static str {
        match self.stage {
            Stage::Curve if self.on_curve => "select a curve for the start point",
            Stage::Curve => "start on a curve; BothSides, Point, PointOnCurve or 2Curves",
            Stage::FreeStart => "start point; BothSides",
            Stage::CurveBase => "start point on the selected curve; BothSides",
            Stage::Endpoint if self.fixed || self.end_point => "end point or signed length",
            Stage::Endpoint if self.guides.is_empty() => "end point or curve; Point",
            Stage::Endpoint => "end point or signed length; Point or FromFirstPoint",
            Stage::FirstOfTwo => "first curve near the perpendicular contact",
            Stage::SecondOfTwo => "second curve near the perpendicular contact",
        }
    }
    pub fn option(&mut self, text: &str) -> Result<bool, CommandError> {
        let word = text.trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(a, b)| (a, Some(b)));
        if name.eq_ignore_ascii_case("BothSides")
            && matches!(
                self.stage,
                Stage::Curve | Stage::FreeStart | Stage::CurveBase
            )
        {
            self.both = match value {
                Some(v) => crate::parse_yes_no(v).ok_or(CommandError::Usage("BothSides=Yes|No"))?,
                None => true,
            };
            return Ok(true);
        }
        if self.stage == Stage::Curve {
            if name.eq_ignore_ascii_case("Point") {
                self.stage = Stage::FreeStart;
                return Ok(true);
            }
            if name.eq_ignore_ascii_case("PointOnCurve") {
                self.on_curve = true;
                self.clears_selection = true;
                return Ok(true);
            }
            if name.eq_ignore_ascii_case("2Curves") {
                self.both = false;
                self.stage = Stage::FirstOfTwo;
                self.clears_selection = true;
                return Ok(true);
            }
        }
        if self.endpoint() && !self.fixed && !self.end_point {
            if name.eq_ignore_ascii_case("FromFirstPoint") && !self.guides.is_empty() {
                self.fixed = true;
                return Ok(true);
            }
            if name.eq_ignore_ascii_case("Point") {
                self.end_point = true;
                return Ok(true);
            }
        }
        Ok(false)
    }
}
