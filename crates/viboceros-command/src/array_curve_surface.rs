//! Rigid arrays along a curve, using an immutable surface for the up direction.
use super::*;
use crate::{curve_reference::CapturedCurve, orient_surface::Target};
use std::sync::Arc;
use viboceros_geometry::{ArcLengthSampler, Curve3, CurveSample, ParameterSide};

pub const USAGE: &str = "ArrayCrvOnSrf base-point (Curve=id|Edge=id,index) Surface=id [Face=index] (Divide=count|Multiple=count,distance|Positions=x,y,z;...|Distances=d,...) [Offset=distance] [SubCrv=start,end] [PickPoint=x,y,z] [Reverse=Yes|No] [Finish=Yes|No]";
#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    Divide(usize),
    Multiple {
        count: usize,
        distance: Real,
        start: Real,
    },
    Positions(Vec<Point3>),
    Distances {
        start: Real,
        deltas: Vec<Real>,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct References {
    pub curve: ObjectId,
    pub edge: Option<usize>,
    pub surface: ObjectId,
    pub face: Option<usize>,
    pub pick: Option<Point3>,
}
impl References {
    pub fn arguments(self) -> String {
        let curve = self.edge.map_or_else(
            || format!("Curve={}", self.curve),
            |edge| format!("Edge={},{}", self.curve, edge),
        );
        format!(
            "{curve} Surface={}{}{}",
            self.surface,
            self.face
                .map_or_else(String::new, |face| format!(" Face={face}")),
            self.pick.map_or_else(String::new, |p| format!(
                " PickPoint={}",
                p.to_array().map(|v| v.to_string()).join(",")
            ))
        )
    }
}
/// Captured guides, exact temporary rail and one prepared arc-length sampler.
#[derive(Debug)]
pub struct Placement {
    pub guide: CapturedCurve,
    pub target: Target,
    pub source: Frame3,
    rail: Arc<Curve3>,
    lengths: ArcLengthSampler<'static>,
    tolerance: Tolerance,
}
impl Placement {
    pub fn try_new(
        doc: &Document,
        base: Point3,
        plane: Frame3,
        refs: References,
        range: Option<[Real; 2]>,
        reverse: bool,
    ) -> Result<Self, CommandError> {
        let guide = CapturedCurve::capture(doc, refs.curve, refs.edge)?;
        let target = Target::capture(doc, refs.surface, refs.face)?;
        let rail = crate::orient_curve::prepare_guide(&guide.curve_ref().to_owned(), range)?;
        let from_end = if let Some(p) = refs.pick {
            p.distance_to(rail.as_ref().start_point()?)?
                > p.distance_to(rail.as_ref().end_point()?)?
        } else {
            false
        };
        let rail = Arc::new(if reverse ^ from_end {
            rail.reversed(doc.tolerance())?
        } else {
            rail
        });
        let mut lengths = ArcLengthSampler::try_new_owned(Arc::clone(&rail), doc.tolerance())?;
        lengths.prepare_budgeted_repeated_sampling(64)?;
        Ok(Self {
            guide,
            target,
            source: plane.with_origin(base),
            rail,
            lengths,
            tolerance: doc.tolerance(),
        })
    }
    pub fn curve(&self) -> CurveRef<'_> {
        self.rail.as_ref().as_ref()
    }
    pub fn cursor_curve(&self) -> Result<NurbsCurve, GeometryError> {
        self.curve().to_nurbs()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.guide.current(doc) && self.target.current(doc)
    }
    pub fn length(&self) -> Real {
        self.lengths.total_length()
    }
    pub fn distance_at(&self, point: Point3) -> Result<Real, CommandError> {
        let parameter = self.curve().closest_parameter(point, self.tolerance)?;
        Ok(self.lengths.distance_at_parameter(parameter)?)
    }
    pub fn point_at_distance(&self, distance: Real) -> Result<Point3, CommandError> {
        let distance = self.bounded_distance(distance)?;
        Ok(self.lengths.point_at_distance(distance)?)
    }
    fn bounded_distance(&self, distance: Real) -> Result<Real, CommandError> {
        if !distance.is_finite() {
            return Err(CommandError::Usage(USAGE));
        }
        if (0. ..=self.length()).contains(&distance) {
            Ok(distance)
        } else {
            Err(CommandError::Usage(
                "ArrayCrvOnSrf distance must lie on the curve",
            ))
        }
    }
    fn transform_sample(&self, sample: CurveSample) -> Result<AffineTransform3, CommandError> {
        let normal = self.target.source_normal(sample.point(), true)?;
        let y = normal
            .cross(sample.tangent().as_vector())?
            .normalized_nonzero()?;
        let frame =
            Frame3::try_from_unit_directions(sample.point(), sample.tangent(), y, self.tolerance)?;
        Ok(AffineTransform3::try_frame_mapping(
            self.source,
            frame,
            [1.; 3],
        )?)
    }
    pub fn transform_at(&self, point: Point3) -> Result<AffineTransform3, CommandError> {
        let parameter = self.curve().closest_parameter(point, self.tolerance)?;
        self.transform_sample(
            self.curve()
                .evaluate_with_tangent_on_side(parameter, ParameterSide::Right)?,
        )
    }
    pub fn transforms(
        &self,
        layout: &Layout,
        source_count: usize,
    ) -> Result<Vec<AffineTransform3>, CommandError> {
        let invalid = || CommandError::TooManyArrayObjects {
            maximum: MAX_ARRAY_OBJECTS,
        };
        let check = |count: usize| {
            count
                .checked_mul(source_count.max(1))
                .filter(|n| *n <= MAX_ARRAY_OBJECTS)
                .ok_or_else(invalid)
        };
        let samples = match layout {
            Layout::Divide(n) => {
                if *n < 2 {
                    return Err(CommandError::Usage(USAGE));
                }
                check(*n)?;
                super::curve_array_samples(
                    self.curve(),
                    CurveArraySpacing::Items(*n),
                    self.tolerance,
                )?
            }
            Layout::Multiple {
                count,
                distance,
                start,
            } => {
                if *count < 2 || !distance.is_finite() || *distance < 0. {
                    return Err(CommandError::Usage(USAGE));
                }
                check(*count)?;
                let mut samples = Vec::new();
                if *distance != 0. {
                    for i in 1..=*count {
                        let distance = start + i as Real * distance;
                        if !(0. ..=self.length()).contains(&distance) {
                            break;
                        }
                        samples.push(self.lengths.sample_at_distance(distance)?);
                    }
                }
                samples
            }
            Layout::Positions(points) => {
                check(points.len())?;
                points
                    .iter()
                    .map(|p| {
                        let parameter = self.curve().closest_parameter(*p, self.tolerance)?;
                        self.curve()
                            .evaluate_with_tangent_on_side(parameter, ParameterSide::Right)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?
            }
            Layout::Distances { start, deltas } => {
                check(deltas.len())?;
                let mut distance = *start;
                deltas
                    .iter()
                    .map(|delta| {
                        distance += delta;
                        let station = self.bounded_distance(distance)?;
                        Ok(self.lengths.sample_at_distance(station)?)
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?
            }
        };
        if samples.is_empty() && !matches!(layout, Layout::Multiple { .. }) {
            return Err(CommandError::Usage(USAGE));
        }
        check(samples.len())?;
        samples
            .into_iter()
            .map(|sample| self.transform_sample(sample))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub count: usize,
    pub distance: Real,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            count: 1,
            distance: 1.,
        }
    }
}
#[derive(Default)]
pub struct ArrayCurveSurfaceCommand {
    preferences: std::sync::Mutex<Preferences>,
}

impl Command for ArrayCurveSurfaceCommand {
    fn name(&self) -> &'static str {
        "ArrayCrvOnSrf"
    }
    fn array_curve_surface_preferences(&self) -> Option<Preferences> {
        Some(*self.preferences.lock().unwrap_or_else(|e| e.into_inner()))
    }
    fn remember_array_curve_surface_preferences(&self, value: Preferences) -> bool {
        if value.count == 0
            || value.count > MAX_ARRAY_OBJECTS
            || !value.distance.is_finite()
            || value.distance < 0.
        {
            return false;
        }
        *self.preferences.lock().unwrap_or_else(|e| e.into_inner()) = value;
        true
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ArrayCurveOnSurface"]
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
        let mut preferences = self.array_curve_surface_preferences().unwrap();
        let (args, mut sources) = history_policy::transform_arguments(doc, args, USAGE)?;
        let (args, range) = crate::orient_curve::range_arguments(&args)?;
        let (base, used) = parse_point(&args)?;
        let mut curve = None;
        let mut target = None;
        let mut face = None;
        let mut pick = None;
        let mut layout = None;
        let mut offset = 0.;
        let mut reverse = false;
        let mut finish = true;
        let mut seen = BTreeSet::new();
        let mut i = used;
        while i < args.len() {
            let (name, value, count) = orient_option(&args, i, USAGE)?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let key = if name == "edge" {
                "curve"
            } else if ["divide", "multiple", "positions", "distances"].contains(&name.as_str()) {
                "layout"
            } else {
                &name
            };
            if !seen.insert(key.to_owned()) {
                return Err(CommandError::Usage(USAGE));
            }
            match name.as_str() {
                "curve" => {
                    curve = Some((value.parse().map_err(|_| CommandError::Usage(USAGE))?, None))
                }
                "edge" => {
                    curve = Some(
                        crate::curve_reference::parse_edge(value)
                            .map(|(id, index)| (id, Some(index)))
                            .ok_or(CommandError::Usage(USAGE))?,
                    )
                }
                "surface" => target = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                "face" => face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                "divide" => {
                    let n = if value.eq_ignore_ascii_case("Default") {
                        preferences.count
                    } else {
                        value
                            .parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))?
                    };
                    if n < 2 {
                        return Err(CommandError::Usage("Divide requires at least two objects"));
                    }
                    preferences.count = n;
                    layout = Some(Layout::Divide(n));
                }
                "multiple" => {
                    let (count, distance) =
                        value.split_once(',').ok_or(CommandError::Usage(USAGE))?;
                    let count = if count.eq_ignore_ascii_case("Default") {
                        preferences.count
                    } else {
                        count
                            .parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))?
                    };
                    if count < 2 {
                        return Err(CommandError::Usage("Multiple requires at least two copies"));
                    }
                    let distance = if distance.eq_ignore_ascii_case("Default") {
                        preferences.distance
                    } else {
                        parse_finite_real(distance)?
                    };
                    if distance < 0. {
                        return Err(CommandError::Usage(
                            "Distance between copies must be nonnegative",
                        ));
                    }
                    preferences = Preferences { count, distance };
                    layout = Some(Layout::Multiple {
                        count,
                        distance,
                        start: 0.,
                    });
                }
                "pickpoint" => pick = Some(parse_single_option_point(value, USAGE)?),
                "positions" => {
                    if value.split(';').count() > MAX_ARRAY_OBJECTS {
                        return Err(CommandError::TooManyArrayObjects {
                            maximum: MAX_ARRAY_OBJECTS,
                        });
                    }
                    layout = Some(Layout::Positions(
                        value
                            .split(';')
                            .map(|p| parse_single_option_point(p, USAGE))
                            .collect::<Result<Vec<_>, _>>()?,
                    ));
                }
                "distances" => {
                    if value.split(',').count() > MAX_ARRAY_OBJECTS {
                        return Err(CommandError::TooManyArrayObjects {
                            maximum: MAX_ARRAY_OBJECTS,
                        });
                    }
                    layout = Some(Layout::Distances {
                        start: 0.,
                        deltas: value
                            .split(',')
                            .map(parse_finite_real)
                            .collect::<Result<Vec<_>, _>>()?,
                    });
                }
                "offset" => offset = parse_finite_real(value)?,
                "reverse" => reverse = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "finish" => finish = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                _ => return Err(CommandError::Usage(USAGE)),
            }
            i += count;
        }
        let mut layout = layout.ok_or(CommandError::Usage(USAGE))?;
        if let Layout::Distances { start, .. } | Layout::Multiple { start, .. } = &mut layout {
            *start = offset;
        } else if seen.contains("offset") {
            return Err(CommandError::Usage(USAGE));
        }
        let (curve, edge) = curve.ok_or(CommandError::Usage(USAGE))?;
        let refs = References {
            curve,
            edge,
            surface: target.ok_or(CommandError::Usage(USAGE))?,
            face,
            pick,
        };
        sources
            .ids
            .retain(|id| *id != refs.curve && *id != refs.surface);
        if sources.ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let placement =
            Placement::try_new(doc, base, context.construction_plane, refs, range, reverse)?;
        self.remember_array_curve_surface_preferences(preferences);
        let transforms = placement.transforms(&layout, sources.ids.len())?;
        let copies = if transforms.is_empty() {
            Vec::new()
        } else {
            if sources.release_on_replay || range.is_some() {
                doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            }
            doc.retain_created_group_definitions_on_undo()?;
            super::arrays::copy_array_objects_in_order(
                doc,
                &sources.ids,
                &transforms,
                sources.postselected,
            )?
        };
        if range.is_some() || finish && sources.postselected {
            doc.clear_selection();
        }
        Ok(format!(
            "Arrayed {} object(s) at {} surface-curve locations, creating {} copies",
            sources.ids.len(),
            transforms.len(),
            copies.len()
        ))
    }
}
