//! Edit-free Hyperbola getters and shared exact branch construction.
use crate::{Command, CommandContext, CommandError, remembered::Remembered};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Construction {
    #[default]
    Center,
    Coefficient,
    Foci,
    Vertex,
}
impl Construction {
    pub fn parse(word: &str) -> Option<Self> {
        match word
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "default" | "center" => Some(Self::Center),
            "fromcoefficient" => Some(Self::Coefficient),
            "fromfoci" => Some(Self::Foci),
            "fromvertex" => Some(Self::Vertex),
            _ => None,
        }
    }
    pub const fn word(self) -> &'static str {
        match self {
            Self::Center => "Default",
            Self::Coefficient => "FromCoefficient",
            Self::Foci => "FromFoci",
            Self::Vertex => "FromVertex",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub a: Real,
    pub b: Real,
    pub both_branches: bool,
    pub mark_foci: bool,
    pub show_asymptotes: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            a: 1.,
            b: 1.,
            both_branches: false,
            mark_foci: false,
            show_asymptotes: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coefficient {
    A,
    B,
}
impl Coefficient {
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Center,
    FirstFocus,
    SecondFocus,
    Focus,
    Vertex,
    Direction,
    End,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Definition {
    center: Point3,
    axis: Vector3,
    radial_direction: Vector3,
    semi_transverse_axis: Real,
    semi_conjugate_axis: Real,
    axial_extent: Real,
    foci: [Point3; 2],
    plane: Frame3,
}
impl Definition {
    pub fn center(self) -> Point3 {
        self.center
    }
    pub fn coefficients(self) -> [Real; 2] {
        [self.semi_transverse_axis, self.semi_conjugate_axis]
    }
    pub fn curves(
        self,
        both: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        // The coefficient getter can finish with focus markers while the
        // rotated extent lies between the vertices. No degenerate NURBS is made.
        if self.axial_extent.abs() <= self.semi_transverse_axis {
            return Ok(Vec::new());
        }
        (if both { &[-1., 1.][..] } else { &[1.][..] })
            .iter()
            .map(|sign| hyperbola_branch(self, *sign, tolerance))
            .collect()
    }
    pub fn focus_points(self, both: bool) -> Vec<Point3> {
        if both {
            self.foci.to_vec()
        } else {
            vec![self.foci[1]]
        }
    }
    /// Two finite viewport guides on the complete mathematical asymptote lines.
    pub fn asymptotes(self) -> Result<[[Point3; 2]; 2], GeometryError> {
        let x = self.axis.scaled(self.axial_extent)?;
        let y = self
            .radial_direction
            .normalized_nonzero()?
            .as_vector()
            .scaled(self.semi_conjugate_axis * (self.axial_extent / self.semi_transverse_axis))?;
        let mut lines = [[self.center; 2]; 2];
        for (line, sign) in lines.iter_mut().zip([-1., 1.]) {
            let delta = Vector3::try_new(
                x.x() + sign * y.x(),
                x.y() + sign * y.y(),
                x.z() + sign * y.z(),
            )?;
            *line = [
                self.center.translated(delta.scaled(-1.)?)?,
                self.center.translated(delta)?,
            ];
        }
        Ok(lines)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub curves: Vec<NurbsCurve>,
    pub foci: Vec<Point3>,
    pub asymptotes: Option<[[Point3; 2]; 2]>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    construction: Construction,
    options: Options,
    points: [Option<Point3>; 3],
    count: usize,
    plane: Frame3,
    tolerance: Tolerance,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::Degenerate { context }
}
impl Input {
    pub fn new(options: Options, plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            construction: Construction::Center,
            options,
            points: [None; 3],
            count: 0,
            plane,
            tolerance,
        }
    }
    pub fn options(self) -> Options {
        self.options
    }
    pub fn construction(self) -> Construction {
        self.construction
    }
    pub fn point_count(self) -> usize {
        self.count
    }
    pub fn anchor(self) -> Option<Point3> {
        self.count.checked_sub(1).and_then(|i| self.points[i])
    }
    pub fn markers(self) -> Vec<Point3> {
        self.points.into_iter().flatten().collect()
    }
    pub fn set_initial_plane(&mut self, plane: Frame3) {
        if self.count == 0 {
            self.plane = plane;
        }
    }
    pub fn mouse_plane(self) -> Frame3 {
        self.plane
            .with_origin(self.anchor().unwrap_or(self.plane.origin()))
    }
    pub fn slot(self) -> Slot {
        match self.count {
            0 => match self.construction {
                Construction::Center | Construction::Coefficient => Slot::Center,
                Construction::Foci => Slot::FirstFocus,
                Construction::Vertex => Slot::Vertex,
            },
            1 => match self.construction {
                Construction::Center | Construction::Vertex => Slot::Focus,
                Construction::Coefficient => Slot::Direction,
                Construction::Foci => Slot::SecondFocus,
            },
            2 => Slot::End,
            _ => Slot::Complete,
        }
    }
    pub fn coefficient(&mut self, key: Coefficient, value: Real) -> Result<(), GeometryError> {
        if self.construction != Construction::Coefficient
            || self.count >= 3
            || !value.is_finite()
            || value <= 0.
        {
            return Err(invalid(
                "hyperbola coefficients require a finite positive length in FromCoefficient mode",
            ));
        }
        match key {
            Coefficient::A => self.options.a = value,
            Coefficient::B => self.options.b = value,
        };
        Ok(())
    }
    /// Rhino ends the center construction when a branch mode is chosen at its focus getter.
    pub fn cancels(self, word: &str) -> bool {
        self.construction == Construction::Center
            && self.count == 1
            && Construction::parse(word).is_some_and(|mode| mode != Construction::Center)
    }
    pub fn option(&mut self, word: &str) -> Result<bool, GeometryError> {
        let word = word.trim_start_matches(['_', '-']);
        if self.count == 0
            && let Some(construction) = Construction::parse(word)
        {
            self.construction = construction;
            return Ok(true);
        }
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(k, v)| (k, Some(v.trim_start_matches('_'))));
        if self.construction == Construction::Coefficient
            && (name.eq_ignore_ascii_case("A") || name.eq_ignore_ascii_case("B"))
        {
            let key = if name.eq_ignore_ascii_case("A") {
                Coefficient::A
            } else {
                Coefficient::B
            };
            let value = value.and_then(|v| v.parse::<Real>().ok()).ok_or(invalid(
                "hyperbola coefficient requires a finite positive length",
            ))?;
            self.coefficient(key, value)?;
            return Ok(true);
        }
        let target = if name.eq_ignore_ascii_case("BothBranches") {
            &mut self.options.both_branches
        } else if name.eq_ignore_ascii_case("MarkFoci") {
            &mut self.options.mark_foci
        } else if name.eq_ignore_ascii_case("ShowAsymptotes") {
            &mut self.options.show_asymptotes
        } else {
            return Ok(false);
        };
        *target = match value {
            None => !*target,
            Some(v) if v.eq_ignore_ascii_case("Yes") => true,
            Some(v) if v.eq_ignore_ascii_case("No") => false,
            _ => return Err(invalid("hyperbola option requires Yes or No")),
        };
        Ok(true)
    }
    pub fn point(&mut self, point: Point3) -> Result<Option<Definition>, GeometryError> {
        if self.count >= 3 {
            return Err(invalid("hyperbola input is complete"));
        }
        let mut next = *self;
        next.points[next.count] = Some(point);
        next.count += 1;
        if next.count == 2 {
            next.points[0]
                .unwrap()
                .vector_to(point)?
                .normalized(self.tolerance)?;
        }
        let result = if next.count == 3 {
            let definition = hyperbola_definition(
                next.construction,
                next.points.map(Option::unwrap),
                next.options,
                next.plane,
                next.tolerance,
            )?;
            definition.curves(next.options.both_branches, next.tolerance)?;
            Some(definition)
        } else {
            None
        };
        *self = next;
        Ok(result)
    }
    pub fn preview(self, cursor: Point3) -> Option<Preview> {
        let mut next = self;
        let definition = next.point(cursor).ok().flatten()?;
        Some(Preview {
            curves: definition
                .curves(self.options.both_branches, self.tolerance)
                .ok()?,
            foci: definition.focus_points(self.options.both_branches),
            asymptotes: self
                .options
                .show_asymptotes
                .then(|| definition.asymptotes().ok())
                .flatten(),
        })
    }
    pub fn command(self) -> String {
        let o = self.options;
        let mut words = vec!["Hyperbola".to_owned(), self.construction.word().to_owned()];
        if self.construction == Construction::Coefficient {
            words.push(format!("A={} B={}", o.a, o.b));
        }
        for (key, value) in [
            ("BothBranches", o.both_branches),
            ("MarkFoci", o.mark_foci),
            ("ShowAsymptotes", o.show_asymptotes),
        ] {
            words.push(format!("{key}={}", if value { "Yes" } else { "No" }));
        }
        words.extend(self.points.into_iter().flatten().map(|p| {
            let [x, y, z] = p.to_array();
            format!("{x},{y},{z}")
        }));
        words.join(" ")
    }
}
#[derive(Default)]
pub(super) struct HyperbolaCommand {
    options: Remembered<Options>,
}
impl Command for HyperbolaCommand {
    fn name(&self) -> &'static str {
        "Hyperbola"
    }
    fn hyperbola_options(&self) -> Option<Options> {
        Some(self.options.get())
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let usage = crate::HYPERBOLA_USAGE;
        let mut input = Input::new(
            self.options.get(),
            context.construction_plane,
            document.tolerance(),
        );
        let mut seen = std::collections::HashSet::new();
        let mut positionals = Vec::new();
        for word in arguments {
            if Construction::parse(word).is_some() && !positionals.is_empty() {
                return Err(CommandError::Usage(usage));
            }
            if input.option(word)? {
                let key = word
                    .trim_start_matches(['_', '-'])
                    .split('=')
                    .next()
                    .unwrap()
                    .to_ascii_lowercase();
                if !seen.insert(key) {
                    return Err(CommandError::Usage(usage));
                }
            } else {
                positionals.push(*word);
            }
        }
        let mut index = 0;
        let mut definition = None;
        while index < positionals.len() {
            let (point, consumed) = crate::parse_point(&positionals[index..])?;
            definition = input.point(point)?;
            index += consumed;
        }
        let definition = definition.ok_or(CommandError::Usage(usage))?;
        let o = input.options;
        let curves = definition.curves(o.both_branches, document.tolerance())?;
        let mut focus_ids = Vec::new();
        if o.mark_foci {
            for focus in definition.focus_points(o.both_branches) {
                focus_ids.push(document.add_geometry(Geometry::Point(focus))?);
            }
        }
        let mut curve_ids = Vec::new();
        for curve in curves {
            curve_ids.push(document.add_geometry(Geometry::NurbsCurve(curve))?);
        }
        self.options.set(o);
        if curve_ids.is_empty() {
            return Ok(if focus_ids.is_empty() {
                "No hyperbola geometry created".to_owned()
            } else {
                format!("Added {} focus point(s)", focus_ids.len())
            });
        }
        Ok(format!(
            "Added {} hyperbola branch{} ({curve_ids:?}){}{}",
            curve_ids.len(),
            if curve_ids.len() == 1 { "" } else { "es" },
            if focus_ids.is_empty() {
                String::new()
            } else {
                format!(" and {} focus point(s) {focus_ids:?}", focus_ids.len())
            },
            if o.show_asymptotes {
                "; asymptotes are preview-only"
            } else {
                ""
            }
        ))
    }
}
fn hyperbola_radial_component(
    offset: Vector3,
    axis: Vector3,
    axial: Real,
) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        (-axial).mul_add(axis.x(), offset.x()),
        (-axial).mul_add(axis.y(), offset.y()),
        (-axial).mul_add(axis.z(), offset.z()),
    )
}

