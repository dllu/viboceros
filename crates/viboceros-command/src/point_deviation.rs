//! Read-only closest-point analysis, shared by the console and viewport panel.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{FiniteSum, MeshProjectionIndex};
mod query;
#[cfg(test)]
mod tests;
pub const USAGE: &str = "PointDeviation [Sources=id,...] [Targets=id,...] [Good=distance] [Bad=distance] [Ignore=distance] [ProximityAngle=degrees] [HairScale=value] [DisplayHair=Yes|No] [IgnoreBad=Yes|No] [CurveDivisions=count] [SurfaceSamples=count] [Visible=Yes|No|Toggle] | PointDeviation End|Apply|MakePermanent";
pub const MAX_SAMPLES: usize = 1_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub good: Real,
    pub bad: Real,
    pub ignore: Real,
    pub proximity_angle: Real,
    pub hair_scale: Real,
    pub display_hair: bool,
    pub ignore_bad: bool,
    pub curve_divisions: usize,
    pub surface_samples: usize,
    pub good_color: [u8; 3],
    pub bad_color: [u8; 3],
    pub automatic: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            good: 0.01,
            bad: 0.1,
            ignore: 1.,
            proximity_angle: 1.,
            hair_scale: 10.,
            display_hair: true,
            ignore_bad: false,
            curve_divisions: 100,
            surface_samples: 8,
            good_color: [0, 0, 255],
            bad_color: [255, 0, 0],
            automatic: true,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), CommandError> {
        if [
            self.good,
            self.bad,
            self.ignore,
            self.proximity_angle,
            self.hair_scale,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || self.good < 0.
            || self.bad < self.good
            || self.ignore < self.bad
            || !(0. ..=180.).contains(&self.proximity_angle)
            || self.hair_scale < 0.
            || !(1..=10000).contains(&self.curve_divisions)
            || !(1..=128).contains(&self.surface_samples)
        {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(())
    }
    pub fn color(&self, distance: Real) -> Option<[u8; 3]> {
        if distance > self.ignore || self.ignore_bad && distance > self.bad {
            return None;
        }
        let t = if distance <= self.good {
            0.
        } else if distance >= self.bad {
            1.
        } else {
            (distance - self.good) / (self.bad - self.good)
        };
        Some(std::array::from_fn(|i| {
            ((1. - t) * f64::from(self.good_color[i]) + t * f64::from(self.bad_color[i])).round()
                as u8
        }))
    }
}
#[derive(Clone, Debug)]
pub struct Sample {
    pub source: ObjectId,
    pub target: ObjectId,
    pub point: Point3,
    pub closest: Point3,
    pub distance: Real,
    pub normal_angle: Option<Real>,
}
/// Exaggerated display segment and its RGB color.
pub type Hair = ([Point3; 2], [u8; 3]);
impl Sample {
    pub fn hair(&self, options: &Options) -> Result<Option<Hair>, GeometryError> {
        let Some(color) = options.color(self.distance) else {
            return Ok(None);
        };
        if !options.display_hair
            || self.distance > 0.
                && options.proximity_angle < 180.
                && self
                    .normal_angle
                    .is_none_or(|a| a > options.proximity_angle)
        {
            return Ok(None);
        };
        Ok(Some((
            [
                self.closest,
                self.closest.translated(
                    self.closest
                        .vector_to(self.point)?
                        .scaled(options.hair_scale)?,
                )?,
            ],
            color,
        )))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Statistics {
    pub count: usize,
    pub mean: Real,
    pub median: Real,
    pub standard_deviation: Real,
    pub minimum: Real,
    pub maximum: Real,
}
impl Statistics {
    pub fn from_distances(
        values: impl IntoIterator<Item = Real>,
    ) -> Result<Option<Self>, GeometryError> {
        let mut values = values.into_iter().collect::<Vec<_>>();
        if values.is_empty() {
            return Ok(None);
        }
        let mut sum = FiniteSum::default();
        for &v in &values {
            if v < 0. {
                return Err(GeometryError::Degenerate {
                    context: "negative deviation distance",
                });
            }
            sum.add(v)?;
        }
        let mean = sum.mean()?;
        values.sort_by(Real::total_cmp);
        let n = values.len();
        let mut middle = FiniteSum::default();
        middle.add(values[(n - 1) / 2])?;
        middle.add(values[n / 2])?;
        // Center and scale before rounding a mean. Adjacent large distances
        // can share a rounded mean, and subnormal midpoints may round to zero.
        let origin = values[0];
        let scale = values[n - 1] - origin;
        let standard_deviation = if scale == 0. {
            0.
        } else {
            let mut normalized = FiniteSum::default();
            for &v in &values {
                normalized.add((v - origin) / scale)?;
            }
            let center = normalized.mean()?;
            let mut squares = FiniteSum::default();
            for &v in &values {
                squares.add(((v - origin) / scale - center).powi(2))?;
            }
            scale * squares.mean()?.sqrt()
        };
        Ok(Some(Self {
            count: n,
            mean,
            median: middle.mean()?,
            standard_deviation,
            minimum: values[0],
            maximum: values[n - 1],
        }))
    }
}
#[derive(Clone, Debug)]
pub struct View {
    pub samples: Arc<[Sample]>,
    pub options: Options,
    pub visible: bool,
    pub statistics: Option<Statistics>,
}
impl View {
    pub fn report(&self) -> String {
        match &self.statistics {
            Some(s) => format!(
                "PointDeviation: {} points; mean {}; median {}; standard deviation {}; minimum {}; maximum {}",
                s.count, s.mean, s.median, s.standard_deviation, s.minimum, s.maximum
            ),
            None => "PointDeviation: no points within the ignore distance".into(),
        }
    }
}
/// Analyze supplied Euclidean locations against all targets, retaining ties in
/// target order. Input occurrences are preserved; proximity filters affect
/// display only. Ignore distance alone controls the statistics population.
pub fn analyze(
    points: &[(ObjectId, Point3)],
    targets: &[(ObjectId, &Geometry)],
    tolerance: Tolerance,
) -> Result<Arc<[Sample]>, CommandError> {
    if points.len() > MAX_SAMPLES || points.len().saturating_mul(targets.len()) > 4_000_000 {
        return Err(CommandError::Usage(
            "PointDeviation exceeds the sample/query limit",
        ));
    }
    if targets.is_empty() {
        return Err(CommandError::Usage(
            "PointDeviation requires target geometry",
        ));
    }
    let targets = targets
        .iter()
        .map(|&(id, g)| query::Target::new(id, g))
        .collect::<Result<Vec<_>, _>>()?;
    points
        .iter()
        .map(|&(source, point)| {
            let mut best: Option<Sample> = None;
            for target in &targets {
                let candidate = target.closest(source, point, tolerance)?;
                if let Some(c) = candidate
                    && best
                        .as_ref()
                        .is_none_or(|b| point.compare_distances(c.closest, b.closest).is_lt())
                {
                    best = Some(c)
                }
            }
            best.ok_or(CommandError::Usage(
                "PointDeviation target has no closest location",
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Into::into)
}

#[derive(Clone, Debug, Default)]
struct State {
    sources: Vec<ObjectId>,
    targets: Vec<ObjectId>,
    snapshots: Vec<GeometrySnapshot>,
    tolerance: Option<Tolerance>,
    sampling: Option<(usize, usize)>,
    samples: Arc<[Sample]>,
    options: Options,
    statistics: Option<Statistics>,
    statistics_ignore: Option<Real>,
    statistics_samples: Arc<[Sample]>,
    enabled: bool,
    visible: bool,
}
#[derive(Default)]
pub(super) struct Session(Mutex<State>);
impl Session {
    pub fn options(&self) -> Options {
        self.0
            .lock()
            .expect("point deviation session")
            .options
            .clone()
    }
    fn resolve(state: &mut State, doc: &Document, force: bool) -> Result<(), CommandError> {
        let objects = state
            .sources
            .iter()
            .chain(&state.targets)
            .map(|id| {
                doc.object(*id).ok_or(CommandError::Usage(
                    "PointDeviation source or target was deleted",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let dirty = state.tolerance != Some(doc.tolerance())
            || state.sampling
                != Some((state.options.curve_divisions, state.options.surface_samples))
            || state.snapshots.len() != objects.len()
            || state
                .snapshots
                .iter()
                .zip(&objects)
                .any(|(a, b)| !a.shares_storage_with(b.geometry_snapshot()));
        if force || dirty && state.options.automatic {
            let mut points = Vec::new();
            for object in &objects[..state.sources.len()] {
                let values =
                    query::source_points(object.geometry(), &state.options, doc.tolerance())?;
                if points.len().saturating_add(values.len()) > MAX_SAMPLES {
                    return Err(CommandError::Usage(
                        "PointDeviation exceeds 1000000 samples",
                    ));
                }
                points.extend(values.into_iter().map(|p| (object.id(), p)));
            }
            if points.is_empty() {
                return Err(CommandError::Usage("PointDeviation needs measured points"));
            }
            let targets = objects[state.sources.len()..]
                .iter()
                .map(|o| (o.id(), o.geometry()))
                .collect::<Vec<_>>();
            let samples = analyze(&points, &targets, doc.tolerance())?;
            state.samples = samples;
            state.snapshots = objects
                .iter()
                .map(|o| o.geometry_snapshot().clone())
                .collect();
            state.tolerance = Some(doc.tolerance());
            state.sampling = Some((state.options.curve_divisions, state.options.surface_samples));
        }
        Ok(())
    }
    fn make_view(state: &mut State) -> Result<View, CommandError> {
        if state.statistics_ignore != Some(state.options.ignore)
            || !Arc::ptr_eq(&state.statistics_samples, &state.samples)
        {
            state.statistics = Statistics::from_distances(
                state
                    .samples
                    .iter()
                    .filter(|s| s.distance <= state.options.ignore)
                    .map(|s| s.distance),
            )?;
            state.statistics_ignore = Some(state.options.ignore);
            state.statistics_samples = state.samples.clone();
        }
        Ok(View {
            samples: state.samples.clone(),
            options: state.options.clone(),
            visible: state.visible,
            statistics: state.statistics.clone(),
        })
    }
    pub(super) fn view(&self, doc: &Document) -> Result<Option<View>, CommandError> {
        let mut state = self.0.lock().expect("point deviation session");
        if !state.enabled {
            return Ok(None);
        }
        Self::resolve(&mut state, doc, false)?;
        Self::make_view(&mut state).map(Some)
    }
}
pub(super) struct PointDeviationCommand(pub Arc<Session>);
fn ids(value: &str) -> Result<Vec<ObjectId>, CommandError> {
    let mut seen = BTreeSet::new();
    value
        .split(',')
        .map(|s| s.parse().map_err(|_| CommandError::Usage(USAGE)))
        .collect::<Result<Vec<_>, _>>()
        .map(|ids| ids.into_iter().filter(|id| seen.insert(*id)).collect())
}
pub fn parse_options(mut options: Options, args: &[&str]) -> Result<Options, CommandError> {
    let mut seen = BTreeSet::new();
    for arg in args {
        let (key, v) = arg
            .trim_start_matches('_')
            .split_once('=')
            .ok_or(CommandError::Usage(USAGE))?;
        let key = key.to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        let v = v.trim_start_matches('_');
        match key.as_str() {
            "good" => options.good = parse_finite_real(v)?,
            "bad" => options.bad = parse_finite_real(v)?,
            "ignore" => options.ignore = parse_finite_real(v)?,
            "proximityangle" => options.proximity_angle = parse_finite_real(v)?,
            "hairscale" => options.hair_scale = parse_finite_real(v)?,
            "displayhair" => {
                options.display_hair = parse_yes_no(v).ok_or(CommandError::Usage(USAGE))?
            }
            "ignorebad" => {
                options.ignore_bad = parse_yes_no(v).ok_or(CommandError::Usage(USAGE))?
            }
            "automatic" => options.automatic = parse_yes_no(v).ok_or(CommandError::Usage(USAGE))?,
            "curvedivisions" => {
                options.curve_divisions = v.parse().map_err(|_| CommandError::Usage(USAGE))?
            }
            "surfacesamples" => {
                options.surface_samples = v.parse().map_err(|_| CommandError::Usage(USAGE))?
            }
            "goodcolor" | "badcolor" => {
                let rgb = v
                    .split(',')
                    .map(str::parse::<u8>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| CommandError::Usage(USAGE))?;
                let rgb = rgb.try_into().map_err(|_| CommandError::Usage(USAGE))?;
                if key == "goodcolor" {
                    options.good_color = rgb
                } else {
                    options.bad_color = rgb
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    options.validate()?;
    Ok(options)
}
impl Command for PointDeviationCommand {
    fn name(&self) -> &'static str {
        "PointDeviation"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut proposed = self.0.0.lock().expect("point deviation session").clone();
        let mut sources = None;
        let mut targets = None;
        let mut action = None;
        let mut option_args = Vec::new();
        let mut visible = None;
        for &a in args {
            if let Some((k, v)) = a.trim_start_matches('_').split_once('=') {
                if k.eq_ignore_ascii_case("Sources") && sources.is_none() {
                    sources = Some(ids(v)?)
                } else if k.eq_ignore_ascii_case("Targets") && targets.is_none() {
                    targets = Some(ids(v)?)
                } else if k.eq_ignore_ascii_case("Visible") && visible.is_none() {
                    visible = Some(
                        if v.trim_start_matches('_').eq_ignore_ascii_case("Toggle") {
                            !proposed.visible
                        } else {
                            parse_yes_no(v).ok_or(CommandError::Usage(USAGE))?
                        },
                    )
                } else {
                    option_args.push(a)
                }
            } else if action.is_none() {
                action = Some(a.trim_start_matches('_').to_ascii_lowercase())
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        proposed.options = parse_options(proposed.options, &option_args)?;
        if action.as_deref() == Some("end") {
            if sources.is_some()
                || targets.is_some()
                || !option_args.is_empty()
                || visible.is_some()
            {
                return Err(CommandError::Usage(USAGE));
            }
            *self.0.0.lock().unwrap() = State {
                options: proposed.options,
                ..State::default()
            };
            return Ok("PointDeviation ended".into());
        }
        let start = sources.is_some() || targets.is_some();
        if start {
            let target_ids =
                targets.ok_or(CommandError::Usage("PointDeviation needs Targets=id,..."))?;
            proposed.sources = sources.unwrap_or_else(|| {
                doc.selected_object_ids()
                    .filter(|id| !target_ids.contains(id))
                    .collect()
            });
            if proposed.sources.is_empty() {
                return Err(CommandError::Usage("PointDeviation needs source objects"));
            }
            proposed.targets = target_ids;
            proposed.enabled = true;
            proposed.visible = true;
        }
        if !proposed.enabled {
            return Err(CommandError::Usage(
                "Start PointDeviation with sources and targets",
            ));
        }
        if let Some(visible) = visible {
            proposed.visible = visible
        }
        if !matches!(action.as_deref(), None | Some("apply" | "makepermanent")) {
            return Err(CommandError::Usage(USAGE));
        }
        Session::resolve(
            &mut proposed,
            doc,
            start || action.as_deref() == Some("apply"),
        )?;
        let view = Session::make_view(&mut proposed)?;
        if action.as_deref() == Some("makepermanent") {
            let mut lines = Vec::new();
            for sample in view.samples.iter() {
                if let Some((ends, color)) = sample.hair(&view.options)?
                    && ends[0] != ends[1]
                {
                    lines.push((
                        LineSegment::try_new(ends[0], ends[1], Tolerance::NUMERICAL_VALIDATION)?,
                        color,
                    ))
                }
            }
            for (line, color) in lines {
                let name = format!("Point Test {},{},{}", color[0], color[1], color[2]);
                let layer = match doc.layer_by_name(&name) {
                    Some(layer) => layer.id(),
                    None => doc.add_layer(name, ColorRgb::new(color[0], color[1], color[2]))?,
                };
                doc.add_geometry_with_attributes(
                    Geometry::Line(line),
                    ObjectAttributes::on_layer(layer),
                )?;
            }
        }
        let report = view.report();
        *self.0.0.lock().unwrap() = proposed;
        Ok(report)
    }
}
