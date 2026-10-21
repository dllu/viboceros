//! Circle construction and a separate, non-mutating second-circle stage.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Around,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
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
        Self {
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
                "AroundFaces"
            } else {
                "VerticalFaces"
            }
            .into();
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
        format!(
            "{}; AroundFaces={} VerticalFaces={}; Tangent AroundCurve FitPoints",
            self.getter.prompt(),
            self.preferences.around_faces,
            self.preferences.vertical_faces
        )
    }
    fn count(&mut self, key: Pending, value: &str) -> Result<(), CommandError> {
        let n = value
            .trim_start_matches('_')
            .parse::<usize>()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let mut p = self.preferences;
        if key == Pending::Around {
            p.around_faces = n;
        } else {
            p.vertical_faces = n;
        }
        p.validate()?;
        self.preferences = p;
        self.pending = None;
        Ok(())
    }
    pub fn option(&mut self, word: &str) -> Result<bool, CommandError> {
        if let Some(p) = self.pending {
            self.count(p, word)?;
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
            if !self.at_start() {
                return Err(CommandError::Usage(USAGE));
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
                        _ => return Err(CommandError::Usage(USAGE)),
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
                return Err(CommandError::Usage(USAGE));
            }
            return Ok(false);
        }
        if name.eq_ignore_ascii_case("FixInnerDimension") {
            return Err(CommandError::Usage(USAGE));
        }
        if self.getter.option(name) {
            if let Some(v) = value {
                self.number(crate::parse_finite_real(v)?)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
    fn accepted_circle(&mut self, circle: CircleDefinition) {
        if self.getter.remembers_radius() {
            self.preferences.radius = circle.radius;
        }
        if self.getter.has_size_mode() {
            self.preferences.diameter = match self.getter.size_mode() {
                CircleSizeMode::Radius => false,
                CircleSizeMode::Diameter => true,
                _ => self.preferences.diameter,
            };
        }
        self.circle = Some(circle);
    }
    pub fn accept_reference(&mut self, circle: CircleDefinition) -> Result<(), CommandError> {
        if !self.at_start() || self.pending.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if !circle.radius.is_finite() || circle.radius <= 0. {
            return Err(CommandError::Usage(USAGE));
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
            return Err(CommandError::Usage(USAGE));
        }
        if let Some(circle) = crate::circle_fit_points::selected_circle(doc)? {
            self.reference_circle(circle)
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
        }
        Ok(())
    }
    pub fn point(&mut self, mut point: Point3) -> Result<(), CommandError> {
        if self.pending.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if let Some(circle) = self.circle {
            let d = super::tube::point(circle, point, self.preferences)?;
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
            return Err(CommandError::Usage(USAGE));
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
            Err(CommandError::Usage(USAGE))
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
