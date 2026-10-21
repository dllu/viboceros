//! A base ellipse, followed by a normal-axis measurement or a focal spheroid.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Around,
    Vertical,
    Cap,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub preferences: Preferences,
    pub getter: EllipseInput,
    pub base: Option<ellipse_input::Definition>,
    pub third_radius: Option<f64>,
    zero_origin: Point3,
    compensation_plane: Frame3,
    tolerance: Tolerance,
    pending: Option<Pending>,
}
impl Input {
    pub fn new(plane: Frame3, p: Preferences, tolerance: Tolerance) -> Self {
        Self {
            preferences: p,
            getter: EllipseInput::new(
                ellipse_input::Options {
                    mark_foci: p.mark_foci,
                    ..Default::default()
                },
                plane,
                tolerance,
            ),
            base: None,
            third_radius: None,
            zero_origin: plane.origin(),
            compensation_plane: plane,
            tolerance,
            pending: None,
        }
    }
    pub fn at_start(&self) -> bool {
        self.base.is_none()
            && self.getter.slot() == Slot::First
            && self.getter.mode() == Mode::Default
    }
    pub fn complete(&self) -> bool {
        self.third_radius.is_some()
    }
    pub fn zero_origin(&self) -> Point3 {
        self.zero_origin
    }
    pub fn pending_prompt(&self) -> Option<&'static str> {
        self.pending.map(|p| match p {
            Pending::Around => "AroundFaces",
            Pending::Vertical => "VerticalFaces",
            Pending::Cap => "CapFaceStyle: Tri or Quad",
        })
    }
    pub fn pending_value(&self) -> bool {
        self.pending.is_some()
    }
    pub fn topology_option(word: &str) -> bool {
        matches!(
            word.trim_start_matches(['_', '-'])
                .split('=')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "aroundfaces" | "verticalfaces" | "capfacestyle"
        )
    }
    pub fn unsupported_option(&self, word: &str) -> bool {
        let name = word
            .trim_start_matches(['_', '-'])
            .split('=')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            name.as_str(),
            "radius" | "default" | "deformable" | "degree" | "pointcount"
        ) || name == "vertical" && self.getter.mode() != Mode::Diameter
    }
    fn value(&mut self, key: Pending, value: &str) -> Result<(), CommandError> {
        let mut p = self.preferences;
        match key {
            Pending::Cap => {
                p.quad_caps = if value.eq_ignore_ascii_case("Quad") {
                    true
                } else if value.eq_ignore_ascii_case("Tri") {
                    false
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            Pending::Around | Pending::Vertical => {
                let n = value.parse().map_err(|_| CommandError::Usage(USAGE))?;
                if key == Pending::Around {
                    p.around_faces = n;
                } else {
                    p.vertical_faces = n;
                }
            }
        }
        p.validate()?;
        self.preferences = p;
        self.pending = None;
        Ok(())
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim_start_matches(['_', '-']);
        if let Some(key) = self.pending {
            if word.eq_ignore_ascii_case("Enter") {
                self.pending = None;
                return Ok(true);
            }
            self.value(key, word)?;
            return Ok(true);
        }
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v.trim_start_matches('_'))));
        let key = match name.to_ascii_lowercase().as_str() {
            "aroundfaces" => Some(Pending::Around),
            "verticalfaces" => Some(Pending::Vertical),
            "capfacestyle" => Some(Pending::Cap),
            _ => None,
        };
        if let Some(key) = key {
            if !self.at_start()
                || key == Pending::Cap && !self.preferences.around_faces.is_multiple_of(2)
            {
                return Err(CommandError::Usage(USAGE));
            }
            if let Some(v) = value {
                self.value(key, v)?;
            } else {
                self.pending = Some(key);
            }
            return Ok(true);
        }
        if self.unsupported_option(word) {
            return Err(CommandError::Usage(USAGE));
        }
        if self.base.is_some() {
            return Ok(false);
        }
        Ok(self.getter.option(word)?)
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.base.is_none() && self.getter.slot() == Slot::First {
            self.zero_origin = plane.origin();
            self.compensation_plane = plane;
            self.getter.set_initial_plane(plane);
        }
    }
    pub fn accept_base(&mut self, base: ellipse_input::Definition) {
        self.preferences.mark_foci = self.getter.options().mark_foci;
        self.base = Some(base);
        if self.getter.mode() == Mode::FromFoci {
            self.third_radius = Some(base.ellipse.radius_y());
        }
    }
    pub fn point(&mut self, point: Point3) -> Result<(), CommandError> {
        if self.pending.is_some() || self.complete() {
            return Err(CommandError::Usage(USAGE));
        }
        if let Some(base) = self.base {
            self.third_radius = Some(
                base.ellipse
                    .center()
                    .vector_to(point)?
                    .dot(base.ellipse.normal()?.as_vector())?
                    .abs(),
            );
        } else if let Some(base) = self.getter.point(point)? {
            self.accept_base(base);
        }
        Ok(())
    }
    pub fn number(&mut self, value: f64) -> Result<(), CommandError> {
        if let Some(key) = self.pending {
            return self.value(key, &value.to_string());
        }
        if self.complete() || !value.is_finite() {
            return Err(CommandError::Usage(USAGE));
        }
        if value == 0. && (self.base.is_some() || self.getter.slot() != Slot::SecondAxis) {
            return self.point(self.zero_origin);
        }
        if self.base.is_some() {
            self.third_radius = Some(
                value.abs()
                    * if self.getter.mode() == Mode::Diameter {
                        0.5
                    } else {
                        1.
                    },
            );
        } else {
            self.getter.number(value)?;
        }
        Ok(())
    }
    pub fn frame(&self) -> Result<Frame3, CommandError> {
        let e = self.base.ok_or(CommandError::Usage(USAGE))?.ellipse;
        Ok(Frame3::try_from_directions(
            e.center(),
            e.x_axis().as_vector(),
            e.y_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )?)
    }
    pub fn mouse_plane(&self) -> Option<Frame3> {
        let f = self.frame().ok()?;
        Frame3::try_from_directions(
            f.origin(),
            f.x_axis().as_vector(),
            f.z_axis().as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .ok()
    }
    pub fn geometry(&self) -> Result<Geometry, CommandError> {
        self.preferences.validate()?;
        let e = self.base.ok_or(CommandError::Usage(USAGE))?.ellipse;
        let frame = self.frame()?;
        let radii = [
            e.radius_x(),
            e.radius_y(),
            self.third_radius.ok_or(CommandError::Usage(USAGE))?,
        ];
        let counts = [
            self.preferences.around_faces,
            self.preferences.vertical_faces,
        ];
        let cap = self.preferences.cap_style();
        let subd = if self.getter.mode() == Mode::FromFoci {
            SubD::try_ellipsoid_in_reference_frame(
                frame,
                radii,
                counts,
                cap,
                self.compensation_plane,
            )?
        } else {
            SubD::try_ellipsoid(frame, radii, counts, cap)?
        };
        Ok(Geometry::SubD(subd))
    }

    pub fn tolerance(&self) -> Tolerance {
        self.tolerance
    }
}