fn centered_hyperbola_definition(
    center: Point3,
    axis: Vector3,
    focal_distance: Real,
    end: Point3,
    foci: [Point3; 2],
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Definition, GeometryError> {
    let offset = center.vector_to(end)?;
    let axial_extent = offset.dot(axis)?;
    let radial_direction = hyperbola_radial_component(offset, axis, axial_extent)?;
    let radial_extent = radial_direction.length()?;
    if axial_extent <= tolerance.absolute() || radial_extent <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "hyperbola",
        });
    }

    // The difference in distances to the two foci is 2a. Evaluate that
    // difference through its conjugate expression to retain accuracy for
    // endpoints far from the center.
    let scale = focal_distance.max(axial_extent).max(radial_extent);
    let normalized_focus = focal_distance / scale;
    let normalized_axial = axial_extent / scale;
    let normalized_radial = radial_extent / scale;
    let distance_to_negative = (normalized_axial + normalized_focus).hypot(normalized_radial);
    let distance_to_positive = (normalized_axial - normalized_focus).hypot(normalized_radial);
    let semi_transverse_axis = scale
        * ((2.0 * normalized_focus * normalized_axial)
            / (distance_to_negative + distance_to_positive));
    if !semi_transverse_axis.is_finite()
        || semi_transverse_axis <= 0.0
        || semi_transverse_axis >= focal_distance
    {
        return Err(GeometryError::Degenerate {
            context: "hyperbola",
        });
    }
    let semi_conjugate_axis = (focal_distance - semi_transverse_axis).sqrt()
        * (focal_distance + semi_transverse_axis).sqrt();
    if !semi_conjugate_axis.is_finite() || semi_conjugate_axis <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "hyperbola",
        });
    }

    Ok(Definition {
        center,
        axis,
        radial_direction,
        semi_transverse_axis,
        semi_conjugate_axis,
        axial_extent,
        foci,
        plane,
    })
}

