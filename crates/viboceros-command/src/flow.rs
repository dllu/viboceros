//! Staged curve-to-curve Flow edits using the shared length and morph kernels.
use super::*;
use std::collections::BTreeMap;
use viboceros_document::{CopyGroupPolicy, ReplacementHistory};
use viboceros_geometry::{BoundingBox3, CurveFlowOptions, CurveFlowPointMorph};

pub const USAGE: &str = "Flow Base=id Target=id [BaseEdge=parent,index] [TargetEdge=parent,index] [Copy=Yes|No] [Rigid=Yes|No] [RigidGroups=Yes|No] [Stretch=Yes|No] [PreserveStructure=Yes|No] [ReverseBase=Yes|No] [ReverseTarget=Yes|No]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub rigid: bool,
    pub rigid_groups: bool,
    pub stretch: bool,
    pub preserve_structure: bool,
    pub reverse_base: bool,
    pub reverse_target: bool,
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
            ..Default::default()
        }
    }
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        format!(
            "Copy={} Rigid={} RigidGroups={} Stretch={} PreserveStructure={} ReverseBase={} ReverseTarget={}",
            yes(self.copy),
            yes(self.rigid),
            yes(self.rigid_groups),
            yes(self.stretch),
            yes(self.preserve_structure),
            yes(self.reverse_base),
            yes(self.reverse_target)
        )
    }
}
#[derive(Clone, Copy, Debug)]
pub struct GuideReference {
    pub id: ObjectId,
    pub edge: Option<usize>,
}
#[derive(Default)]
pub struct References {
    pub base: Option<GuideReference>,
    pub target: Option<GuideReference>,
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
        let canonical = match key.as_str() {
            "baseedge" => "base",
            "targetedge" => "target",
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
                    refs.base = Some(reference);
                } else {
                    refs.target = Some(reference);
                }
            }
            "copy" | "rigid" | "rigidgroups" | "stretch" | "preservestructure" | "reversebase"
            | "reversetarget" => {
                let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                match key.as_str() {
                    "copy" => options.copy = value,
                    "rigid" => options.rigid = value,
                    "rigidgroups" => options.rigid_groups = value,
                    "stretch" => options.stretch = value,
                    "preservestructure" => options.preserve_structure = value,
                    "reversebase" => options.reverse_base = value,
                    "reversetarget" => options.reverse_target = value,
                    _ => unreachable!(),
                }
            }
            "roadlike" | "local" | "stairlike" => {
                let enabled = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                if enabled {
                    return Err(CommandError::FlowOptionNotSupported(match key.as_str() {
                        "roadlike" => "Roadlike",
                        "local" => "Local",
                        _ => "Stairlike",
                    }));
                }
            }
            "targetsurface" => return Err(CommandError::FlowOptionNotSupported("TargetSurface")),
            _ => return Err(CommandError::Usage(USAGE)),
        }
        i += 1;
    }
    Ok((options, refs))
}
fn guide(doc: &Document, r: GuideReference) -> Result<Curve3, CommandError> {
    let object = doc
        .object(r.id)
        .filter(|_| doc.is_object_selectable(r.id))
        .ok_or(DocumentError::ObjectNotFound(r.id))?;
    let curve = curve_reference::resolve(object.geometry(), r.edge, doc.tolerance())
        .ok_or(CommandError::FlowGuideNotCurve)?;
    Ok(curve.curve().to_owned())
}
pub fn prepare(
    doc: &Document,
    base: GuideReference,
    target: GuideReference,
    options: Options,
) -> Result<CurveFlowPointMorph, CommandError> {
    let mut morph = CurveFlowPointMorph::try_new(
        guide(doc, base)?,
        guide(doc, target)?,
        options.geometry(),
        doc.tolerance(),
    )?;
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
    let mut bounds = BTreeMap::new();
    if options.rigid && options.rigid_groups {
        for id in ids {
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
    ids.iter()
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
            ..value
        });
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        // Grips need separate native copy/rigid policies; never deform a
        // selected owner as a substitute for its selected grips.
        if doc.selected_control_points().next().is_some() {
            return Err(CommandError::FlowGripsNotSupported);
        }
        let (args, mut sources) = transform_arguments(doc, args, USAGE)?;
        let (options, refs) = options(&args, self.0.get())?;
        let base = refs.base.ok_or(CommandError::Usage(USAGE))?;
        let target = refs.target.ok_or(CommandError::Usage(USAGE))?;
        sources.ids.retain(|id| *id != base.id && *id != target.id);
        if sources.ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let morph = prepare(doc, base, target, options)?;
        let staged = deformed_geometries(doc, &sources.ids, &morph, options)?;
        let count = staged.len();
        if sources.release_on_replay {
            doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            doc.clear_selection();
        }
        if options.copy {
            doc.copy_object_geometries_with_groups(staged, CopyGroupPolicy::Preserve)?;
        } else {
            doc.replace_object_geometries_with_history(
                staged,
                ReplacementHistory::EveryReplacement,
            )?;
            doc.move_objects_to_end_in_order(sources.ids.iter().copied())?;
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
