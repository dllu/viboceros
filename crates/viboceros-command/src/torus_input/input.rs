//! Circle construction and a separate, non-mutating second-circle stage.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Around,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub kind: Kind,
    pub preferences: Preferences,
    pub circle: Option<CircleDefinition>,
    pub definition: Option<Definition>,
    getter: CircleInput,
    project_base: bool,
    zero_origin: Point3,
    pending: Option<Pending>,
}
impl Input {
    pub fn new(plane: Frame3, preferences: Preferences) -> Self {
        Self::for_kind(Kind::SubD, plane, preferences)
    }

    pub fn for_kind(kind: Kind, plane: Frame3, preferences: Preferences) -> Self {
        Self {
            kind,
            preferences,
            circle: None,
            definition: None,
            getter: CircleInput::new(plane).with_size_mode(if preferences.diameter {
                CircleSizeMode::Diameter
            } else {
                CircleSizeMode::Radius
            }),
            project_base: false,
            zero_origin: plane.origin(),
            pending: None,
        }
    }
    pub fn at_start(&self) -> bool {
        self.circle.is_none() && self.getter.at_start()
    }
    fn at_center_prompt(&self) -> bool {
        self.at_start()
            || self.kind == Kind::Mesh
                && self.circle.is_none()
                && self.getter.has_size_mode()
                && self.getter.anchor().is_none()
    }

    pub fn snap_projection_plane(&self) -> Option<Frame3> {
        if !self.kind.projects_base_snaps() || self.circle.is_some() || self.pending.is_some() {
            return None;
        }
        self.getter.snap_projection_plane()
    }

