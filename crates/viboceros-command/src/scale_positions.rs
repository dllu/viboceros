//! Bounding-box center placement, independent of group layout and deformation.
use super::*;
use crate::rigid_transform::{RigidLayout, rigid_map};
use viboceros_document::ReplacementHistory;

pub const USAGE: &str = "ScalePositions origin factor [direction] | origin reference target [Mode=1D|2D|3D] [Copy=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScaleMode {
    OneDimensional,
    TwoDimensional,
    #[default]
    ThreeDimensional,
}

impl ScaleMode {
    pub fn parse(token: &str) -> Option<Self> {
        match token.trim_start_matches('_').to_ascii_lowercase().as_str() {
            "1d" => Some(Self::OneDimensional),
            "2d" => Some(Self::TwoDimensional),
            "3d" => Some(Self::ThreeDimensional),
            _ => None,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::OneDimensional => "1D",
            Self::TwoDimensional => "2D",
            Self::ThreeDimensional => "3D",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PositionOptions {
    pub mode: ScaleMode,
    pub copy: bool,
}

fn options<'a>(
    arguments: &[&'a str],
    mut options: PositionOptions,
) -> Result<(Vec<&'a str>, PositionOptions), CommandError> {
    let mut remaining = Vec::new();
    let mut i = 0;
    while i < arguments.len() {
        let token = arguments[i];
        if let Some((name, value)) = token.split_once('=')
            && option_name_eq(name, "Mode")
        {
            options.mode = ScaleMode::parse(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(token, "Mode") {
            i += 1;
            options.mode = ScaleMode::parse(arguments.get(i).ok_or(CommandError::Usage(USAGE))?)
                .ok_or(CommandError::Usage(USAGE))?;
        } else {
            remaining.push(token);
        }
        i += 1;
    }
    let has_copy = remaining.iter().any(|token| {
        token
            .split_once('=')
            .is_some_and(|(name, _)| option_name_eq(name, "Copy"))
    });
    let (positional, copy) = parse_transform_copy_arguments(&remaining, USAGE)?;
    if has_copy {
        options.copy = copy;
    }
    Ok((positional, options))
}

pub fn start_options(
    arguments: &[&str],
    defaults: PositionOptions,
) -> Result<PositionOptions, CommandError> {
    let copy_pair;
    let arguments = if let [name, value] = arguments
        && option_name_eq(name, "Copy")
    {
        copy_pair = format!("Copy={value}");
        vec![copy_pair.as_str()]
    } else {
        arguments.to_vec()
    };
    let (positional, options) = options(&arguments, defaults)?;
    if positional.is_empty() {
        Ok(options)
    } else {
        Err(CommandError::Usage(USAGE))
    }
}

pub(super) struct ScalePositionsCommand {
    factor: remembered::Remembered<Real>,
    mode: remembered::Remembered<ScaleMode>,
}

impl Default for ScalePositionsCommand {
    fn default() -> Self {
        Self {
            factor: remembered::Remembered::new(1.),
            mode: remembered::Remembered::new(ScaleMode::default()),
        }
    }
}

impl Command for ScalePositionsCommand {
    fn name(&self) -> &'static str {
        "ScalePositions"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn scalar_default(&self) -> Option<Real> {
        Some(self.factor.get())
    }
    fn scale_mode_default(&self) -> Option<ScaleMode> {
        Some(self.mode.get())
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
        let getter_mode = self.mode.get();
        let (arguments, sources) = transform_arguments(document, arguments, USAGE)?;
        let (positional, options) = options(
            &arguments,
            PositionOptions {
                mode: getter_mode,
                copy: false,
            },
        )?;
        let (origin, consumed) = parse_point(&positional)?;
        let tail = &positional[consumed..];
        let first = tail.first().ok_or(CommandError::Usage(USAGE))?;
        let (factor, direction, count) = if first.contains(',') {
            let (reference, a) = parse_point(tail)?;
            let (target, b) = parse_point(&tail[a..])?;
            (
                reference_factor(
                    options.mode,
                    origin,
                    reference,
                    target,
                    document.tolerance(),
                )?,
                Some(reference),
                a + b,
            )
        } else {
            let factor = parse_nonzero_scale(first)?.abs();
            if tail.len() > 1 {
                let (direction, n) = parse_point(&tail[1..])?;
                (factor, Some(direction), 1 + n)
            } else {
                (factor, None, 1)
            }
        };
        require_consumed(tail, count, USAGE)?;
        let map = scale_map(
            options.mode,
            context.construction_plane,
            origin,
            factor,
            direction,
            document.tolerance(),
        )?;
        let Some(map) = map else {
            if factor != 0. {
                self.factor.set(factor);
            }
            self.mode.set(options.mode);
            return Ok("Scaled positions of 0 object(s)".into());
        };
        let count = apply(document, &sources.ids, map, options.copy)?;
        self.factor.set(factor);
        self.mode.set(options.mode);
        Ok(format!(
            "Scaled positions of {count} object(s) in {} by {factor:.6}",
            options.mode.name()
        ))
    }
}

pub fn reference_factor(
    mode: ScaleMode,
    origin: Point3,
    reference: Point3,
    target: Point3,
    tolerance: Tolerance,
) -> Result<Real, CommandError> {
    if mode == ScaleMode::OneDimensional {
        scale1d_factor_from_reference(origin, reference, target, tolerance, true)
    } else {
        scale_factor_from_reference_allow_zero(origin, reference, target, tolerance)
    }
}

/// Numeric direction collection follows the mode at command startup. Changing
/// 3D/2D to 1D before numeric input can therefore complete without a direction
/// and no placement. Complete reference input always supplies its direction.
pub fn scale_map(
    mode: ScaleMode,
    plane: Frame3,
    origin: Point3,
    factor: Real,
    direction: Option<Point3>,
    tolerance: Tolerance,
) -> Result<Option<AffineTransform3>, CommandError> {
    if factor == 0. {
        return Ok(None);
    }
    Ok(Some(match mode {
        ScaleMode::OneDimensional => {
            let Some(direction) = direction else {
                return Ok(None);
            };
            AffineTransform3::try_directional_scale(
                origin,
                origin.vector_to(direction)?.normalized(tolerance)?,
                factor,
            )?
        }
        ScaleMode::TwoDimensional => {
            crate::nonuniform_scale::scale_map(plane, origin, [factor, factor, 1.])?
        }
        ScaleMode::ThreeDimensional => AffineTransform3::try_uniform_scale(origin, factor)?,
    }))
}

fn apply(
    document: &mut Document,
    ids: &[ObjectId],
    transform: AffineTransform3,
    copy: bool,
) -> Result<usize, CommandError> {
    if !copy && transform == AffineTransform3::identity() {
        return Ok(ids.len());
    }
    let layout = RigidLayout::try_individual(document, ids)?;
    let maps = ids
        .iter()
        .map(|id| Ok((*id, rigid_map(layout.center(*id).unwrap(), transform)?)))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    if copy {
        let staged = maps
            .into_iter()
            .map(|(id, map)| {
                Ok((
                    id,
                    document
                        .object(id)
                        .unwrap()
                        .geometry()
                        .transformed_for_edit(map, document.tolerance())?,
                ))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let copies = document.copy_object_geometries_in_source_groups(staged)?;
        for (&source, &copy) in ids.iter().zip(&copies) {
            if document.control_point_locations(source).is_some() {
                let selected = document
                    .selected_control_points()
                    .filter(|(p, _)| p.object == source)
                    .map(|(p, _)| p.index)
                    .collect::<Vec<_>>();
                document.enable_control_points([copy])?;
                document.select_control_points(
                    selected
                        .into_iter()
                        .map(|index| viboceros_document::ControlPointId {
                            object: copy,
                            index,
                        }),
                    SelectionMode::Add,
                )?;
            }
        }
    } else {
        document.transform_objects_individually(maps, ReplacementHistory::EveryReplacement)?;
    }
    Ok(ids.len())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalePositionsPrompt {
    pub origin: Option<Point3>,
    pub reference: Option<Point3>,
    pub factor: Option<Real>,
    pub mode: ScaleMode,
    pub getter_mode: ScaleMode,
    pub choosing_mode: bool,
}

impl ScalePositionsPrompt {
    pub const fn new(mode: ScaleMode) -> Self {
        Self {
            origin: None,
            reference: None,
            factor: None,
            mode,
            getter_mode: mode,
            choosing_mode: false,
        }
    }
    pub const fn prompt(self) -> &'static str {
        if self.choosing_mode {
            "ScalePositions: choose Mode=1D|2D|3D"
        } else if self.origin.is_none() {
            "ScalePositions: pick the origin (Mode=1D|2D|3D; Copy=Yes|No; Esc cancels)"
        } else if self.factor.is_some() {
            "ScalePositions: pick the scale direction"
        } else if self.reference.is_some() {
            "ScalePositions: pick the second reference point"
        } else {
            "ScalePositions: factor or first reference point (Enter accepts default)"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_positions_maps_centers_and_retains_selection_metadata_and_order() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let a = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 3., 4.).unwrap()))
            .unwrap();
        let b = doc
            .add_geometry(Geometry::Point(Point3::try_new(8., 3., 4.).unwrap()))
            .unwrap();
        let group = doc.add_group(None, [a, b]).unwrap();
        doc.select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        doc.set_object_geometry_user_text([a], "Code", Some("geometry"))
            .unwrap();
        registry
            .execute(&mut doc, "ScalePositions 0,0,0 -2")
            .unwrap();
        assert_eq!(
            doc.object(a).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(4., 6., 8.).unwrap())
        );
        assert_eq!(doc.objects().map(|o| o.id()).collect::<Vec<_>>(), [a, b]);
        assert!(doc.is_selected(a));
        assert!(doc.is_selected(b));
        doc.undo().unwrap();
        assert_eq!(
            doc.object(a).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(2., 3., 4.).unwrap())
        );
        registry
            .execute(&mut doc, "ScalePositions 0,0,0 2 0,1,0 Mode=1D Copy=Yes")
            .unwrap();
        let copies = doc.objects().skip(2).map(|o| o.id()).collect::<Vec<_>>();
        assert_eq!(
            doc.object(copies[0]).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(2., 6., 4.).unwrap())
        );
        assert_eq!(
            doc.object(copies[0]).unwrap().geometry_user_text()["Code"],
            "geometry"
        );
        assert_eq!(doc.groups().count(), 1);
        assert_eq!(doc.group(group).unwrap().members().count(), 4);
        assert!(doc.is_selected(a));
        assert!(!doc.is_selected(copies[0]));
        doc.undo().unwrap();
        assert_eq!(doc.objects().count(), 2);
        doc.redo().unwrap();
        assert_eq!(doc.objects().count(), 4);
    }

    #[test]
    fn scale_positions_rejects_invalid_inputs_atomically_without_changing_defaults() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let id = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 0., 0.).unwrap()))
            .unwrap();
        doc.select_all();
        let before = format!("{doc:?}");
        for input in [
            "ScalePositions 0,0,0 0",
            "ScalePositions 0,0,0 NaN",
            "ScalePositions 0,0,0 2 Mode=4D",
            "ScalePositions 0,0,0 2 0,0,0 Mode=1D",
            "ScalePositions 0,0,0 0,0,0 2,0,0",
            "ScalePositions 0,0,0 1e308 Mode=2D",
        ] {
            assert!(registry.execute(&mut doc, input).is_err(), "{input}");
            assert_eq!(format!("{doc:?}"), before);
            assert_eq!(
                registry.transform_scalar_default("ScalePositions"),
                Some(1.)
            );
            assert_eq!(
                registry.scale_mode_default("ScalePositions"),
                Some(ScaleMode::ThreeDimensional)
            );
        }
        registry
            .execute(&mut doc, "ScalePositions 0,0,0 2 Mode=1D")
            .unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::Point(Point3::try_new(2., 0., 0.).unwrap())
        );
        assert_eq!(
            registry.scale_mode_default("ScalePositions"),
            Some(ScaleMode::OneDimensional)
        );
    }
}
