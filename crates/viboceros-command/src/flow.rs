//! Staged curve-to-curve Flow edits using the shared length and morph kernels.
use super::*;
use std::collections::BTreeMap;
use viboceros_document::{ControlPointId, CopyGroupPolicy, ReplacementHistory};
use viboceros_geometry::{
    BoundingBox3, CurveFlowLocalRadii, CurveFlowOptions, CurveFlowPointMorph,
};

pub const USAGE: &str = "Flow (Base=id|BaseLine=start;end) (Target=id|TargetLine=start;end) [BaseEdge=parent,index] [TargetEdge=parent,index] [Copy=Yes|No] [Rigid=Yes|No] [RigidGroups=Yes|No] [Stretch=Yes|No] [PreserveStructure=Yes|No] [ReverseBase=Yes|No] [ReverseTarget=Yes|No] [Roadlike=Yes|No] [BaseUp=x,y,z] [TargetUp=x,y,z] [Local=Yes|No] [LocalRadii=inner,outer]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub rigid: bool,
    pub rigid_groups: bool,
    pub stretch: bool,
    pub preserve_structure: bool,
    pub reverse_base: bool,
    pub reverse_target: bool,
    pub roadlike: bool,
    pub base_up: Option<Vector3>,
    pub target_up: Option<Vector3>,
    pub local: bool,
    pub local_radii: Option<CurveFlowLocalRadii>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            copy: true,
            rigid: false,
            rigid_groups: true,
            stretch: false,
            preserve_structure: false,
            reverse_base: false,
            reverse_target: false,
            roadlike: false,
            base_up: None,
            target_up: None,
            local: false,
            local_radii: None,
        }
    }
}
impl Options {
    pub fn geometry(self) -> CurveFlowOptions {
        CurveFlowOptions {
            stretch: self.stretch,
            preserve_structure: self.preserve_structure,
            reverse_base: self.reverse_base,
            reverse_target: self.reverse_target,
            base_up: self.roadlike.then(|| {
                self.base_up
                    .unwrap_or(Vector3::try_new(0., 0., 1.).unwrap())
            }),
            target_up: self.roadlike.then(|| {
                self.target_up
                    .unwrap_or(Vector3::try_new(0., 0., 1.).unwrap())
            }),
            local_radii: self.local.then_some(self.local_radii).flatten(),
        }
    }
    /// Resolve the CPlane Z picker preset into explicit directions.
    pub fn with_cplane_up(mut self, up: Vector3) -> Self {
        if self.roadlike {
            self.base_up.get_or_insert(up);
            self.target_up.get_or_insert(up);
        }
        self
    }
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        let mut arguments = format!(
            "Copy={} Rigid={} RigidGroups={} Stretch={} PreserveStructure={} ReverseBase={} ReverseTarget={} Roadlike={}",
            yes(self.copy),
            yes(self.rigid),
            yes(self.rigid_groups),
            yes(self.stretch),
            yes(self.preserve_structure),
            yes(self.reverse_base),
            yes(self.reverse_target),
            yes(self.roadlike)
        );
        arguments.push_str(&format!(" Local={}", yes(self.local)));
        if self.local
            && let Some(radii) = self.local_radii
        {
            let [inner, outer] = radii.radii();
            arguments.push_str(&format!(" LocalRadii={inner},{outer}"));
        }
        if self.roadlike {
            for (name, up) in [("BaseUp", self.base_up), ("TargetUp", self.target_up)] {
                if let Some(up) = up {
                    let value = up.to_array().map(|v| v.to_string()).join(",");
                    arguments.push_str(&format!(" {name}={value}"));
                }
            }
        }
        arguments
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideReference {
    pub id: ObjectId,
    pub edge: Option<usize>,
}
/// A selected document curve/edge or an immutable temporary line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuideInput {
    Object(GuideReference),
    Line([Point3; 2]),
}
impl From<GuideReference> for GuideInput {
    fn from(value: GuideReference) -> Self {
        Self::Object(value)
    }
}
impl GuideInput {
    pub fn object_id(self) -> Option<ObjectId> {
        match self {
            Self::Object(r) => Some(r.id),
            Self::Line(_) => None,
        }
    }
    pub fn arguments(self, name: &str) -> String {
        match self {
            Self::Object(r) => match r.edge {
                Some(edge) => format!("{name}Edge={},{}", r.id, edge),
                None => format!("{name}={}", r.id),
            },
            Self::Line([a, b]) => {
                let point = |p: Point3| p.to_array().map(|x| x.to_string()).join(",");
                format!("{name}Line={};{}", point(a), point(b))
            }
        }
    }
}
#[derive(Default)]
pub struct References {
    pub base: Option<GuideInput>,
    pub target: Option<GuideInput>,
}
fn line_pair(value: &str) -> Result<[Point3; 2], CommandError> {
    let (a, b) = value.split_once(';').ok_or(CommandError::Usage(USAGE))?;
    let (a, n) = parse_point(&[a])?;
    let (b, m) = parse_point(&[b])?;
    if n != 1 || m != 1 {
        return Err(CommandError::Usage(USAGE));
    }
    Ok([a, b])
}
pub fn options(args: &[&str], mut options: Options) -> Result<(Options, References), CommandError> {
    let mut refs = References::default();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let (key, value) = args[i]
            .split_once('=')
            .map_or((args[i], None), |(k, v)| (k, Some(v)));
        let key = key.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if key == "line" {
            let base = refs.base.is_none();
            let role = if base { "base" } else { "target" };
            if !seen.insert(role.to_owned()) {
                return Err(CommandError::Usage(USAGE));
            }
            let (points, consumed) = if let Some(value) = value {
                (line_pair(value)?, 0)
            } else {
                let (a, n) = parse_point(&args[i + 1..])?;
                let (b, m) = parse_point(&args[i + 1 + n..])?;
                ([a, b], n + m)
            };
            if base {
                refs.base = Some(GuideInput::Line(points));
            } else {
                refs.target = Some(GuideInput::Line(points));
            }
            i += consumed + 1;
            continue;
        }
        let canonical = match key.as_str() {
            "baseedge" | "baseline" => "base",
            "targetedge" | "targetline" => "target",
            _ => &key,
        };
        if !seen.insert(canonical.to_owned()) {
            return Err(CommandError::Usage(USAGE));
        }
        let value = if let Some(v) = value {
            v
        } else {
            i += 1;
            *args.get(i).ok_or(CommandError::Usage(USAGE))?
        };
        match key.as_str() {
            "localradii" => {
                let (a, b) = value.split_once(',').ok_or(CommandError::Usage(USAGE))?;
                let a = a.parse().map_err(|_| CommandError::Usage(USAGE))?;
                let b = b.parse().map_err(|_| CommandError::Usage(USAGE))?;
                options.local_radii = Some(CurveFlowLocalRadii::try_from_pair([a, b])?);
            }
            "baseline" | "targetline" => {
                let input = GuideInput::Line(line_pair(value)?);
                if key.starts_with("base") {
                    refs.base = Some(input);
                } else {
                    refs.target = Some(input);
                }
            }
            "base" | "target" | "baseedge" | "targetedge" => {
                let reference = if key.ends_with("edge") {
                    let (id, edge) =
                        curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                    GuideReference {
                        id,
                        edge: Some(edge),
                    }
                } else {
                    GuideReference {
                        id: value.parse().map_err(|_| CommandError::Usage(USAGE))?,
                        edge: None,
                    }
                };
                if key.starts_with("base") {
                    refs.base = Some(reference.into());
                } else {
                    refs.target = Some(reference.into());
                }
            }
            "baseup" | "targetup" => {
                let up = if value
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("Automatic")
                {
                    return Err(CommandError::FlowOptionNotSupported("Roadlike Automatic"));
                } else {
                    let (point, count) = parse_point(&[value])?;
                    if count != 1 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    Some(
                        Vector3::try_from(point.to_array())?
                            .normalized_nonzero()?
                            .as_vector(),
                    )
                };
                if key == "baseup" {
                    options.base_up = up;
                } else {
                    options.target_up = up;
                }
            }
            "copy" | "rigid" | "rigidgroups" | "stretch" | "preservestructure" | "reversebase"
            | "reversetarget" | "roadlike" | "local" => {
                let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                match key.as_str() {
                    "copy" => options.copy = value,
                    "rigid" => options.rigid = value,
                    "rigidgroups" => options.rigid_groups = value,
                    "stretch" => options.stretch = value,
                    "preservestructure" => options.preserve_structure = value,
                    "reversebase" => options.reverse_base = value,
                    "reversetarget" => options.reverse_target = value,
                    "roadlike" => options.roadlike = value,
                    "local" => options.local = value,
                    _ => unreachable!(),
                }
            }
            "stairlike" => {
                let enabled = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                if enabled {
                    return Err(CommandError::FlowOptionNotSupported("Stairlike"));
                }
            }
            "targetsurface" => return Err(CommandError::FlowOptionNotSupported("TargetSurface")),
            _ => return Err(CommandError::Usage(USAGE)),
        }
        i += 1;
    }
    Ok((options, refs))
}
fn guide(doc: &Document, input: GuideInput) -> Result<Curve3, CommandError> {
    let r = match input {
        GuideInput::Line([a, b]) => {
            return Ok(Curve3::Line(LineSegment::try_new(a, b, doc.tolerance())?));
        }
        GuideInput::Object(r) => r,
    };
    let object = doc
        .object(r.id)
        .filter(|_| doc.is_object_selectable(r.id))
        .ok_or(DocumentError::ObjectNotFound(r.id))?;
    let curve = curve_reference::resolve(object.geometry(), r.edge, doc.tolerance())
        .ok_or(CommandError::FlowGuideNotCurve)?;
    Ok(curve.curve().to_owned())
}
/// Plane used by the interactive local-influence Circle getters.
pub fn local_radius_frame(
    doc: &Document,
    base: GuideInput,
    reverse: bool,
) -> Result<viboceros_geometry::Frame3, CommandError> {
    let curve = guide(doc, base)?;
    let curve = curve.as_ref();
    let t = if reverse {
        *curve.domain().end()
    } else {
        *curve.domain().start()
    };
    let sample = curve.evaluate_with_tangent(t)?;
    Ok(viboceros_geometry::Frame3::try_from_normal(
        sample.point(),
        sample.tangent().as_vector(),
        Tolerance::NUMERICAL_VALIDATION,
    )?)
}
pub fn prepare(
    doc: &Document,
    base: impl Into<GuideInput>,
    target: impl Into<GuideInput>,
    options: Options,
) -> Result<CurveFlowPointMorph, CommandError> {
    let base = guide(doc, base.into())?;
    let target = guide(doc, target.into())?;
    if options.local && options.local_radii.is_none() {
        return Err(CommandError::FlowLocalRadiiRequired);
    }
    if options.roadlike && (options.base_up.is_none() || options.target_up.is_none()) {
        return Err(CommandError::FlowRoadlikeAxesRequired);
    }
    let mut morph =
        CurveFlowPointMorph::try_new(base, target, options.geometry(), doc.tolerance())?;
    if morph.uses_spatial_transport() {
        return Err(CommandError::FlowSpatialFramesNotSupported);
    }
    if let Some(solver) = morph_interpolation_solver() {
        morph = morph.with_interpolation_solver(solver);
    }
    Ok(morph)
}
pub fn deformed_geometries(
    doc: &Document,
    ids: &[ObjectId],
    morph: &CurveFlowPointMorph,
    options: Options,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    deformed_with_grips(doc, ids, &[], morph, options)
}
/// Rigid ordinary objects include selected grip owners in their group bounds.
/// Each selected grip itself always follows the point map.
pub fn deformed_with_grips(
    doc: &Document,
    ids: &[ObjectId],
    grips: &[ControlPointId],
    morph: &CurveFlowPointMorph,
    options: Options,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let grip_owners = grips.iter().map(|g| g.object).collect::<BTreeSet<_>>();
    let context = ids
        .iter()
        .copied()
        .chain(grip_owners.iter().copied())
        .collect::<BTreeSet<_>>();
    let ordinary = ids
        .iter()
        .copied()
        .filter(|id| !grip_owners.contains(id))
        .collect::<Vec<_>>();
    let mut bounds = BTreeMap::new();
    if options.rigid && options.rigid_groups && !ordinary.is_empty() {
        for id in &context {
            let o = doc.object(*id).ok_or(DocumentError::ObjectNotFound(*id))?;
            if let Some(group) = o.top_group() {
                let b = o.geometry().tight_bounds(doc.tolerance())?;
                let next = if let Some(prior) = bounds.get(&group) {
                    BoundingBox3::union(*prior, b)?
                } else {
                    b
                };
                bounds.insert(group, next);
            }
        }
    }
    let transforms = bounds
        .into_iter()
        .map(|(id, b)| Ok((id, morph.rigid_transform(b.center()?)?)))
        .collect::<Result<BTreeMap<_, _>, GeometryError>>()?;
    ordinary
        .iter()
        .map(|id| {
            let o = doc.object(*id).ok_or(DocumentError::ObjectNotFound(*id))?;
            let geometry = if options.rigid {
                let transform = if options.rigid_groups
                    && let Some(group) = o.top_group()
                {
                    transforms[&group]
                } else {
                    morph.rigid_transform(o.geometry().tight_bounds(doc.tolerance())?.center()?)?
                };
                o.geometry().transformed(transform, doc.tolerance())?
            } else {
                // Polysurfaces use the existing shared-edge fitting path.
                let morph = if matches!(o.geometry(),Geometry::Brep(b)if b.faces().len()>1) {
                    morph.clone().with_preserve_structure(false)
                } else {
                    morph.clone()
                };
                o.geometry().morphed(&morph, doc.tolerance())?
            };
            Ok((*id, geometry))
        })
        .collect()
}
#[derive(Default)]
pub(super) struct FlowCommand(remembered::Remembered<Options>);
impl Command for FlowCommand {
    fn name(&self) -> &'static str {
        "Flow"
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(true)
    }
    fn flow_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_flow_options(&self, value: Options) -> bool {
        self.0.set(Options {
            copy: true,
            base_up: None,
            target_up: None,
            local_radii: None,
            ..value
        });
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (args, mut sources) = affine_transform_arguments(doc, args, USAGE)?;
        let (options, refs) = options(&args, self.0.get())?;
        let base = refs.base.ok_or(CommandError::Usage(USAGE))?;
        let target = refs.target.ok_or(CommandError::Usage(USAGE))?;
        sources
            .ids
            .retain(|id| Some(*id) != base.object_id() && Some(*id) != target.object_id());
        if sources
            .grips
            .iter()
            .any(|g| Some(g.object) == base.object_id() || Some(g.object) == target.object_id())
        {
            return Err(CommandError::Usage(
                "Flow guides cannot supply source grips",
            ));
        }
        if sources.ids.is_empty() && sources.grips.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let morph = prepare(doc, base, target, options)?;
        let staged = deformed_with_grips(doc, &sources.ids, &sources.grips, &morph, options)?;
        let mut active_grips = sources.grips.clone();
        if options.local {
            // Preserve the original grip-owner context while staging whole
            // objects. A simultaneously selected parent must not become an
            // ordinary source when its selected grips are outside the tube.
            let mut filtered = Vec::new();
            for grip in &sources.grips {
                let point = doc
                    .control_point_locations(grip.object)
                    .and_then(|p| p.get(grip.index).copied())
                    .ok_or(DocumentError::InvalidControlPointSelection {
                        object: grip.object,
                        index: grip.index,
                    })?;
                if morph.local_influence_at(point)? != 0. {
                    filtered.push(*grip);
                }
            }
            active_grips = filtered;
        }
        let staged_ids = staged.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let count = staged.len()
            + active_grips
                .iter()
                .map(|g| g.object)
                .collect::<BTreeSet<_>>()
                .len();
        if sources.release_on_replay {
            doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            // Postselected grip replacements restore the getter's selected
            // grips on Undo. Copy records their released source selection.
            if options.copy || sources.grips.is_empty() {
                doc.clear_selection();
            }
        }
        if !active_grips.is_empty() {
            doc.morph_grips_with_object_geometries(
                staged,
                active_grips.iter().copied(),
                &morph,
                options.copy,
                CopyGroupPolicy::Preserve,
            )?;
        } else if options.copy {
            doc.copy_object_geometries_with_groups(staged, CopyGroupPolicy::Preserve)?;
        } else {
            doc.replace_object_geometries_with_history(
                staged,
                ReplacementHistory::EveryReplacement,
            )?;
            doc.move_objects_to_end_in_order(staged_ids)?;
        }
        doc.select_objects_direct(sources.ids.iter().copied(), SelectionMode::Replace)?;
        if sources.postselected {
            doc.clear_selection();
        }
        self.remember_flow_options(options);
        Ok(format!(
            "Flowed {count} object(s){}",
            if options.copy {
                ", creating copies"
            } else {
                ""
            }
        ))
    }
}

#[cfg(test)]
mod tests;