    pub fn anchor(&self) -> Option<Point3> {
        self.circle
            .map(|c| c.frame.origin())
            .or(self.getter.anchor())
    }
    pub fn pending_value(&self) -> bool {
        self.pending.is_some()
    }
    pub fn wants_number(&self) -> bool {
        self.pending.is_some() || self.circle.is_some() || self.getter.requests_size()
    }
    pub fn scalar_units(&self) -> bool {
        self.pending.is_some()
            || self.circle.is_none() && self.getter.size_mode() == CircleSizeMode::Area
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.circle.is_none() && !self.project_base {
            if self.getter.anchor().is_none() {
                self.zero_origin = plane.origin();
            }
            self.getter.set_initial_plane(plane);
        }
    }
    pub fn mouse_plane(&self) -> Option<Frame3> {
        self.circle.map(|c| c.frame).or(self.getter.mouse_plane())
    }
    pub fn prompt(&self) -> String {
        if let Some(p) = self.pending {
            return if p == Pending::Around {
                format!("AroundFaces <{}>", self.preferences.around_faces)
            } else {
                format!("VerticalFaces <{}>", self.preferences.vertical_faces)
            };
        }
        if self.circle.is_some() {
            return format!(
                "Second {} <{}>; {} FixInnerDimension={}",
                if self.preferences.tube_diameter {
                    "diameter"
                } else {
                    "radius"
                },
                self.preferences.tube_radius
                    * if self.preferences.tube_diameter {
                        2.
                    } else {
                        1.
                    },
                if self.preferences.tube_diameter {
                    "Radius"
                } else {
                    "Diameter"
                },
                if self.preferences.fix_inner_dimension {
                    "Yes"
                } else {
                    "No"
                }
            );
        }
        if self.kind == Kind::SubD {
            return format!(
                "{}; AroundFaces={} VerticalFaces={}; Tangent AroundCurve FitPoints",
                self.getter.prompt(),
                self.preferences.around_faces,
                self.preferences.vertical_faces,
            );
        }
        let mut prompt = self
            .getter
            .prompt()
            .replace("Center of circle", "Center of torus");
        if self.kind == Kind::Mesh && self.at_center_prompt() {
            prompt.push_str(&format!(
                "; VerticalFaces={} AroundFaces={}",
                self.preferences.vertical_faces, self.preferences.around_faces
            ));
        }
        if self.at_start() {
            prompt.push_str("; Tangent AroundCurve FitPoints");
        }
        if self.getter.requests_size() {
            match self.getter.size_mode() {
                CircleSizeMode::Radius => {
                    prompt.push_str(&format!(" <{}>", self.preferences.radius))
                }
                CircleSizeMode::Diameter => {
                    prompt.push_str(&format!(" <{}>", self.preferences.radius * 2.))
                }
                _ => (),
            }
        }
        prompt
    }
    fn count(&mut self, key: Pending, value: &str) -> Result<(), CommandError> {
        let n = value
            .trim_start_matches('_')
            .parse::<usize>()
            .map_err(|_| CommandError::Usage(self.kind.usage()))?;
        let mut p = self.preferences;
        if key == Pending::Around {
            p.around_faces = n;
        } else {
            p.vertical_faces = n;
        }
        p.validate_for(self.kind)?;
        self.preferences = p;
        self.pending = None;
        Ok(())
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        if let Some(p) = self.pending {
            if option_name_eq(word, "Enter") {
                self.enter()?;
            } else {
                self.count(p, word)?;
            }
            return Ok(true);
        }
        let word = word.trim_start_matches(['_', '-']);
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v)));
        let count = if name.eq_ignore_ascii_case("AroundFaces") {
            Some(Pending::Around)
        } else if name.eq_ignore_ascii_case("VerticalFaces") {
            Some(Pending::Vertical)
        } else {
            None
        };
        if let Some(p) = count {
            if self.kind == Kind::Solid || !self.at_center_prompt() {
                return Err(CommandError::Usage(self.kind.usage()));
            }
            if let Some(v) = value {
                self.count(p, v)?;
            } else {
                self.pending = Some(p);
            }
            return Ok(true);
        }
        if self.circle.is_some() {
            if name.eq_ignore_ascii_case("FixInnerDimension") {
                self.preferences.fix_inner_dimension =
                    match value.map(|v| v.trim_start_matches('_')) {
                        None => !self.preferences.fix_inner_dimension,
                        Some(v) if v.eq_ignore_ascii_case("Yes") => true,
                        Some(v) if v.eq_ignore_ascii_case("No") => false,
                        _ => return Err(CommandError::Usage(self.kind.usage())),
                    };
                return Ok(true);
            }
            if name.eq_ignore_ascii_case("Radius") || name.eq_ignore_ascii_case("Diameter") {
                self.preferences.tube_diameter = name.eq_ignore_ascii_case("Diameter");
                if let Some(v) = value {
                    self.number(crate::parse_finite_real(v)?)?;
                }
                return Ok(true);
            }
            if CircleSizeMode::parse(name).is_some() {
                return Err(CommandError::Usage(self.kind.usage()));
            }
            return Ok(false);
        }
        if name.eq_ignore_ascii_case("FixInnerDimension") {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if self.getter.option(name) {
            if self.kind == Kind::Solid {
                self.remember_base_mode();
            }
            if let Some(v) = value {
                self.number(crate::parse_finite_real(v)?)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
    fn remember_base_mode(&mut self) {
        if self.getter.has_size_mode() {
            self.preferences.diameter = match self.getter.size_mode() {
                CircleSizeMode::Radius => false,
                CircleSizeMode::Diameter => true,
                _ => self.preferences.diameter,
            };
        }
    }
    fn accepted_circle(&mut self, circle: CircleDefinition) {
        if self.getter.remembers_radius() {
            self.preferences.radius = circle.radius;
        }
        self.remember_base_mode();
        self.circle = Some(circle);
    }
    pub fn accept_reference(&mut self, circle: CircleDefinition) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if !circle.radius.is_finite() || circle.radius <= 0. {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        self.circle = Some(circle);
        self.preferences.radius = circle.radius;
        Ok(())
    }
    pub fn reference_circle(&mut self, circle: Circle3) -> Result<(), CommandError> {
        self.accept_reference(CircleDefinition::from_circle(circle)?)
    }
    pub fn fit_points(&mut self, doc: &Document) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(circle) = crate::circle_fit_points::selected_circle(doc)? {
            let remembered_radius = self.preferences.radius;
            self.reference_circle(circle)?;
            if self.kind != Kind::SubD {
                self.preferences.radius = remembered_radius;
            }
            Ok(())
        } else {
            let mut sums: [viboceros_geometry::FiniteSum; 3] =
                std::array::from_fn(|_| Default::default());
            for point in crate::circle_fit_points::selected_points(doc) {
                for (sum, value) in sums.iter_mut().zip(point.to_array()) {
                    sum.add(value)?;
                }
            }
            let center = Point3::try_new(sums[0].mean()?, sums[1].mean()?, sums[2].mean()?)?;
            self.circle = Some(CircleDefinition {
                frame: self.getter.construction_plane().with_origin(center),
                radius: 0.,
            });
            Ok(())
        }
    }
    pub fn around_frame(&mut self, frame: Frame3) {
        self.getter = CircleInput::at_center(frame, frame.origin()).with_size_mode(
            if self.preferences.diameter {
                CircleSizeMode::Diameter
            } else {
                CircleSizeMode::Radius
            },
        );
        self.project_base = true;
    }
    fn accepted_tube(&mut self, d: Definition) {
        self.preferences.tube_radius = d.tube_radius;
        self.definition = Some(d);
    }
    pub fn number(&mut self, value: f64) -> Result<(), CommandError> {
        if let Some(p) = self.pending {
            return self.count(p, &value.to_string());
        }
        if let Some(circle) = self.circle {
            if value == 0. {
                return self.point(self.zero_origin);
            }
            let d = super::tube::number(circle, value, self.preferences)?;
            self.accepted_tube(d);
        } else if let Some(c) = self.getter.number(value)? {
            self.accepted_circle(c);
        } else if self.kind == Kind::Solid && self.getter.has_size_mode() {
            // A numeric vertical size is remembered before its direction pick.
            self.preferences.radius = self.getter.size_mode().radius(value);
        }
        Ok(())
    }
    pub fn point(&mut self, mut point: Point3) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if let Some(circle) = self.circle {
            let d = super::tube::point(circle, point, self.preferences, self.kind)?;
            self.accepted_tube(d);
        } else {
            if self.project_base {
                let f = self.getter.construction_plane();
                let [x, y] = f.projected_coordinates_of(point)?;
                point = f.point_at([x, y, 0.])?;
            }
            if let Some(c) = self.getter.point(point)? {
                self.accepted_circle(c);
            }
        }
        Ok(())
    }
    pub fn enter(&mut self) -> Result<(), CommandError> {
        if self.pending.is_some() {
            if self.kind == Kind::Mesh {
                self.pending = None;
                return Ok(());
            }
            return Err(CommandError::Usage(self.kind.usage()));
        }
        if self.circle.is_some() {
            self.number(
                self.preferences.tube_radius
                    * if self.preferences.tube_diameter {
                        2.
                    } else {
                        1.
                    },
            )
        } else if self.getter.requests_size()
            && matches!(
                self.getter.size_mode(),
                CircleSizeMode::Radius | CircleSizeMode::Diameter
            )
        {
            self.number(
                self.preferences.radius
                    * if self.getter.size_mode() == CircleSizeMode::Diameter {
                        2.
                    } else {
                        1.
                    },
            )
        } else {
            Err(CommandError::Usage(self.kind.usage()))
        }
    }
    pub fn preview(&self, point: Point3) -> Option<Definition> {
        let mut p = *self;
        p.point(point).ok()?;
        p.definition
    }
    pub fn preview_circle(&self, point: Point3) -> Option<CircleDefinition> {
        self.circle.or_else(|| self.getter.preview(point))
    }
}
