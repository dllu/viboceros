//! Arrays over immutable underlying surface charts, shared by scripts and previews.
use super::*;
use crate::orient_surface::Target;

pub const USAGE: &str = "ArraySrf u-count v-count BasePoint=x,y,z [Up=x,y,z] [Mode=UV|Isocurve] [Surface=id|SurfaceName=name] [Face=index] [Preview=Yes|No]";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Uv,
    Isocurve,
}
impl Mode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uv => "UV",
            Self::Isocurve => "Isocurve",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "uv" => Some(Self::Uv),
            "isocurve" => Some(Self::Isocurve),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub counts: [usize; 2],
    pub mode: Mode,
    pub preview: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            counts: [2, 2],
            mode: Mode::Uv,
            preview: false,
        }
    }
}
#[derive(Debug)]
pub struct Placement {
    pub target: Target,
    pub source: Frame3,
    tolerance: Tolerance,
}
impl Placement {
    pub fn try_new(
        doc: &Document,
        id: ObjectId,
        face: Option<usize>,
        base: Point3,
        up: Option<UnitVector3>,
        plane: Frame3,
    ) -> Result<Self, CommandError> {
        let source = if let Some(up) = up {
            Frame3::try_from_unit_normal(base, up)?
        } else {
            plane.with_origin(base)
        };
        Ok(Self {
            target: Target::capture(doc, id, face)?,
            source,
            tolerance: doc.tolerance(),
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.target.current(doc)
    }
    pub fn transforms(
        &self,
        counts: [usize; 2],
        mode: Mode,
        source_count: usize,
    ) -> Result<Vec<AffineTransform3>, CommandError> {
        if counts.contains(&0) {
            return Err(CommandError::Usage(USAGE));
        }
        let too_many = || CommandError::TooManyArrayObjects {
            maximum: MAX_ARRAY_OBJECTS,
        };
        let count = counts[0]
            .checked_mul(counts[1])
            .filter(|n| {
                n.checked_mul(source_count.max(1))
                    .is_some_and(|n| n <= MAX_ARRAY_OBJECTS)
            })
            .ok_or_else(too_many)?;
        let surface = &self.target.surface;
        let (u, v) = parameters(surface, counts, mode, self.tolerance)?;
        let mut samples = Vec::new();
        samples.try_reserve_exact(count).map_err(|_| too_many())?;
        for u in u {
            let v = if mode == Mode::Isocurve && counts[1] > 1 {
                surface
                    .divide_v_isocurve_by_count(
                        u,
                        if surface.is_closed_v()? {
                            counts[1]
                        } else {
                            counts[1] - 1
                        },
                        true,
                        self.tolerance,
                    )?
                    .into_iter()
                    .take(counts[1])
                    .collect::<Vec<_>>()
            } else {
                v.clone()
            };
            samples.extend(v.into_iter().map(|v| [u, v]));
        }
        let frames = surface.limit_frames_at(&samples)?;
        let maps = frames
            .into_iter()
            .map(|frame| {
                let frame = frame.ok_or(GeometryError::Degenerate {
                    context: "surface array frame",
                })?;
                Ok(AffineTransform3::try_frame_mapping(
                    self.source,
                    frame,
                    [1., 1., if self.target.reversed { -1. } else { 1. }],
                )?)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(maps)
    }
}
#[derive(Default)]
pub(super) struct ArraySurfaceCommand(remembered::Remembered<Preferences>);
impl Command for ArraySurfaceCommand {
    fn name(&self) -> &'static str {
        "ArraySrf"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ArraySurface"]
    }
    fn surface_array_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_surface_array_preferences(&self, p: Preferences) -> bool {
        if p.counts.iter().any(|n| *n == 0 || *n > MAX_ARRAY_OBJECTS) {
            return false;
        }
        self.0.set(p);
        true
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
        let (args, mut sources) = history_policy::transform_arguments(doc, args, USAGE)?;
        let mut prefs = self.0.get();
        let u = parse_surface_array_count(args.first().ok_or(CommandError::Usage(USAGE))?)?;
        let v = parse_surface_array_count(args.get(1).ok_or(CommandError::Usage(USAGE))?)?;
        let mut base = None;
        let mut up = None;
        let mut target = None;
        let mut face = None;
        let mut seen = BTreeSet::new();
        let mut i = 2;
        while i < args.len() {
            let (name, value, used) = orient_option(&args, i, USAGE)?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let key = if name == "normal" {
                "up"
            } else if name == "surfacename" {
                "surface"
            } else {
                &name
            };
            if !seen.insert(key.to_owned()) {
                return Err(CommandError::Usage(USAGE));
            }
            match name.as_str() {
                "basepoint" => base = Some(parse_single_option_point(value, USAGE)?),
                "up" | "normal" => {
                    up = Some(
                        Vector3::try_from(parse_single_option_point(value, USAGE)?.to_array())?
                            .normalized_nonzero()?,
                    )
                }
                "mode" => prefs.mode = Mode::parse(value).ok_or(CommandError::Usage(USAGE))?,
                "preview" => {
                    prefs.preview = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "surface" => target = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                "face" => face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                "surfacename" => {
                    let matches = doc
                        .objects()
                        .filter(|o| {
                            o.attributes()
                                .name()
                                .is_some_and(|n| n.eq_ignore_ascii_case(value))
                        })
                        .map(|o| o.id())
                        .collect::<Vec<_>>();
                    target = Some(match matches.as_slice() {
                        [] => return Err(CommandError::SurfaceArrayTargetNotFound(value.into())),
                        [id] => *id,
                        _ => return Err(CommandError::AmbiguousSurfaceArrayTarget(value.into())),
                    });
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
            i += used;
        }
        let id = target
            .or_else(|| sources.ids.last().copied())
            .ok_or(CommandError::SurfaceArrayTargetRequired)?;
        sources.ids.retain(|source| *source != id);
        if sources.ids.is_empty() {
            return Err(CommandError::SurfaceArraySourcesRequired);
        }
        let placement = Placement::try_new(
            doc,
            id,
            face,
            base.ok_or(CommandError::Usage(USAGE))?,
            up,
            context.construction_plane,
        )?;
        prefs.counts = [u, v];
        self.remember_surface_array_preferences(prefs);
        let maps = placement.transforms(prefs.counts, prefs.mode, sources.ids.len())?;
        let copies = if maps.is_empty() {
            Vec::new()
        } else {
            if sources.release_on_replay {
                doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            }
            doc.retain_created_group_definitions_on_undo()?;
            arrays::copy_array_objects_in_order(doc, &sources.ids, &maps, sources.postselected)?
        };
        if sources.postselected {
            doc.clear_selection();
        }
        Ok(format!(
            "Arrayed {} object(s) into {u}×{v} surface cells using {} spacing, creating {} copy object(s)",
            sources.ids.len(),
            prefs.mode.name(),
            copies.len()
        ))
    }
}
fn parse_surface_array_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 1)
        .ok_or_else(|| CommandError::InvalidSurfaceArrayCount(value.to_owned()))
}

fn parameters(
    surface: &NurbsSurface,
    counts: [usize; 2],
    mode: Mode,
    tolerance: Tolerance,
) -> Result<(Vec<Real>, Vec<Real>), GeometryError> {
    let [u_count, v_count] = counts;
    let u_closed = surface.is_closed_u()?;
    let v_closed = surface.is_closed_v()?;
    let u_start = *surface.domain_u().start();
    let v_start = *surface.domain_v().start();
    if mode == Mode::Isocurve && u_count > 1 {
        let curve = surface.isocurve_u(v_start)?;
        let first = curve.control_points()[0].point();
        if curve.control_points().iter().all(|p| p.point() == first) {
            return Ok((Vec::new(), Vec::new()));
        }
    }
    let parameters = match mode {
        Mode::Uv => (
            normalized_surface_parameters(u_count, u_closed, |normalized| {
                surface.parameter_at_u(normalized)
            })?,
            normalized_surface_parameters(v_count, v_closed, |normalized| {
                surface.parameter_at_v(normalized)
            })?,
        ),
        Mode::Isocurve => (
            if u_count == 1 {
                vec![u_start]
            } else {
                surface
                    .divide_u_isocurve_by_count(
                        v_start,
                        if u_closed { u_count } else { u_count - 1 },
                        true,
                        tolerance,
                    )?
                    .into_iter()
                    .take(u_count)
                    .collect()
            },
            vec![v_start],
        ),
    };
    Ok(parameters)
}

fn normalized_surface_parameters(
    count: usize,
    closed: bool,
    mut parameter_at: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Vec<Real>, GeometryError> {
    if count == 1 {
        return Ok(vec![parameter_at(0.0)?]);
    }
    (0..count)
        .map(|index| {
            parameter_at(
                index as Real
                    / if closed {
                        count as Real
                    } else {
                        (count - 1) as Real
                    },
            )
        })
        .collect()
}