fn hyperbola_definition(
    construction: Construction,
    points: [Point3; 3],
    options: Options,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Definition, GeometryError> {
    let [first, second, end] = points;
    match construction {
        Construction::Center => {
            let center = first;
            let to_focus = center.vector_to(second)?;
            let focal_distance = to_focus.length()?;
            let axis = to_focus.normalized(tolerance)?.as_vector();
            Frame3::try_from_x_and_normal(first, axis, plane.z_axis().as_vector(), tolerance)?;
            let opposite_focus = center.translated(axis.scaled(-focal_distance)?)?;
            centered_hyperbola_definition(
                center,
                axis,
                focal_distance,
                end,
                [opposite_focus, second],
                plane,
                tolerance,
            )
        }
        Construction::Foci => {
            let first_focus = first;
            let second_focus = second;
            let between_foci = first_focus.vector_to(second_focus)?;
            let focal_distance = 0.5 * between_foci.length()?;
            let axis = between_foci.normalized(tolerance)?.as_vector();
            Frame3::try_from_x_and_normal(first, axis, plane.z_axis().as_vector(), tolerance)?;
            let center = Point3::try_new(
                0.5 * first_focus.x() + 0.5 * second_focus.x(),
                0.5 * first_focus.y() + 0.5 * second_focus.y(),
                0.5 * first_focus.z() + 0.5 * second_focus.z(),
            )?;
            centered_hyperbola_definition(
                center,
                axis,
                focal_distance,
                end,
                [first_focus, second_focus],
                plane,
                tolerance,
            )
        }
        Construction::Vertex => {
            let vertex = first;
            let focus = second;
            let vertex_to_focus = vertex.vector_to(focus)?;
            let vertex_focus_distance = vertex_to_focus.length()?;
            let axis = vertex_to_focus.normalized(tolerance)?.as_vector();
            Frame3::try_from_x_and_normal(first, axis, plane.z_axis().as_vector(), tolerance)?;
            let offset = vertex.vector_to(end)?;
            let beyond_vertex = offset.dot(axis)?;
            let radial_direction = hyperbola_radial_component(offset, axis, beyond_vertex)?;
            let radial_extent = radial_direction.length()?;
            if beyond_vertex <= tolerance.absolute() || radial_extent <= tolerance.absolute() {
                return Err(GeometryError::Degenerate {
                    context: "hyperbola",
                });
            }

            let scale = vertex_focus_distance.max(beyond_vertex).max(radial_extent);
            let normalized_focus = vertex_focus_distance / scale;
            let normalized_beyond = beyond_vertex / scale;
            let normalized_radial = radial_extent / scale;
            let denominator =
                normalized_radial * normalized_radial - 4.0 * normalized_beyond * normalized_focus;
            let endpoint_to_focus = (normalized_beyond - normalized_focus).hypot(normalized_radial);
            let numerator = normalized_beyond
                * normalized_focus
                * (endpoint_to_focus + normalized_beyond + normalized_focus);
            if denominator <= 0.0 || numerator <= 0.0 {
                return Err(GeometryError::Degenerate {
                    context: "hyperbola",
                });
            }
            let semi_transverse_axis = scale * (numerator / denominator);
            let focal_distance = semi_transverse_axis + vertex_focus_distance;
            let semi_conjugate_axis = vertex_focus_distance.sqrt()
                * (2.0 * semi_transverse_axis + vertex_focus_distance).sqrt();
            let axial_extent = semi_transverse_axis + beyond_vertex;
            if ![
                semi_transverse_axis,
                focal_distance,
                semi_conjugate_axis,
                axial_extent,
            ]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
            {
                return Err(GeometryError::Degenerate {
                    context: "hyperbola",
                });
            }
            let center = vertex.translated(axis.scaled(-semi_transverse_axis)?)?;
            let opposite_focus = center.translated(axis.scaled(-focal_distance)?)?;
            Ok(Definition {
                center,
                axis,
                radial_direction,
                semi_transverse_axis,
                semi_conjugate_axis,
                axial_extent,
                foci: [opposite_focus, focus],
                plane,
            })
        }
        Construction::Coefficient => {
            let center = first;
            let axis = center.vector_to(second)?.normalized(tolerance)?.as_vector();
            let offset = center.vector_to(end)?;
            let axial_extent = offset.dot(axis)?;
            // Public Rhino 8 getter controls show this world-X gate, independent
            // of the rotated axis and the chosen center (see hyperbola_gates.json).
            if end.x() <= options.a {
                return Err(invalid("hyperbola end must have world X greater than A"));
            }
            let frame =
                Frame3::try_from_x_and_normal(center, axis, plane.z_axis().as_vector(), tolerance)?;
            let radial_direction = frame.y_axis().as_vector().scaled(-1.)?;
            let focal_distance = options.a.hypot(options.b);
            let negative_focus = center.translated(axis.scaled(-focal_distance)?)?;
            let positive_focus = center.translated(axis.scaled(focal_distance)?)?;
            Ok(Definition {
                center,
                axis,
                radial_direction,
                semi_transverse_axis: options.a,
                semi_conjugate_axis: options.b,
                axial_extent,
                foci: [negative_focus, positive_focus],
                plane,
            })
        }
    }
}

fn hyperbola_branch(
    definition: Definition,
    branch_sign: Real,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let frame = Frame3::try_from_directions(
        definition.center,
        definition
            .axis
            .scaled(branch_sign * definition.axial_extent.signum())?,
        definition.radial_direction,
        tolerance,
    )?;
    let curve = NurbsCurve::try_hyperbola(
        frame,
        definition.semi_transverse_axis,
        definition.semi_conjugate_axis,
        definition.axial_extent.abs(),
    )?;
    let native_plane = Frame3::try_from_x_and_normal(
        definition.center,
        definition.axis,
        definition.plane.z_axis().as_vector(),
        tolerance,
    )?;
    let cosine = definition
        .radial_direction
        .normalized_nonzero()?
        .as_vector()
        .dot(native_plane.y_axis().as_vector())?
        .abs()
        .min(1.);
    let axial_ratio = definition.semi_transverse_axis / definition.axial_extent;
    // Projecting the radial direction into the captured plane modifies the
    // rational half-angle factor. The signed extent retains projective charts.
    let squared = cosine * cosine;
    let factor = if cosine >= 1. - 8. * Real::EPSILON {
        axial_ratio
    } else {
        axial_ratio.mul_add(squared, squared - 1.)
    };
    if factor > 0. && cosine >= 1. - 8. * Real::EPSILON {
        return Ok(curve);
    }
    let middle = definition.center.translated(
        definition
            .axis
            .scaled(branch_sign * definition.semi_transverse_axis * factor)?,
    )?;
    let controls = vec![
        curve.control_points()[0],
        viboceros_geometry::WeightedPoint3::try_new(middle, 1. / factor)?,
        curve.control_points()[2],
    ];
    NurbsCurve::try_new_rational(2, controls, curve.knots().to_vec())
}
