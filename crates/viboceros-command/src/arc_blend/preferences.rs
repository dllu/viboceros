//! Application settings belong to the command instance, outside model history.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Preferences {
    pub radius_difference: Real,
    pub trim: bool,
    pub join: bool,
}

impl Input {
    /// Each invocation starts at ratio 1 with the primary tangent solution.
    pub fn with_preferences(
        tolerance: Tolerance,
        preferences: Preferences,
    ) -> Result<Self, CommandError> {
        if !preferences.radius_difference.is_finite() {
            return Err(CommandError::Usage(
                "ArcBlend radius difference must be finite",
            ));
        }
        let mut input = Self::new(tolerance);
        input.last_radius_difference = preferences.radius_difference;
        input.trim = preferences.trim;
        input.join = preferences.join;
        Ok(input)
    }

    pub fn preferences(&self) -> Preferences {
        Preferences {
            radius_difference: self.last_radius_difference,
            trim: self.trim,
            join: self.join,
        }
    }
}
