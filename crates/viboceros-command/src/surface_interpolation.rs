//! Curve-on-surface plans shared by scripts and point getters.
use super::*;
use crate::orient_surface::Target;
use viboceros_geometry::{MAX_CURVE_INTERPOLATION_POINTS, Point2, SurfaceInterpolatedCurve};
mod support;
pub use support::Support;
pub const USAGE: &str = "InterpCrvOnSrf points... [Surface=id Face=index|InfinitePlane=definition] [Close=Yes|No] [AutoClose=Yes|No] [PersistentClose=Yes|No]";
pub const MAX_POINTS: usize = MAX_CURVE_INTERPOLATION_POINTS;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub auto_close: bool,
    pub persistent_close: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            auto_close: true,
            persistent_close: false,
        }
    }
}
#[derive(Default)]
pub struct Reference {
    pub surface: Option<ObjectId>,
    pub face: Option<usize>,
    pub plane: Option<String>,
    pub close: bool,
}
pub fn input<'a>(
    args: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Reference, Options), CommandError> {
    let mut points = Vec::new();
    let mut refs = Reference::default();
    let mut seen = BTreeSet::new();
    for arg in args {
        let word = arg.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Close") {
            refs.close = true;
            continue;
        }
        if let Some((key, value)) = word.split_once('=') {
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "surface" => {
                    refs.surface = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "face" => refs.face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                "infiniteplane" => refs.plane = Some(value.into()),
                "close" => refs.close = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                "autoclose" => {
                    options.auto_close = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "persistentclose" => {
                    options.persistent_close =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            points.push(*arg);
        }
    }
    if refs.plane.is_some() && (refs.surface.is_some() || refs.face.is_some()) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((points, refs, options))
}
impl Reference {
    pub fn target(&self, doc: &Document, cplane: Frame3) -> Result<Option<Support>, CommandError> {
        if let Some(plane) = &self.plane {
            return Ok(Some(Support::capture_plane(doc, plane, cplane)?));
        }
        self.surface
            .or_else(|| crate::short_path::selected_target(doc))
            .map(|id| Target::capture(doc, id, self.face).map(Support::from))
            .transpose()
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub target: Support,
    pub interpolation: SurfaceInterpolatedCurve,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        target: impl Into<Support>,
        points: &[Point3],
        closed: bool,
    ) -> Result<Self, CommandError> {
        let target = target.into();
        if !target.current(doc) {
            return Err(CommandError::Usage(
                "Select the interpolation surface again",
            ));
        }
        if points.len() > MAX_POINTS {
            return Err(CommandError::Usage("Too many interpolation points"));
        }
        let interpolation = target.interpolate(doc, points, closed)?;
        Ok(Self {
            target,
            interpolation,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.target.current(doc)
    }
    pub fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Select the interpolation surface again",
            ));
        }
        doc.add_geometry(Geometry::NurbsCurve(self.interpolation.curve.clone()))?;
        Ok("Created interpolated curve on surface".into())
    }
    pub fn commit(
        &self,
        doc: &mut Document,
        group: &mut viboceros_document::HistoryGroup,
    ) -> Result<String, CommandError> {
        doc.begin_group_transaction(group)?;
        match self.apply(doc) {
            Ok(message) => {
                doc.commit_group_transaction(group)?;
                Ok(message)
            }
            Err(error) => {
                doc.rollback_transaction()?;
                Err(error)
            }
        }
    }
}
pub struct InterpolateCurveOnSurfaceCommand;
impl Command for InterpolateCurveOnSurfaceCommand {
    fn name(&self) -> &'static str {
        "InterpCrvOnSrf"
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
        let (args, reference, options) = input(args, Options::default())?;
        let mut points = Vec::new();
        let mut consumed = 0;
        while consumed < args.len() {
            let (p, n) = parse_point(&args[consumed..])?;
            points.push(p);
            consumed += n;
            if points.len() > MAX_POINTS {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let target = reference
            .target(doc, context.construction_plane)?
            .ok_or(CommandError::Usage(USAGE))?;
        let auto_closed = options.auto_close && points.len() >= 4 && {
            let first = target.point(points[0])?;
            let last = target.point(*points.last().unwrap())?;
            first.distance_to(last)? <= 1.490116119385e-8
        };
        Plan::prepare(
            doc,
            target,
            &points,
            reference.close || options.persistent_close || auto_closed,
        )?
        .apply(doc)
    }
}
