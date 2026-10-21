//! Stage-specific options and validated preference updates.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pending {
    Around,
    Vertical,
    Solid,
    Cap,
    Direction,
    Both,
    BaseAngle,
    ApexAngle,
}
impl Pending {
    pub(super) fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "aroundfaces" => Some(Self::Around),
            "verticalfaces" => Some(Self::Vertical),
            "solid" => Some(Self::Solid),
            "capfacestyle" => Some(Self::Cap),
            "directionconstraint" => Some(Self::Direction),
            "bothsides" => Some(Self::Both),
            "orbaseangle" => Some(Self::BaseAngle),
            "orapexangle" => Some(Self::ApexAngle),
            _ => None,
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Around => "AroundFaces",
            Self::Vertical => "VerticalFaces",
            Self::Solid => "Solid",
            Self::Cap => "CapFaceStyle",
            Self::Direction => "DirectionConstraint",
            Self::Both => "BothSides",
            Self::BaseAngle => "OrBaseAngle",
            Self::ApexAngle => "OrApexAngle",
        }
    }
}

impl Input {
    fn setting(&mut self, key: Pending, value: &str) -> Result<(), CommandError> {
        let value = value.trim_start_matches('_');
        let mut p = self.preferences;
        let yes_no = || {
            if value.eq_ignore_ascii_case("Yes") {
                Ok(true)
            } else if value.eq_ignore_ascii_case("No") {
                Ok(false)
            } else {
                Err(CommandError::Usage(self.kind.usage()))
            }
        };
        match key {
            Pending::Around => {
                p.around_faces = value
                    .parse()
                    .map_err(|_| CommandError::Usage(self.kind.usage()))?;
                if !(3..=100_000).contains(&p.around_faces) {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
            }
            Pending::Vertical => {
                p.vertical_faces = value
                    .parse()
                    .map_err(|_| CommandError::Usage(self.kind.usage()))?;
                if !(2..=100_000).contains(&p.vertical_faces) {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
            }
            Pending::Solid => p.solid = yes_no()?,
            Pending::Both => p.both_sides = yes_no()?,
            Pending::Cap => {
                p.cap = if value.eq_ignore_ascii_case("Tri") {
                    SubDCylinderCap::Triangles
                } else if value.eq_ignore_ascii_case("Quad") {
                    SubDCylinderCap::Quads
                } else {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
            }
            Pending::BaseAngle | Pending::ApexAngle => {
                let degrees = crate::parse_finite_real(value)?;
                let base = key == Pending::BaseAngle;
                let bound = if base { 90. } else { 180. };
                if degrees == 0. || degrees.abs() >= bound {
                    return Err(CommandError::Usage(self.kind.usage()));
                }
                if base {
                    p.base_angle = degrees;
                } else {
                    p.apex_angle = degrees;
                }
                let radius = self
                    .circle
                    .ok_or(CommandError::Usage(self.kind.usage()))?
                    .radius;
                let angle = (if base { degrees } else { degrees * 0.5 }).to_radians();
                let height = if base {
                    radius * angle.tan()
                } else {
                    radius / angle.tan()
                };
                self.preferences = p;
                self.pending = None;
                return self.end(height, self.cursor);
            }
            Pending::Direction => {
                p.direction = if value.eq_ignore_ascii_case("Vertical") {
                    Direction::Vertical
                } else if value.eq_ignore_ascii_case("None") {
                    Direction::None
                } else {
                    return Err(CommandError::Usage(self.kind.usage()));
                };
                p.both_sides = false;
            }
        }
        self.preferences = p;
        self.pending = None;
        Ok(())
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        if let Some(key) = self.pending {
            self.setting(key, word)?;
            return Ok(true);
        }
        let word = word.trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if let Some(end) = self.end_circle.as_mut() {
            if end.option(name) {
                self.preferences.diameter = end.diameter();
                if let Some(value) = value {
                    self.number(crate::parse_finite_real(value)?)?;
                }
                return Ok(true);
            }
            if Pending::parse(name).is_some() || CircleSizeMode::parse(name).is_some() {
                return Err(CommandError::Usage(self.kind.usage()));
            }
            return Ok(false);
        }
        if let Some(key) = Pending::parse(name) {
            let allowed = match key {
                Pending::Direction => self.kind != Kind::TruncatedCone || self.at_start(),
                Pending::Both => self.kind == Kind::Cylinder && self.circle.is_some(),
                Pending::BaseAngle | Pending::ApexAngle => {
                    self.kind == Kind::Cone && self.circle.is_some() && self.allow_angles
                }
                _ => self.at_start(),
            };
            if !allowed {
                return Err(CommandError::Usage(self.kind.usage()));
            }
            if let Some(value) = value {
                self.setting(key, value)?
            } else {
                self.pending = Some(key)
            }
            return Ok(true);
        }
        if self.circle.is_none() {
            if ["Vertical", "Orientation"]
                .iter()
                .any(|m| name.eq_ignore_ascii_case(m))
            {
                return Err(CommandError::Usage(
                    "These SubD primitives do not use Circle's Vertical or Orientation modes",
                ));
            }
            if self.getter.option(name) {
                if let Some(value) = value {
                    self.number(crate::parse_finite_real(value)?)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}
