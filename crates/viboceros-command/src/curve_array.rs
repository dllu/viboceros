//! Curve arrays share arc-length sampling and curve frame transport.
use super::*;
const ARRAY_CURVE_USAGE: &str = "ArrayCrv item-count | ArrayCrv Items item-count | \
    ArrayCrv Distance spacing [Orientation=Freeform|Roadlike|Stairlike|NoRotation] \
    [BasePoint=x,y,z] [PathName=name|Curve=id|Edge=id,index] [SubCrv=start,end] [PickPoint=x,y,z] [Reverse=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub items: usize,
    pub distance: Real,
    pub by_distance: bool,
    pub orientation: CurveArrayOrientation,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            items: 2,
            distance: 1.,
            by_distance: false,
            orientation: CurveArrayOrientation::Freeform,
        }
    }
}
impl Preferences {
    pub fn spacing(self) -> CurveArraySpacing {
        if self.by_distance {
            CurveArraySpacing::Distance(self.distance)
        } else {
            CurveArraySpacing::Items(self.items)
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Reference {
    pub curve: ObjectId,
    pub edge: Option<usize>,
    pub pick: Option<Point3>,
}
impl Reference {
    pub fn arguments(self) -> String {
        format!(
            "{}{}",
            self.edge.map_or_else(
                || format!("Curve={}", self.curve),
                |edge| format!("Edge={},{}", self.curve, edge)
            ),
            self.pick.map_or_else(String::new, |p| format!(
                " PickPoint={}",
                p.to_array().map(|v| v.to_string()).join(",")
            ))
        )
    }
}
/// One immutable rail is shared by complete invocations and read-only previews.
pub struct Placement {
    pub guide: curve_reference::CapturedCurve,
    curve: viboceros_geometry::Curve3,
    base: Option<Point3>,
    plane: Frame3,
    tolerance: Tolerance,
}
impl Placement {
    pub fn try_new(
        doc: &Document,
        sources: &[ObjectId],
        reference: Reference,
        range: Option<[Real; 2]>,
        base: Option<Point3>,
        plane: Frame3,
        reverse: bool,
    ) -> Result<Self, CommandError> {
        let guide = curve_reference::CapturedCurve::capture(doc, reference.curve, reference.edge)?;
        let mut curve = orient_curve::prepare_guide(&guide.curve_ref().to_owned(), range)?;
        if curve.as_ref().is_closed()? {
            let anchor = if let Some(base) = base {
                base
            } else {
                let bounds = sources
                    .iter()
                    .map(|id| {
                        doc.object(*id)
                            .ok_or(DocumentError::ObjectNotFound(*id))
                            .map_err(CommandError::from)
                            .and_then(|o| Ok(o.geometry().tight_bounds(doc.tolerance())?))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let Some(first) = bounds.first() else {
                    return Err(CommandError::CurveArraySourcesRequired);
                };
                bounds[1..]
                    .iter()
                    .try_fold(*first, |b, other| b.union(*other))?
                    .center()?
            };
            let parameter = curve.as_ref().closest_parameter(anchor, doc.tolerance())?;
            curve = curve_segments::seam(&curve, parameter)?;
        }
        let from_end = if let Some(p) = reference.pick {
            !curve.as_ref().is_closed()? && {
                let domain = curve.as_ref().domain();
                curve.as_ref().closest_parameter(p, doc.tolerance())?
                    > domain.start() + (domain.end() - domain.start()) * 0.5
            }
        } else {
            false
        };
        let curve = if reverse ^ from_end {
            curve.reversed(doc.tolerance())?
        } else {
            curve
        };
        Ok(Self {
            guide,
            curve,
            base,
            plane,
            tolerance: doc.tolerance(),
        })
    }
    pub fn curve(&self) -> CurveRef<'_> {
        self.curve.as_ref()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.guide.current(doc)
    }
    pub fn spacing_preferences(&self, mut p: Preferences) -> Result<Preferences, CommandError> {
        let length = self.curve().length(self.tolerance)?;
        if p.by_distance {
            p.items = curve_array_samples(self.curve(), p.spacing(), self.tolerance)?.len();
        } else {
            p.distance = length
                / if self.curve().is_closed()? {
                    p.items.max(1)
                } else {
                    p.items.saturating_sub(1).max(1)
                } as Real;
        }
        Ok(p)
    }
    pub fn transforms(
        &self,
        spacing: CurveArraySpacing,
        orientation: CurveArrayOrientation,
        source_count: usize,
    ) -> Result<Vec<AffineTransform3>, CommandError> {
        let skip = usize::from(self.base.is_none());
        let check = |n: usize| {
            n.checked_mul(source_count.max(1))
                .filter(|n| *n <= MAX_ARRAY_OBJECTS)
                .ok_or(CommandError::TooManyArrayObjects {
                    maximum: MAX_ARRAY_OBJECTS,
                })
        };
        if let CurveArraySpacing::Items(n) = spacing {
            if n == 0 {
                return Err(CommandError::InvalidCurveArrayItemCount(n.to_string()));
            }
            check(n - skip)?;
        }
        let samples = curve_array_samples(self.curve(), spacing, self.tolerance)?;
        check(samples.len() - skip)?;
        if samples.len() == skip {
            return Ok(Vec::new());
        }
        Ok(curve_array_transforms(
            self.curve(),
            &samples,
            self.base.unwrap_or_else(|| samples[0].point()),
            orientation,
            skip,
            self.plane,
            self.tolerance,
        )?)
    }
}
#[derive(Default)]
pub(super) struct ArrayCurveCommand(remembered::Remembered<Preferences>);
impl Command for ArrayCurveCommand {
    fn name(&self) -> &'static str {
        "ArrayCrv"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ArrayCurve"]
    }
    fn curve_array_preferences(&self) -> Option<Preferences> {
        Some(self.0.get())
    }
    fn remember_curve_array_preferences(&self, p: Preferences) -> bool {
        if p.items == 0
            || p.items > MAX_ARRAY_OBJECTS + 1
            || !p.distance.is_finite()
            || p.distance <= 0.
        {
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
        let (args, mut sources) =
            history_policy::transform_arguments(doc, args, ARRAY_CURVE_USAGE)?;
        let (args, range) = orient_curve::range_arguments(&args)?;
        let mut prefs = self.0.get();
        let first = args.first().ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?;
        let (spacing, mut i) = if let Some((name, value)) = first.split_once('=') {
            (parse_curve_array_spacing(name, value)?, 1)
        } else if option_name_eq(first, "Items") || option_name_eq(first, "Distance") {
            (
                parse_curve_array_spacing(
                    first,
                    args.get(1).ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?,
                )?,
                2,
            )
        } else {
            (
                CurveArraySpacing::Items(parse_curve_array_item_count(first)?),
                1,
            )
        };
        let mut base = None;
        let mut target = None;
        let mut edge = None;
        let mut pick = None;
        let mut reverse = false;
        let mut seen = BTreeSet::new();
        while i < args.len() {
            let (name, value, n) = orient_option(&args, i, ARRAY_CURVE_USAGE)?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let key = match name.as_str() {
                "curve" | "edge" | "pathname" | "path" => "path",
                _ => &name,
            };
            if !seen.insert(key.to_owned()) {
                return Err(CommandError::Usage(ARRAY_CURVE_USAGE));
            }
            match name.as_str() {
                "orientation" => prefs.orientation = parse_curve_array_orientation(value)?,
                "basepoint" => base = Some(parse_single_option_point(value, ARRAY_CURVE_USAGE)?),
                "curve" | "path" => {
                    target = Some(
                        value
                            .parse()
                            .map_err(|_| CommandError::Usage(ARRAY_CURVE_USAGE))?,
                    )
                }
                "edge" => {
                    let (id, index) = curve_reference::parse_edge(value)
                        .ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?;
                    target = Some(id);
                    edge = Some(index);
                }
                "pickpoint" => pick = Some(parse_single_option_point(value, ARRAY_CURVE_USAGE)?),
                "reverse" => {
                    reverse = parse_yes_no(value).ok_or(CommandError::Usage(ARRAY_CURVE_USAGE))?
                }
                "pathname" => {
                    let matches = doc
                        .objects()
                        .filter(|o| {
                            o.attributes()
                                .name()
                                .is_some_and(|name| name.eq_ignore_ascii_case(value))
                        })
                        .map(|o| o.id())
                        .collect::<Vec<_>>();
                    target = Some(match matches.as_slice() {
                        [] => return Err(CommandError::CurveArrayPathNotFound(value.into())),
                        [id] => *id,
                        _ => return Err(CommandError::AmbiguousCurveArrayPath(value.into())),
                    });
                }
                _ => return Err(CommandError::Usage(ARRAY_CURVE_USAGE)),
            }
            i += n;
        }
        let id = target
            .or_else(|| sources.ids.last().copied())
            .ok_or(CommandError::CurveArrayPathRequired)?;
        sources.ids.retain(|source| *source != id);
        if sources.ids.is_empty() {
            return Err(CommandError::CurveArraySourcesRequired);
        }
        let placement = Placement::try_new(
            doc,
            &sources.ids,
            Reference {
                curve: id,
                edge,
                pick,
            },
            range,
            base,
            context.construction_plane,
            reverse,
        )?;
        match spacing {
            CurveArraySpacing::Items(n) => {
                prefs.items = n;
                prefs.by_distance = false;
            }
            CurveArraySpacing::Distance(d) => {
                prefs.distance = d;
                prefs.by_distance = true;
            }
        }
        let maps = placement.transforms(spacing, prefs.orientation, sources.ids.len())?;
        prefs = placement.spacing_preferences(prefs)?;
        self.remember_curve_array_preferences(prefs);
        let copies = if maps.is_empty() {
            Vec::new()
        } else {
            if sources.release_on_replay || range.is_some() {
                doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            }
            doc.retain_created_group_definitions_on_undo()?;
            arrays::copy_array_objects_in_order(doc, &sources.ids, &maps, sources.postselected)?
        };
        if sources.postselected || range.is_some() {
            doc.clear_selection();
        }
        Ok(format!(
            "Arrayed {} object(s) at {} {} location(s) with {} orientation, creating {} copy object(s)",
            sources.ids.len(),
            maps.len() + usize::from(base.is_none()),
            spacing.name(),
            prefs.orientation.name(),
            copies.len()
        ))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveArraySpacing {
    Items(usize),
    Distance(Real),
}

impl CurveArraySpacing {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Items(_) => "item",
            Self::Distance(_) => "distance",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveArrayOrientation {
    Freeform,
    Roadlike,
    Stairlike,
    NoRotation,
}

impl CurveArrayOrientation {
    pub fn parse(value: &str) -> Option<Self> {
        parse_curve_array_orientation(value).ok()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Freeform => "Freeform",
            Self::Roadlike => "Roadlike",
            Self::Stairlike => "Stairlike",
            Self::NoRotation => "NoRotation",
        }
    }
}

fn parse_curve_array_spacing(name: &str, value: &str) -> Result<CurveArraySpacing, CommandError> {
    if option_name_eq(name, "Items") {
        Ok(CurveArraySpacing::Items(parse_curve_array_item_count(
            value,
        )?))
    } else if option_name_eq(name, "Distance") {
        let distance = parse_finite_real(value)?;
        if distance <= 0.0 {
            return Err(CommandError::InvalidCurveArrayDistance(value.to_owned()));
        }
        Ok(CurveArraySpacing::Distance(distance))
    } else {
        Err(CommandError::Usage(ARRAY_CURVE_USAGE))
    }
}

fn parse_curve_array_item_count(value: &str) -> Result<usize, CommandError> {
    value
        .parse::<usize>()
        .ok()
        .filter(|count| *count >= 1)
        .ok_or_else(|| CommandError::InvalidCurveArrayItemCount(value.to_owned()))
}

fn parse_curve_array_orientation(value: &str) -> Result<CurveArrayOrientation, CommandError> {
    let value = value.trim_start_matches(['_', '-']);
    if value.eq_ignore_ascii_case("Freeform") {
        Ok(CurveArrayOrientation::Freeform)
    } else if value.eq_ignore_ascii_case("Roadlike") {
        Ok(CurveArrayOrientation::Roadlike)
    } else if value.eq_ignore_ascii_case("Stairlike") {
        Ok(CurveArrayOrientation::Stairlike)
    } else if value.eq_ignore_ascii_case("NoRotation") {
        Ok(CurveArrayOrientation::NoRotation)
    } else {
        Err(CommandError::Usage(ARRAY_CURVE_USAGE))
    }
}

pub(super) fn curve_array_samples(
    curve: CurveRef<'_>,
    spacing: CurveArraySpacing,
    tolerance: Tolerance,
) -> Result<Vec<CurveSample>, CommandError> {
    let closed = curve.is_closed()?;
    let mut samples = match spacing {
        CurveArraySpacing::Items(1) => vec![curve.start_sample(tolerance)?],
        CurveArraySpacing::Items(item_count) if closed => {
            let mut samples = curve.divide_by_count_samples(item_count, true, tolerance)?;
            samples.pop();
            samples
        }
        CurveArraySpacing::Items(item_count) => {
            curve.divide_by_count_samples(item_count - 1, true, tolerance)?
        }
        CurveArraySpacing::Distance(distance) => {
            let mut samples = curve.divide_by_length_samples(distance, true, tolerance)?;
            if closed
                && samples.len() > 1
                && samples[0].point().is_near(
                    samples.last().expect("the start sample is present").point(),
                    tolerance,
                )
            {
                samples.pop();
            }
            samples
        }
    };
    if samples.is_empty() {
        samples.push(curve.start_sample(tolerance)?);
    }
    Ok(samples)
}

fn curve_array_transforms(
    curve: CurveRef<'_>,
    samples: &[CurveSample],
    base_point: Point3,
    orientation: CurveArrayOrientation,
    skip: usize,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<AffineTransform3>, GeometryError> {
    if orientation == CurveArrayOrientation::NoRotation {
        return samples[skip..]
            .iter()
            .map(|sample| {
                base_point
                    .vector_to(sample.point())
                    .map(AffineTransform3::from_translation)
            })
            .collect();
    }
    let frames = curve_array_frames(curve, samples, orientation, plane, tolerance)?;
    let source_frame = frames[0];
    samples[skip..]
        .iter()
        .zip(&frames[skip..])
        .map(|(sample, frame)| {
            curve_array_frame_transform(base_point, source_frame, sample.point(), *frame, tolerance)
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct CurveArrayFrame {
    x: UnitVector3,
    y: UnitVector3,
}

fn curve_array_frames(
    curve: CurveRef<'_>,
    samples: &[CurveSample],
    orientation: CurveArrayOrientation,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<CurveArrayFrame>, GeometryError> {
    match orientation {
        CurveArrayOrientation::Freeform => freeform_curve_array_frames(curve, samples),
        CurveArrayOrientation::Roadlike => {
            let mut previous = None;
            samples
                .iter()
                .map(|sample| {
                    let frame = roadlike_curve_array_frame(
                        curve
                            .evaluate_with_tangent_on_side(
                                sample.parameter(),
                                viboceros_geometry::ParameterSide::Right,
                            )?
                            .tangent(),
                        previous,
                        plane,
                        tolerance,
                    )?;
                    previous = Some(frame);
                    Ok(frame)
                })
                .collect()
        }
        CurveArrayOrientation::Stairlike => {
            let mut previous = None;
            samples
                .iter()
                .map(|sample| {
                    let frame = stairlike_curve_array_frame(
                        curve
                            .evaluate_with_tangent_on_side(
                                sample.parameter(),
                                viboceros_geometry::ParameterSide::Right,
                            )?
                            .tangent(),
                        previous,
                        plane,
                        tolerance,
                    )?;
                    previous = Some(frame);
                    Ok(frame)
                })
                .collect()
        }
        CurveArrayOrientation::NoRotation => unreachable!("handled before frame construction"),
    }
}

fn freeform_curve_array_frames(
    curve: CurveRef<'_>,
    samples: &[CurveSample],
) -> Result<Vec<CurveArrayFrame>, GeometryError> {
    let parameters = samples.iter().map(|s| s.parameter()).collect::<Vec<_>>();
    Ok(crate::curve_frames::perpendicular_frames(
        curve,
        &parameters,
        samples.len().saturating_mul(8).max(131_072),
    )?
    .into_iter()
    .map(|f| CurveArrayFrame {
        x: f.x_axis(),
        y: f.y_axis(),
    })
    .collect())
}

fn roadlike_curve_array_frame(
    tangent: UnitVector3,
    previous: Option<CurveArrayFrame>,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<CurveArrayFrame, GeometryError> {
    let up = plane.z_axis();
    let horizontal_y = up.as_vector().cross(tangent.as_vector())?;
    let provisional_y = if horizontal_y.length()? > tolerance.angular() {
        horizontal_y.normalized_nonzero()?
    } else if let Some(frame) = previous {
        frame.y
    } else {
        plane.y_axis()
    };
    let frame_x = provisional_y
        .as_vector()
        .cross(tangent.as_vector())?
        .normalized_nonzero()?;
    let frame_y = tangent
        .as_vector()
        .cross(frame_x.as_vector())?
        .normalized_nonzero()?;
    Ok(CurveArrayFrame {
        x: frame_x,
        y: frame_y,
    })
}

fn stairlike_curve_array_frame(
    tangent: UnitVector3,
    previous: Option<CurveArrayFrame>,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<CurveArrayFrame, GeometryError> {
    let up = plane.z_axis();
    let horizontal_tangent = subtract_vectors(
        tangent.as_vector(),
        up.as_vector()
            .scaled(tangent.as_vector().dot(up.as_vector())?)?,
    )?;
    let frame_x = if horizontal_tangent.length()? > tolerance.angular() {
        horizontal_tangent.normalized_nonzero()?
    } else if let Some(frame) = previous {
        frame.x
    } else {
        plane.x_axis()
    };
    let frame_y = up
        .as_vector()
        .cross(frame_x.as_vector())?
        .normalized_nonzero()?;
    Ok(CurveArrayFrame {
        x: frame_x,
        y: frame_y,
    })
}

fn curve_array_frame_transform(
    base_point: Point3,
    source: CurveArrayFrame,
    target_point: Point3,
    target: CurveArrayFrame,
    tolerance: Tolerance,
) -> Result<AffineTransform3, GeometryError> {
    let source = Frame3::try_from_unit_directions(base_point, source.x, source.y, tolerance)?;
    let target = Frame3::try_from_unit_directions(target_point, target.x, target.y, tolerance)?;
    AffineTransform3::try_frame_mapping(source, target, [1.0; 3])
}
