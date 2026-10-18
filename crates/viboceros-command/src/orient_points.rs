//! Shared point-defined Orient maps for commands and read-only cursor previews.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Two,
    Three,
}
impl Kind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "orient" => Some(Self::Two),
            "orient3pt" | "orient3point" => Some(Self::Three),
            _ => None,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Two => "Orient",
            Self::Three => "Orient3Pt",
        }
    }
    pub const fn count(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Three => 3,
        }
    }
    pub const fn usage(self) -> &'static str {
        match self {
            Self::Two => {
                "Orient reference-1 reference-2 target-1 target-2 [Scale=No|1D|3D] [Copy=Yes|No]"
            }
            Self::Three => {
                "Orient3Pt reference-1 reference-2 reference-3 target-1 target-2 target-3 [Scale=Yes|No] [Copy=Yes|No]"
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scale {
    #[default]
    No,
    OneDimensional,
    Uniform,
}
impl Scale {
    pub const fn name(self, kind: Kind) -> &'static str {
        match (self, kind) {
            (Self::No, _) => "No",
            (Self::OneDimensional, _) => "1D",
            (Self::Uniform, Kind::Two) => "3D",
            (Self::Uniform, Kind::Three) => "Yes",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub scale: Scale,
}
impl Options {
    pub fn arguments(self, kind: Kind) -> String {
        format!(
            "Copy={} Scale={}",
            if self.copy { "Yes" } else { "No" },
            self.scale.name(kind)
        )
    }
}

pub fn options<'a>(
    kind: Kind,
    args: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Options), CommandError> {
    let mut rest = Vec::new();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let (name, value) = args[i]
            .split_once('=')
            .map_or((args[i], None), |(a, b)| (a, Some(b)));
        let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if matches!(name.as_str(), "copy" | "scale") {
            if !seen.insert(name.clone()) {
                return Err(CommandError::Usage(kind.usage()));
            }
            let value = match value {
                Some(v) => v,
                None => {
                    i += 1;
                    *args.get(i).ok_or(CommandError::Usage(kind.usage()))?
                }
            };
            if name == "copy" {
                options.copy = parse_yes_no(value).ok_or(CommandError::Usage(kind.usage()))?;
            } else {
                let value = value.trim_start_matches(['_', '-']);
                options.scale = if value.eq_ignore_ascii_case("No") {
                    Scale::No
                } else if kind == Kind::Two && value.eq_ignore_ascii_case("1D") {
                    Scale::OneDimensional
                } else if (kind == Kind::Two && value.eq_ignore_ascii_case("3D"))
                    || (kind == Kind::Three && value.eq_ignore_ascii_case("Yes"))
                {
                    Scale::Uniform
                } else {
                    return Err(CommandError::Usage(kind.usage()));
                };
            }
        } else {
            rest.push(args[i]);
        }
        i += 1;
    }
    Ok((rest, options))
}

/// Reference-point coincidence is independent of the document's modeling tolerance.
pub fn validate_references(points: &[Point3], _tolerance: Tolerance) -> Result<(), GeometryError> {
    let tolerance = Tolerance::NUMERICAL_VALIDATION;
    if points.len() >= 2 {
        points[0].vector_to(points[1])?.normalized(tolerance)?;
    }
    if points.len() >= 3 {
        Frame3::try_from_points(points[0], points[1], points[2], tolerance)?;
    }
    Ok(())
}

pub fn transform(
    references: &[Point3],
    targets: &[Point3],
    scale: Scale,
    _tolerance: Tolerance,
) -> Result<AffineTransform3, CommandError> {
    let tolerance = Tolerance::NUMERICAL_VALIDATION;
    if references.len() != targets.len()
        || ![2, 3].contains(&references.len())
        || references.len() == 3 && scale == Scale::OneDimensional
    {
        return Err(CommandError::Usage(
            "Orient requires matching pairs or triples of points",
        ));
    }
    let source = references[0].vector_to(references[1])?;
    let target = targets[0].vector_to(targets[1])?;
    let factor = if scale == Scale::No {
        1.
    } else {
        target.length()? / source.length()?
    };
    if !factor.is_finite() || factor <= 0. {
        return Err(CommandError::InvalidScaleFactor(factor.to_string()));
    }
    if references.len() == 3 {
        Ok(AffineTransform3::try_frame_mapping(
            Frame3::try_from_points(references[0], references[1], references[2], tolerance)?,
            Frame3::try_from_points(targets[0], targets[1], targets[2], tolerance)?,
            [factor; 3],
        )?)
    } else {
        let from = source.normalized(tolerance)?;
        let to = target.normalized(tolerance)?;
        let perpendicular = if scale == Scale::Uniform { factor } else { 1. };
        // OpenNURBS chooses PerpendicularTo(from), rather than an arbitrary
        // least-aligned cross product, when the two directions are opposite.
        // The existing normal-frame constructor implements that public policy.
        if from.as_vector().cross(to.as_vector())?.length()? == 0.0
            && from.as_vector().dot(to.as_vector())? < 0.0
        {
            let axis = Frame3::try_from_unit_normal(references[0], from)?.x_axis();
            return Ok(AffineTransform3::try_frame_mapping(
                Frame3::try_from_unit_directions(references[0], from, axis, tolerance)?,
                Frame3::try_from_unit_directions(targets[0], to, axis, tolerance)?,
                [factor, perpendicular, perpendicular],
            )?);
        }
        Ok(AffineTransform3::try_direction_mapping(
            references[0],
            from,
            targets[0],
            to,
            factor,
            perpendicular,
            Tolerance::MESH_VALIDATION,
        )?)
    }
}

/// Each accepted destination fixes one axis before the next cursor point arrives.
#[derive(Clone, Copy, Debug)]
pub struct Preview {
    pub references: [Point3; 2],
    pub third: Option<Point3>,
    pub targets: [Option<Point3>; 2],
    pub scale: Scale,
}
impl Preview {
    pub fn transform_at(
        self,
        point: Point3,
        tolerance: Tolerance,
    ) -> Result<AffineTransform3, CommandError> {
        let Some(first) = self.targets[0] else {
            return Ok(AffineTransform3::from_translation(
                self.references[0].vector_to(point)?,
            ));
        };
        if let (Some(third), Some(second)) = (self.third, self.targets[1]) {
            transform(
                &[self.references[0], self.references[1], third],
                &[first, second, point],
                self.scale,
                tolerance,
            )
        } else {
            transform(&self.references, &[first, point], self.scale, tolerance)
        }
    }
}

pub(super) struct OrientCommand {
    kind: Kind,
    scale: remembered::Remembered<Scale>,
}
impl OrientCommand {
    pub(super) fn new(kind: Kind) -> Self {
        Self {
            kind,
            scale: remembered::Remembered::new(Scale::No),
        }
    }
}
impl Command for OrientCommand {
    fn orient_point_scale(&self) -> Option<Scale> {
        Some(self.scale.get())
    }
    fn remember_orient_point_scale(&self, value: Scale) -> bool {
        if self.kind == Kind::Three && value == Scale::OneDimensional {
            return false;
        }
        self.scale.set(value);
        true
    }
    fn name(&self) -> &'static str {
        self.kind.name()
    }
    fn aliases(&self) -> &'static [&'static str] {
        if self.kind == Kind::Three {
            &["Orient3Point"]
        } else {
            &[]
        }
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (args, sources) = affine_transform_arguments(document, args, self.kind.usage())?;
        let (args, options) = options(
            self.kind,
            &args,
            Options {
                scale: self.scale.get(),
                ..Default::default()
            },
        )?;
        let mut points = Vec::new();
        let mut consumed = 0;
        for _ in 0..2 * self.kind.count() {
            let (p, n) = parse_point(&args[consumed..])?;
            points.push(p);
            consumed += n;
        }
        require_consumed(&args, consumed, self.kind.usage())?;
        let map = transform(
            &points[..self.kind.count()],
            &points[self.kind.count()..],
            options.scale,
            document.tolerance(),
        )?;
        let (transformed, copied) =
            apply_transform_with_renewal(document, &sources, map, options.copy)?;
        self.scale.set(options.scale);
        Ok(format!(
            "Oriented {transformed} object(s) with {} scaling, creating {copied} copy object(s)",
            options.scale.name(self.kind)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orient_points_failed_commands_preserve_model_and_scale_preferences() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let source = doc
            .add_geometry(Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))
            .unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(&mut doc, "Orient 0,0,0 1,0,0 0,0,0 2,0,0 Scale=1D")
            .unwrap();
        let before = format!("{doc:?}");
        for command in [
            "Orient 0,0,0 0,0,0 3,0,0 4,0,0 Scale=3D",
            "Orient 0,0,0 1,0,0 3,0,0 3,0,0 Scale=3D",
            "Orient 0,0,0 1,0,0 3,0,0 4,0,0 Scale=No Scale=3D",
            "Orient3Pt 0,0,0 1,0,0 2,0,0 0,0,0 1,0,0 0,1,0 Scale=Yes",
            "Orient3Pt 0,0,0 1,0,0 0,1,0 0,0,0 1,0,0 2,0,0 Scale=Yes",
            "Orient3Pt 0,0,0 1,0,0 0,1,0 0,0,0 1,0,0 0,1,0 Scale=1D",
        ] {
            assert!(registry.execute(&mut doc, command).is_err(), "{command}");
            assert_eq!(format!("{doc:?}"), before);
            assert_eq!(
                registry.orient_point_scale(Kind::Two),
                Scale::OneDimensional
            );
            assert_eq!(registry.orient_point_scale(Kind::Three), Scale::No);
        }
        registry
            .execute(&mut doc, "Orient 0,0,0 1,0,0 0,0,0 0,3,0")
            .unwrap();
        let Geometry::Point(point) = doc.object(source).unwrap().geometry() else {
            panic!()
        };
        assert!(
            point
                .distance_to(Point3::try_new(0., 6., 0.).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}
