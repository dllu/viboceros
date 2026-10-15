//! Tapered curve extrusion: kernel geometry, staged document admission.
use super::*;

pub const USAGE: &str = "ExtrudeCrvTapered distance | base target [DraftAngle=degrees] [Solid=Yes|No] [Corners=Sharp|Round] [DeleteInput=Yes|No] [Direction=x,y,z] [FlipAngle]";
pub(super) struct ExtrudeTaperedCommand;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaperedExtrusionOptions {
    pub draft_angle: f64,
    pub solid: bool,
    pub round_corners: bool,
    pub delete_input: bool,
    pub direction: Option<Vector3>,
}
impl Default for TaperedExtrusionOptions {
    fn default() -> Self {
        Self {
            draft_angle: 5.,
            solid: false,
            round_corners: false,
            delete_input: false,
            direction: None,
        }
    }
}
impl TaperedExtrusionOptions {
    pub fn parse(words: &[&str]) -> Result<Self, CommandError> {
        let mut result = Self::default();
        result.update(words)?;
        Ok(result)
    }
    /// Apply one accepted option line atomically, retaining prior values.
    pub fn update(&mut self, words: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for word in words {
            let word = word.trim_start_matches('_');
            if word.eq_ignore_ascii_case("FlipAngle") {
                if !seen.insert("flipangle".into()) {
                    return Err(CommandError::Usage(USAGE));
                }
                next.draft_angle = -next.draft_angle;
                continue;
            }
            let (key, value) = word.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "draftangle" => {
                    let angle = parse_finite_real(value)?;
                    if angle.abs() >= 90. {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.draft_angle = angle;
                }
                "solid" => next.solid = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "deleteinput" => {
                    next.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "corners" => {
                    next.round_corners =
                        match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                            "sharp" => false,
                            "round" => true,
                            _ => return Err(CommandError::Usage(USAGE)),
                        }
                }
                "direction" => {
                    let (point, count) = parse_point(&[value])?;
                    if count != 1 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    next.direction = Some(
                        Vector3::try_from(point.to_array())?
                            .normalized_nonzero()?
                            .as_vector(),
                    );
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        let mut result = format!(
            "DraftAngle={} Solid={} Corners={} DeleteInput={}",
            self.draft_angle,
            if self.solid { "Yes" } else { "No" },
            if self.round_corners { "Round" } else { "Sharp" },
            if self.delete_input { "Yes" } else { "No" }
        );
        if let Some(direction) = self.direction {
            let [x, y, z] = direction.to_array();
            result.push_str(&format!(" Direction={x},{y},{z}"));
        }
        result
    }
}

impl Command for ExtrudeTaperedCommand {
    fn name(&self) -> &'static str {
        "ExtrudeCrvTapered"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if args.iter().any(|s| {
            !s.contains('=') && !s.trim_start_matches('_').eq_ignore_ascii_case("FlipAngle")
        }) {
            return Ok(None);
        }
        TaperedExtrusionOptions::parse(args)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (options, distance) = parse(args)?;
        let curves = doc
            .selected_objects()
            .filter_map(|o| {
                o.geometry()
                    .nurbs_curve_representation()
                    .map(|c| c.map(|c| (o.id(), c)))
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if curves.is_empty() {
            return Err(CommandError::NoExtrudableCurves);
        }
        let mut staged = Vec::new();
        for (id, curve) in curves {
            staged.push((
                id,
                produce(
                    &curve,
                    distance,
                    options,
                    context.construction_plane,
                    doc.tolerance(),
                )?,
            ));
        }
        let count = staged.len();
        let mut ids = Vec::with_capacity(count);
        for (source, geometry) in staged {
            ids.push(doc.add_geometry(geometry)?);
            if options.delete_input {
                doc.delete_object(source)?;
            }
        }
        doc.select_command_results(ids)?;
        Ok(format!("Tapered {count} curve object(s)"))
    }
}
#[derive(Clone, Copy)]
enum Distance {
    Numeric(f64),
    Displacement(Vector3),
}
fn parse(args: &[&str]) -> Result<(TaperedExtrusionOptions, Distance), CommandError> {
    let (positional, options): (Vec<_>, Vec<_>) = args.iter().copied().partition(|s| {
        !s.contains('=') && !s.trim_start_matches('_').eq_ignore_ascii_case("FlipAngle")
    });
    let options = TaperedExtrusionOptions::parse(&options)?;
    let distance = if positional.len() == 1 {
        let height = parse_finite_real(positional[0])?;
        if height == 0. {
            return Err(CommandError::Usage(USAGE));
        }
        Distance::Numeric(height)
    } else {
        let (base, n) = parse_point(&positional)?;
        let (target, m) = parse_point(&positional[n..])?;
        if n + m != positional.len() || options.direction.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let displacement = base.vector_to(target)?;
        displacement.normalized_nonzero()?;
        Distance::Displacement(displacement)
    };
    Ok((options, distance))
}
fn produce(
    curve: &NurbsCurve,
    distance: Distance,
    options: TaperedExtrusionOptions,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        use viboceros_smlib::{KernelCurve, Shape};
        let origin = curve.evaluate(*curve.domain().start())?;
        let zero = Point3::try_new(0., 0., 0.)?;
        let local =
            curve.transformed(AffineTransform3::from_translation(origin.vector_to(zero)?))?;
        let profile = KernelCurve::from_nurbs_for_modeling(&local)?;
        let displacement = match distance {
            Distance::Displacement(d) => d,
            Distance::Numeric(height) => {
                let direction = if let Some(direction) = options.direction {
                    direction
                } else if let Some(normal) =
                    KernelCurve::parallel_plane_normal(&[&profile], tolerance)?
                {
                    let c = normal.to_array();
                    let axis = (0..3)
                        .max_by(|&a, &b| c[a].abs().total_cmp(&c[b].abs()))
                        .unwrap();
                    if c[axis] < 0. {
                        normal.scaled(-1.)?
                    } else {
                        normal
                    }
                } else {
                    cplane.z_axis().as_vector()
                };
                direction.scaled(height)?
            }
        };
        let cap = options.solid && curve.is_closed()?;
        let brep = Shape::tapered_extrusion(
            &profile,
            displacement,
            options.draft_angle,
            options.round_corners,
            cap,
            tolerance,
        )?
        .to_brep(tolerance)?
        .transformed(
            AffineTransform3::from_translation(zero.vector_to(origin)?),
            tolerance,
        )?;
        if cap && !brep.is_solid() {
            return Err(CommandError::Usage(
                "Tapered extrusion did not produce a closed solid",
            ));
        }
        Ok(Geometry::Brep(brep))
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        // Validate the same displacement form before declining the unavailable producer.
        match distance {
            Distance::Numeric(h) => {
                let _ = h;
            }
            Distance::Displacement(d) => {
                let _ = d;
            }
        }
        let _ = (curve, options, cplane, tolerance);
        Err(CommandError::TaperedExtrusionRequiresNativeKernel)
    }
}
#[cfg(test)]
mod tests;
