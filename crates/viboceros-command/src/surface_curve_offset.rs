//! Captured curve-on-face offsets, shared by scripted execution and previews.
use super::*;
use crate::{curve_reference::CapturedCurve, orient_surface::Target};
use std::sync::Arc;
use viboceros_geometry::{
    CurveExtensionSide, CurveExtensionStyle, SurfaceCurveOffsetGapStyle, SurfaceCurveOffsetOptions,
};
mod through_point;
pub use through_point::PointSetPlan;

pub const USAGE: &str = "OffsetCrvOnSrf distance|ThroughPoint=x,y,z|ThroughPoint Points=id,... Surface=id [Source=id|Edge=id,index] [Face=index] [SidePoint=x,y,z] [Flip=Yes|No] [Number=count] [SubCrv=start,end]";
pub const MAX_NUMBER: usize = 1024;
pub const MAX_POINTS: usize = 1024;
const MAX_OUTPUTS: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distance: Real,
    pub number: usize,
    pub flip: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distance: 1.,
            number: 1,
            flip: false,
        }
    }
}
impl Options {
    pub fn arguments(self) -> String {
        format!(
            "{} Number={} Flip={}",
            self.distance,
            self.number,
            if self.flip { "Yes" } else { "No" }
        )
    }
    pub fn validate(self) -> Result<(), CommandError> {
        if !self.distance.is_finite()
            || self.distance == 0.
            || !(1..=MAX_NUMBER).contains(&self.number)
        {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct References {
    pub source: Option<ObjectId>,
    pub edge: Option<usize>,
    pub surface: Option<ObjectId>,
    pub face: Option<usize>,
    pub side: Option<Point3>,
    pub range: Option<[Real; 2]>,
    pub through: Option<Point3>,
    pub through_mode: bool,
    pub points: Option<Vec<ObjectId>>,
}

pub fn options(
    args: &[&str],
    mut options: Options,
) -> Result<(Options, References, bool), CommandError> {
    let mut refs = References::default();
    let mut distance = false;
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let token = args[i].trim_start_matches(['_', '-']);
        // A leading minus on a bare distance is a numeric sign.
        if !args[i].contains('=')
            && !distance
            && let Ok(value) = args[i].parse::<Real>()
        {
            if !value.is_finite() {
                return Err(CommandError::Usage(USAGE));
            }
            options.distance = value;
            distance = true;
            seen.insert("distance".into());
            i += 1;
            continue;
        }
        let key = token.split('=').next().unwrap().to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        if key == "throughpoint"
            && !token.contains('=')
            && args
                .get(i + 1)
                .is_none_or(|v| parse_single_option_point(v, USAGE).is_err())
        {
            refs.through_mode = true;
            i += 1;
            continue;
        }
        if key == "flip"
            && !token.contains('=')
            && args.get(i + 1).is_none_or(|v| parse_yes_no(v).is_none())
        {
            options.flip = !options.flip;
            i += 1;
            continue;
        }
        let (_, value, n) = orient_option(args, i, USAGE)?;
        match key.as_str() {
            "distance" => {
                options.distance = parse_finite_real(value)?;
                distance = true;
            }
            "number" => options.number = value.parse().map_err(|_| CommandError::Usage(USAGE))?,
            "flip" => options.flip = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
            "source" => refs.source = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
            "edge" => {
                let (id, index) =
                    curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                if refs.source.is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
                refs.source = Some(id);
                refs.edge = Some(index);
            }
            "surface" => {
                refs.surface = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
            }
            "face" => refs.face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
            "sidepoint" => refs.side = Some(parse_single_option_point(value, USAGE)?),
            "throughpoint" => {
                refs.through = Some(parse_single_option_point(value, USAGE)?);
                refs.through_mode = true;
            }
            "points" => {
                let ids = value
                    .split(',')
                    .take(MAX_POINTS + 1)
                    .map(|v| v.parse().map_err(|_| CommandError::Usage(USAGE)))
                    .collect::<Result<Vec<ObjectId>, _>>()?;
                if ids.is_empty() || ids.len() > MAX_POINTS {
                    return Err(CommandError::Usage(USAGE));
                }
                refs.points = Some(ids);
            }
            "subcrv" => {
                let values = value
                    .split(',')
                    .map(parse_finite_real)
                    .collect::<Result<Vec<_>, _>>()?;
                refs.range = Some(values.try_into().map_err(|_| CommandError::Usage(USAGE))?);
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
        i += n;
    }
    if seen.contains("source") && seen.contains("edge") {
        return Err(CommandError::Usage(USAGE));
    }
    if distance && refs.through_mode
        || refs.points.is_some() && (!refs.through_mode || refs.through.is_some())
    {
        return Err(CommandError::Usage(USAGE));
    }
    options.validate()?;
    Ok((options, refs, distance))
}

#[derive(Clone, Debug)]
pub struct Source {
    pub reference: CapturedCurve,
    pub range: Option<[Real; 2]>,
    curve: Arc<NurbsCurve>,
}
impl Source {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        range: Option<[Real; 2]>,
    ) -> Result<Self, CommandError> {
        let reference = CapturedCurve::capture(doc, id, edge)?;
        let curve = orient_curve::prepare_guide(&reference.curve_ref().to_owned(), range)?
            .as_ref()
            .to_nurbs()?;
        Ok(Self {
            reference,
            range,
            curve: Arc::new(curve),
        })
    }
    pub fn curve(&self) -> &NurbsCurve {
        &self.curve
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.reference.current(doc)
    }
    pub fn arguments(&self) -> String {
        let (_, edge) = self.reference.source();
        let mut args = if let Some(edge) = edge {
            format!("Edge={},{}", self.reference.object_id(), edge)
        } else {
            format!("Source={}", self.reference.object_id())
        };
        if let Some([a, b]) = self.range {
            args += &format!(" SubCrv={a},{b}");
        }
        args
    }
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub source: Source,
    pub target: Target,
    pub options: Options,
    pub side: Option<Point3>,
    pub signed_distance: Real,
    pub curves: Vec<NurbsCurve>,
    pub through: Option<Point3>,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        source: Source,
        target: Target,
        side: Option<Point3>,
        options: Options,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        if !source.current(doc) || !target.current(doc) {
            return Err(CommandError::Usage(
                "Select the source curve and surface again",
            ));
        }
        let tolerance = doc.tolerance();
        let orientation = if target.reversed { -1. } else { 1. };
        let sign = if let Some(pick) = side {
            let pick = target.location(pick, false)?.point;
            let t = source.curve().closest_parameter(pick, tolerance)?;
            let sample = CurveRef::NurbsCurve(source.curve()).evaluate_with_tangent(t)?;
            let uv = target
                .surface
                .closest_parameters(sample.point(), tolerance)?;
            let left = target
                .surface
                .normal_at(uv.0, uv.1)?
                .as_vector()
                .cross(sample.tangent().as_vector())?
                .normalized_nonzero()?;
            let side = sample.point().vector_to(pick)?.dot(left.as_vector())?;
            if side.abs() <= tolerance.absolute() {
                return Err(GeometryError::AmbiguousCurveOffsetSide.into());
            }
            side.signum()
        } else {
            orientation
        };
        let distance = options.distance * sign * if options.flip { -1. } else { 1. };
        let object = doc
            .object(target.id)
            .ok_or(DocumentError::ObjectNotFound(target.id))?;
        let face = if let Geometry::Brep(b) = object.geometry() {
            Some(&b.faces()[target.face])
        } else {
            None
        };
        let trimmed = face
            .map(|f| f.is_untrimmed(tolerance))
            .transpose()?
            .is_some_and(|v| !v);
        let planar_boundaries = if target.surface.plane(tolerance)?.is_some() {
            Some(
                Brep::try_surface_face((*target.surface).clone(), tolerance)?
                    .edges()
                    .iter()
                    .map(|e| e.curve().clone())
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let mut current = vec![source.curve().clone()];
        let mut outputs = Vec::new();
        for _ in 0..options.number {
            let mut next = Vec::new();
            let mut visible = Vec::new();
            for c in &current {
                let result = viboceros_geometry::try_offset_curves_on_surface_with_options(
                    &target.surface,
                    c,
                    distance,
                    tolerance,
                    SurfaceCurveOffsetOptions {
                        closed_gaps: SurfaceCurveOffsetGapStyle::ConnectClosed,
                        interpolation_solver: morph_interpolation_solver(),
                    },
                )?;
                for c in result.curves {
                    // The public command extends planar open offsets along
                    // their endpoint tangents. The SDK producer keeps source
                    // endpoints; share its geometry and add command policy here.
                    let c = if let Some(boundaries) = &planar_boundaries {
                        let c = extend_planar_offset(c, boundaries, tolerance)?;
                        if !c.is_closed()? {
                            // A surface planar only within tolerance still has
                            // to admit the complete tangent extension.
                            target.surface.try_pullback_curve_certified(&c, tolerance)?;
                        }
                        c
                    } else {
                        c
                    };
                    if trimmed {
                        let uv = target.surface.try_pullback_curve_certified(&c, tolerance)?;
                        for event in viboceros_geometry::clip_surface_curve_to_face_with_operation(
                            &c,
                            &uv,
                            face.unwrap(),
                            tolerance,
                            &mut curve_intersections::events,
                        )? {
                            if let viboceros_geometry::SurfaceSurfaceIntersectionEvent::Curve(c) =
                                event
                            {
                                visible.push(c);
                            }
                        }
                    } else {
                        visible.push(c.clone());
                    }
                    // Number advances the complete offset before face trimming.
                    // A hole can split one copy and disappear from the next;
                    // advancing each clipped fragment would duplicate that copy.
                    next.push(c);
                }
            }
            if next.is_empty() {
                break;
            }
            if outputs.len() + visible.len() > MAX_OUTPUTS || next.len() > MAX_OUTPUTS {
                return Err(CommandError::Usage("Too many surface offset curves"));
            }
            outputs.extend(visible);
            current = next;
        }
        Ok(Self {
            source,
            target,
            options,
            side,
            signed_distance: distance,
            curves: outputs,
            through: None,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.source.current(doc) && self.target.current(doc)
    }
    pub fn arguments(&self) -> String {
        let side = self.side.map_or_else(String::new, |p| {
            let [x, y, z] = p.to_array();
            format!(" SidePoint={x},{y},{z}")
        });
        let mode = if let Some(point) = self.through {
            let [x, y, z] = point.to_array();
            format!("ThroughPoint={x},{y},{z} Number={}", self.options.number)
        } else {
            self.options.arguments()
        };
        format!(
            "OffsetCrvOnSrf {} Surface={} Face={} {}{side}",
            mode,
            self.target.id,
            self.target.face,
            self.source.arguments()
        )
    }
    /// Commit the cached preview without repeating geodesic fitting. Captures
    /// and the history token are checked before a single atomic document edit.
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
    fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Select the source curve and surface again",
            ));
        }
        if self.curves.is_empty() {
            return Err(CommandError::Usage(
                "No offset curves lie on the selected surface",
            ));
        }
        for c in &self.curves {
            doc.add_geometry(Geometry::NurbsCurve(c.clone()))?;
        }
        Ok(format!(
            "Created {} surface offset curve(s)",
            self.curves.len()
        ))
    }
}

fn extend_planar_offset(
    curve: NurbsCurve,
    boundaries: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<NurbsCurve, CommandError> {
    if curve.is_closed()? {
        return Ok(curve);
    }
    let on_boundary = |point: Point3| -> Result<bool, GeometryError> {
        for boundary in boundaries {
            let t = boundary.closest_parameter(point, tolerance)?;
            if boundary.evaluate(t)?.distance_to(point)? <= tolerance.absolute() {
                return Ok(true);
            }
        }
        Ok(false)
    };
    let start = on_boundary(curve.evaluate(*curve.domain().start())?)?;
    let end = on_boundary(curve.evaluate(*curve.domain().end())?)?;
    let side = match (start, end) {
        (true, true) => return Ok(curve),
        (false, true) => CurveExtensionSide::Start,
        (true, false) => CurveExtensionSide::End,
        (false, false) => CurveExtensionSide::Both,
    };
    Ok(curve.try_merged_to_curve_boundaries(
        side,
        CurveExtensionStyle::Line,
        boundaries,
        tolerance,
    )?)
}

pub(super) struct OffsetCurveOnSurfaceCommand;
impl Command for OffsetCurveOnSurfaceCommand {
    fn name(&self) -> &'static str {
        "OffsetCrvOnSrf"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (options, mut refs, has_distance) = options(args, Options::default())?;
        if !has_distance && refs.through.is_none() && refs.points.is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        if refs.source.is_none() {
            let ids = doc.selected_object_ids().collect::<Vec<_>>();
            if let [id] = ids.as_slice() {
                refs.source = Some(*id);
            }
        }
        let source = Source::capture(
            doc,
            refs.source.ok_or(CommandError::NoObjectsSelected)?,
            refs.edge,
            refs.range,
        )?;
        let target = Target::capture(
            doc,
            refs.surface.ok_or(CommandError::Usage(USAGE))?,
            refs.face,
        )?;
        if let Some(ids) = refs.points {
            return PointSetPlan::prepare(doc, source, target, &ids, options)?.apply(doc);
        }
        let plan = if let Some(point) = refs.through {
            Plan::prepare_through_point(doc, source, target, point, options)?
        } else {
            Plan::prepare(doc, source, target, refs.side, options)?
        };
        plan.apply(doc)
    }
}

#[cfg(test)]
mod tests;
