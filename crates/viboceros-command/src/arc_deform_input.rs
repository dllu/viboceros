//! Arc output settings stay outside document history and construction geometry.
use crate::CommandError;
use viboceros_document::Geometry;
use viboceros_geometry::{CircularArc3, NurbsCurve};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deformation {
    pub degree: usize,
    pub point_count: usize,
}
impl Default for Deformation {
    fn default() -> Self {
        Self {
            degree: 3,
            point_count: 10,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub deformable: bool,
    pub deformation: Deformation,
}
impl Output {
    pub fn curve(self, arc: CircularArc3) -> Result<NurbsCurve, CommandError> {
        if !self.deformable {
            return Ok(arc.to_nurbs()?);
        }
        let d = self.deformation;
        #[cfg(feature = "native-smlib")]
        {
            arc.try_deformable_with_rebuild::<CommandError>(
                d.degree,
                d.point_count,
                &mut |unit, p, n| {
                    Ok(viboceros_smlib::rebuild_curve(
                        viboceros_geometry::CurveRef::Arc(&unit),
                        n,
                        p,
                        true,
                        viboceros_geometry::Tolerance::NUMERICAL_VALIDATION,
                    )?)
                },
            )
        }
        #[cfg(not(feature = "native-smlib"))]
        {
            Ok(arc.try_deformable(d.degree, d.point_count)?)
        }
    }
    pub fn geometry(self, arc: CircularArc3) -> Result<Geometry, CommandError> {
        if self.deformable {
            Ok(Geometry::NurbsCurve(self.curve(arc)?))
        } else {
            Ok(Geometry::Arc(arc))
        }
    }
    pub fn command_prefix(self) -> String {
        if !self.deformable {
            return "Arc".into();
        }
        let d = self.deformation;
        if d.point_count < d.degree.saturating_add(2) {
            format!(
                "Arc Deformable Degree=2 PointCount={} Degree={}",
                d.point_count, d.degree
            )
        } else {
            format!(
                "Arc Deformable Degree={} PointCount={}",
                d.degree, d.point_count
            )
        }
    }
    pub fn wrap_command(self, arc_command: &str) -> String {
        format!(
            "{} {}",
            self.command_prefix(),
            arc_command.strip_prefix("Arc ").expect("Arc construction")
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Degree,
    PointCount,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    output: Output,
    pending: Option<Setting>,
}
impl Settings {
    pub fn new(deformation: Deformation) -> Self {
        Self {
            output: Output {
                deformable: false,
                deformation,
            },
            pending: None,
        }
    }
    pub fn output(self) -> Output {
        self.output
    }
    pub fn pending(self) -> Option<Setting> {
        self.pending
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        if self.pending.is_some() {
            return Ok(false);
        }
        if name.eq_ignore_ascii_case("Deformable") && !self.output.deformable && value.is_none() {
            self.output.deformable = true;
            return Ok(true);
        }
        if !self.output.deformable {
            return Ok(false);
        }
        let setting = if name.eq_ignore_ascii_case("Degree") {
            Setting::Degree
        } else if name.eq_ignore_ascii_case("PointCount") {
            Setting::PointCount
        } else {
            return Ok(false);
        };
        let mut next = *self;
        next.pending = Some(setting);
        if let Some(value) = value {
            next.number(crate::parse_finite_real(value)?)?;
        }
        *self = next;
        Ok(true)
    }
    pub fn number(&mut self, n: f64) -> Result<(), CommandError> {
        let setting = self
            .pending
            .ok_or(CommandError::Usage("Choose Degree or PointCount first"))?;
        let (min, max) = match setting {
            Setting::Degree => (2, 11),
            Setting::PointCount => (self.output.deformation.degree.saturating_add(2), 1000),
        };
        if !n.is_finite() || n.fract() != 0. || n < min as f64 || n > max as f64 {
            return Err(CommandError::Usage(
                "Arc Degree requires 2..11; PointCount requires degree+2..1000",
            ));
        }
        match setting {
            Setting::Degree => self.output.deformation.degree = n as usize,
            Setting::PointCount => self.output.deformation.point_count = n as usize,
        };
        self.pending = None;
        Ok(())
    }
    pub fn default_value(&mut self) -> Result<(), CommandError> {
        let value = match self.pending {
            Some(Setting::Degree) => self.output.deformation.degree,
            Some(Setting::PointCount) => self.output.deformation.point_count,
            None => return Err(CommandError::Usage("Arc getter has no default")),
        };
        self.number(value as f64)
    }
    pub fn prompt(self) -> String {
        let d = self.output.deformation;
        match self.pending {
            Some(Setting::Degree) => format!("Degree <{}>", d.degree),
            Some(Setting::PointCount) => format!("PointCount <{}>", d.point_count),
            None => format!(
                "center; Degree={}, PointCount={}, StartPoint, Tangent, Extension",
                d.degree, d.point_count
            ),
        }
    }
}
pub fn output_prefix(args: &[&str], saved: Deformation) -> Result<(Output, usize), CommandError> {
    let mut settings = Settings::new(saved);
    let mut used = 0;
    while used < args.len() {
        if settings.pending().is_some() {
            if args[used]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Enter")
            {
                settings.default_value()?;
            } else {
                settings.number(crate::parse_finite_real(
                    args[used].trim_start_matches('_'),
                )?)?;
            }
        } else if !settings.option(args[used])? {
            break;
        }
        used += 1;
    }
    if settings.pending().is_some() {
        return Err(CommandError::Usage("Arc setting requires an integer"));
    }
    Ok((settings.output(), used))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_document::Document;
    #[test]
    fn arc_deformable_ordered_bounds_retry_default_and_atomic_preferences() {
        let mut settings = Settings::new(Deformation::default());
        for option in [
            "Deformable",
            "Degree=2",
            "PointCount=4",
            "Degree=3",
            "PointCount",
        ] {
            assert!(settings.option(option).unwrap());
        }
        assert!(settings.default_value().is_err());
        assert_eq!(settings.pending(), Some(Setting::PointCount));
        for invalid in [0., 4., 4.5, 1001., f64::INFINITY] {
            assert!(settings.number(invalid).is_err());
        }
        settings.number(5.).unwrap();
        assert_eq!(
            settings.output().deformation,
            Deformation {
                degree: 3,
                point_count: 5
            }
        );
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry
            .execute(
                &mut doc,
                "A Deformable Degree=5 PointCount=8 0,0,0 4,0,0 90",
            )
            .unwrap();
        let saved = registry.arc_deformation();
        let count = doc.objects().count();
        for bad in [
            "Arc Deformable Degree=2 PointCount=4 Degree=5 0,0,0 4,0,0 90",
            "Arc Deformable Degree=2 PointCount=3 0,0,0 4,0,0 90",
            "Arc Deformable Degree=3 PointCount=10 0,0,0 0,0,0 90",
        ] {
            assert!(registry.execute(&mut doc, bad).is_err());
            assert_eq!(registry.arc_deformation(), saved);
            assert_eq!(doc.objects().count(), count);
        }
        assert_eq!(doc.undo().unwrap().as_deref(), Some("Arc"));
        assert!(doc.undo().unwrap().is_none());
        assert_eq!(registry.arc_deformation(), saved);
        assert_eq!(
            CommandRegistry::with_builtins().arc_deformation(),
            Deformation::default()
        );
    }
}
