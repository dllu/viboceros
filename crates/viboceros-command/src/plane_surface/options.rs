//! Per-registry editable basis settings; Deformable is per construction.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub degrees: [usize; 2],
    pub counts: [usize; 2],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            degrees: [3; 2],
            counts: [10; 2],
        }
    }
}
impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        if (0..2).any(|i| !(2..=11).contains(&self.degrees[i]) || self.counts[i] <= self.degrees[i])
            || self.counts[0]
                .checked_mul(self.counts[1])
                .is_none_or(|n| n > MAX_PLANE_CONTROL_POINTS)
        {
            return Err(CommandError::Usage(
                "Deformable Plane requires degrees 2–11, more controls than degree in each direction, and at most 100000 controls",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    UDegree,
    VDegree,
    UCount,
    VCount,
}
impl Setting {
    fn parse(word: &str) -> Option<Self> {
        [Self::UDegree, Self::VDegree, Self::UCount, Self::VCount]
            .into_iter()
            .find(|s| option_name_eq(word, s.name()))
    }
    fn name(self) -> &'static str {
        match self {
            Self::UDegree => "UDegree",
            Self::VDegree => "VDegree",
            Self::UCount => "UPointCount",
            Self::VCount => "VPointCount",
        }
    }
    fn current(self, p: Preferences) -> usize {
        match self {
            Self::UDegree => p.degrees[0],
            Self::VDegree => p.degrees[1],
            Self::UCount => p.counts[0],
            Self::VCount => p.counts[1],
        }
    }
    fn set(self, p: &mut Preferences, n: usize) {
        match self {
            Self::UDegree => {
                p.degrees[0] = n;
                p.counts[0] = p.counts[0].max(n.saturating_add(1));
            }
            Self::VDegree => {
                p.degrees[1] = n;
                p.counts[1] = p.counts[1].max(n.saturating_add(1));
            }
            Self::UCount => p.counts[0] = n,
            Self::VCount => p.counts[1] = n,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub deformable: bool,
    pub preferences: Preferences,
    pending: Option<Setting>,
}
impl Options {
    pub fn new(preferences: Preferences) -> Self {
        Self {
            deformable: false,
            preferences,
            pending: None,
        }
    }
    pub fn pending(self) -> bool {
        self.pending.is_some()
    }
    pub fn prompt(self) -> String {
        if let Some(s) = self.pending {
            return format!("{} <{}>", s.name(), s.current(self.preferences));
        }
        if !self.deformable {
            return "Deformable".into();
        }
        format!(
            "UDegree={} VDegree={} UPointCount={} VPointCount={}",
            self.preferences.degrees[0],
            self.preferences.degrees[1],
            self.preferences.counts[0],
            self.preferences.counts[1]
        )
    }
    /// Only call at a construction start or while an integer setting is pending.
    /// Invalid values leave both the settings and pending getter unchanged.
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim_start_matches('_');
        if let Some(setting) = self.pending {
            let n = if word.is_empty() || option_name_eq(word, "Enter") {
                setting.current(self.preferences)
            } else {
                word.parse().map_err(|_| CommandError::Usage(USAGE))?
            };
            let mut next = self.preferences;
            setting.set(&mut next, n);
            next.validate()?;
            self.preferences = next;
            self.pending = None;
            return Ok(true);
        }
        if option_name_eq(word, "Deformable") {
            self.deformable = true;
            return Ok(true);
        }
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let Some(setting) = Setting::parse(key).filter(|_| self.deformable) else {
            return Ok(false);
        };
        if let Some(value) = value {
            let mut next = self.preferences;
            setting.set(
                &mut next,
                value.parse().map_err(|_| CommandError::Usage(USAGE))?,
            );
            next.validate()?;
            self.preferences = next;
        } else {
            self.pending = Some(setting);
        }
        Ok(true)
    }
    pub fn arguments(self) -> String {
        if self.deformable {
            format!("Deformable {}", self.prompt())
        } else {
            String::new()
        }
    }
}
