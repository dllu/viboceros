//! FlowAlongSrf reference preparation and one staged document edit.
use super::*;
use std::sync::Arc;
use viboceros_document::{ControlPointId, CopyGroupPolicy, ReplacementHistory};
use viboceros_geometry::{SurfaceFlowFrame, SurfaceFlowOptions, SurfaceFlowPointMorph};
pub const USAGE: &str = "FlowAlongSrf (Base=id|BasePlane=origin;xcorner;widthpoint) Target=id [BaseFace=parent,index] [TargetFace=parent,index] [BaseCorner=0..3] [TargetCorner=0..3] [BaseSwap=Yes|No] [TargetSwap=Yes|No] [Copy=Yes|No] [Rigid=Yes|No] [RigidGroups=Yes|No] [AutoAdjust=Yes|No] [ConstrainNormal=Yes|No] [Normal=x,y,z] [PreserveStructure=Yes|No]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub rigid: bool,
    pub rigid_groups: bool,
    pub auto_adjust: bool,
    pub constrain_normal: bool,
    pub normal: Option<Vector3>,
    pub preserve_structure: bool,
    pub base_frame: SurfaceFlowFrame,
    pub target_frame: SurfaceFlowFrame,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            copy: true,
            rigid: false,
            rigid_groups: true,
            auto_adjust: true,
            constrain_normal: false,
            normal: None,
            preserve_structure: false,
            base_frame: SurfaceFlowFrame::default(),
            target_frame: SurfaceFlowFrame::default(),
        }
    }
}
impl Options {
    pub fn geometry(self) -> SurfaceFlowOptions {
        SurfaceFlowOptions {
            base_frame: if self.auto_adjust {
                self.base_frame
            } else {
                SurfaceFlowFrame::default()
            },
            target_frame: if self.auto_adjust {
                self.target_frame
            } else {
                SurfaceFlowFrame::default()
            },
            normal: self.constrain_normal.then_some(self.normal).flatten(),
            preserve_structure: self.preserve_structure,
        }
    }
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        let mut args = format!(
            "Copy={} Rigid={} RigidGroups={} AutoAdjust={} ConstrainNormal={} PreserveStructure={}",
            yes(self.copy),
            yes(self.rigid),
            yes(self.rigid_groups),
            yes(self.auto_adjust),
            yes(self.constrain_normal),
            yes(self.preserve_structure)
        );
        for (name, frame) in [("Base", self.base_frame), ("Target", self.target_frame)] {
            let flags = if frame.swap_uv {
                [frame.reverse_v, frame.reverse_u]
            } else {
                [frame.reverse_u, frame.reverse_v]
            };
            let corner = match flags {
                [false, false] => 0,
                [true, false] => 1,
                [true, true] => 2,
                [false, true] => 3,
            };
            args.push_str(&format!(
                " {name}Corner={corner} {name}Swap={}",
                yes(frame.swap_uv)
            ));
        }
        if let Some(normal) = self.normal {
            args.push_str(&format!(
                " Normal={}",
                normal.to_array().map(|v| v.to_string()).join(",")
            ));
        }
        args
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub id: ObjectId,
    pub face: Option<usize>,
}
impl Reference {
    pub fn arguments(self, name: &str) -> String {
        match self.face {
            Some(face) => format!("{name}Face={},{}", self.id, face),
            None => format!("{name}={}", self.id),
        }
    }
}
/// An existing surface/face or a temporary three-point rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BaseInput {
    Object(Reference),
    Plane([Point3; 3]),
}
impl From<Reference> for BaseInput {
    fn from(value: Reference) -> Self {
        Self::Object(value)
    }
}
impl BaseInput {
    pub fn object_id(self) -> Option<ObjectId> {
        match self {
            Self::Object(r) => Some(r.id),
            Self::Plane(_) => None,
        }
    }
    pub fn arguments(self) -> String {
        match self {
            Self::Object(r) => r.arguments("Base"),
            Self::Plane(p) => format!(
                "BasePlane={}",
                p.map(|p| p.to_array().map(|v| v.to_string()).join(","))
                    .join(";")
            ),
        }
    }
}
#[derive(Default)]
pub struct References {
    pub base: Option<BaseInput>,
    pub target: Option<Reference>,
}
fn corner_frame(corner: usize, swap: bool) -> Result<SurfaceFlowFrame, CommandError> {
    let flags = match corner {
        0 => [false, false],
        1 => [true, false],
        2 => [true, true],
        3 => [false, true],
        _ => return Err(CommandError::Usage(USAGE)),
    };
    Ok(SurfaceFlowFrame {
        swap_uv: swap,
        reverse_u: flags[usize::from(swap)],
        reverse_v: flags[usize::from(!swap)],
    })
}
pub fn options(args: &[&str], mut options: Options) -> Result<(Options, References), CommandError> {
    let mut refs = References::default();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    let mut corners = [None, None];
    let mut swaps = [None, None];
    while i < args.len() {
        let (name, value) = args[i]
            .split_once('=')
            .map_or((args[i], None), |(k, v)| (k, Some(v)));
        let key = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        let canonical = match key.as_str() {
            "baseface" | "baseplane" => "base",
            "targetface" => "target",
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
            "base" | "target" | "baseface" | "targetface" => {
                let r = if key.ends_with("face") {
                    let (id, face) =
                        curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                    Reference {
                        id,
                        face: Some(face),
                    }
                } else {
                    Reference {
                        id: value.parse().map_err(|_| CommandError::Usage(USAGE))?,
                        face: None,
                    }
                };
                if key.starts_with("base") {
                    refs.base = Some(r.into())
                } else {
                    refs.target = Some(r)
                }
            }
            "baseplane" => {
                let points = value
                    .split(';')
                    .map(|v| {
                        let (p, n) = parse_point(&[v])?;
                        if n != 1 {
                            return Err(CommandError::Usage(USAGE));
                        }
                        Ok(p)
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?;
                refs.base = Some(BaseInput::Plane(
                    points.try_into().map_err(|_| CommandError::Usage(USAGE))?,
                ));
            }
            "basecorner" | "targetcorner" => {
                corners[usize::from(key.starts_with("target"))] = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                );
            }
            "baseswap" | "targetswap" => {
                swaps[usize::from(key.starts_with("target"))] =
                    Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            }
            "normal" => {
                let (p, n) = parse_point(&[value])?;
                if n != 1 {
                    return Err(CommandError::Usage(USAGE));
                }
                options.normal = Some(
                    Vector3::try_from(p.to_array())?
                        .normalized_nonzero()?
                        .as_vector(),
                );
            }
            "copy" | "rigid" | "rigidgroups" | "autoadjust" | "constrainnormal"
            | "preservestructure" => {
                let value = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                match key.as_str() {
                    "copy" => options.copy = value,
                    "rigid" => options.rigid = value,
                    "rigidgroups" => options.rigid_groups = value,
                    "autoadjust" => options.auto_adjust = value,
                    "constrainnormal" => options.constrain_normal = value,
                    "preservestructure" => options.preserve_structure = value,
                    _ => unreachable!(),
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
        i += 1;
    }
    for (index, frame) in [&mut options.base_frame, &mut options.target_frame]
        .into_iter()
        .enumerate()
    {
        let old = if frame.swap_uv {
            [frame.reverse_v, frame.reverse_u]
        } else {
            [frame.reverse_u, frame.reverse_v]
        };
        let old = match old {
            [false, false] => 0,
            [true, false] => 1,
            [true, true] => 2,
            [false, true] => 3,
        };
        *frame = corner_frame(
            corners[index].unwrap_or(old),
            swaps[index].unwrap_or(frame.swap_uv),
        )?;
    }
    Ok((options, refs))
}
pub fn capture(doc: &Document, r: Reference) -> Result<orient_surface::Target, CommandError> {
    orient_surface::Target::capture(doc, r.id, r.face).map_err(|e| match e {
        CommandError::SurfaceOrientTargetNotSurface => CommandError::SurfaceFlowNotSurface,
        other => other,
    })
}
fn surface(doc: &Document, r: Reference, adjust: bool) -> Result<Arc<NurbsSurface>, CommandError> {
    let target = capture(doc, r)?;
    if adjust && let Geometry::Brep(b) = doc.object(r.id).unwrap().geometry() {
        let shrunk = b.try_shrunk_surface_faces_with_extractor(
            &[target.face],
            viboceros_geometry::BrepSurfaceShrinkMode::Standard,
            doc.tolerance(),
            &mut surface_segments::trimmed,
        )?;
        return Ok(Arc::new(shrunk.faces()[target.face].surface().clone()));
    }
    Ok(target.surface)
}
/// Drawn planes retain length-based parameter domains, including exterior offsets.
pub fn plane_surface(
    [origin, x_point, y_point]: [Point3; 3],
) -> Result<Arc<NurbsSurface>, CommandError> {
    let x = origin.vector_to(x_point)?;
    let y = origin.vector_to(y_point)?;
    let frame = viboceros_geometry::Frame3::try_from_directions(
        origin,
        x,
        y,
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    let width = x.length()?;
    let height = y.dot(frame.y_axis().as_vector())?;
    if width <= 0. || height <= 0. {
        return Err(CommandError::Usage(USAGE));
    }
    let v = frame.y_axis().as_vector().scaled(height)?;
    let y_point = origin.translated(v)?;
    let opposite = x_point.translated(v)?;
    Ok(Arc::new(
        NurbsSurface::try_bilinear([origin, x_point, opposite, y_point])?
            .try_reparameterized(0. ..=width, 0. ..=height)?,
    ))
}
/// Derive the picked corner and incident U edge on the effective surface.
pub fn frame_at_pick(
    doc: &Document,
    r: Reference,
    point: Point3,
    adjust: bool,
) -> Result<SurfaceFlowFrame, CommandError> {
    let surface = surface(doc, r, adjust)?;
    let (u, v) = surface.closest_parameters(point, Tolerance::NUMERICAL_VALIDATION)?;
    let mut frame = SurfaceFlowFrame::from_corner_pick(&surface, [u, v])?;
    let normalized = [
        (u - surface.domain_u().start()) / (surface.domain_u().end() - surface.domain_u().start()),
        (v - surface.domain_v().start()) / (surface.domain_v().end() - surface.domain_v().start()),
    ];
    let distances = normalized.map(|p| p.min(1. - p));
    if distances[0] < distances[1] {
        frame.swap_uv = true;
        std::mem::swap(&mut frame.reverse_u, &mut frame.reverse_v);
    }
    Ok(frame)
}
pub fn prepare(
    doc: &Document,
    base: impl Into<BaseInput>,
    target: Reference,
    options: Options,
) -> Result<SurfaceFlowPointMorph, CommandError> {
    if options.constrain_normal && options.normal.is_none() {
        return Err(CommandError::SurfaceFlowNormalRequired);
    }
    let mut morph = SurfaceFlowPointMorph::try_new(
        match base.into() {
            BaseInput::Object(r) => surface(doc, r, options.auto_adjust)?,
            BaseInput::Plane(points) => plane_surface(points)?,
        },
        surface(doc, target, options.auto_adjust)?,
        options.geometry(),
        doc.tolerance(),
    )?;
    if let Some(solver) = morph_interpolation_solver() {
        morph = morph.with_interpolation_solver(solver);
    }
    Ok(morph)
}
impl crate::morph_edit::ObjectMorph for SurfaceFlowPointMorph {
    fn rigid_pose(
        &self,
        center: Point3,
    ) -> Result<viboceros_geometry::AffineTransform3, GeometryError> {
        self.rigid_transform(center)
    }
    fn preserving(self, preserve: bool) -> Self {
        self.with_preserve_structure(preserve)
    }
}
pub fn deformed_with_grips(
    doc: &Document,
    ids: &[ObjectId],
    grips: &[ControlPointId],
    morph: &SurfaceFlowPointMorph,
    options: Options,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    crate::morph_edit::deformed(doc, ids, grips, morph, options.rigid, options.rigid_groups)
}
#[derive(Default)]
pub(super) struct SurfaceFlowCommand(remembered::Remembered<Options>);
impl Command for SurfaceFlowCommand {
    fn name(&self) -> &'static str {
        "FlowAlongSrf"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["FlowAlongSurface"]
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(true)
    }
    fn surface_flow_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_surface_flow_options(&self, value: Options) -> bool {
        self.0.set(Options {
            copy: true,
            normal: None,
            base_frame: SurfaceFlowFrame::default(),
            target_frame: SurfaceFlowFrame::default(),
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
            .retain(|id| Some(*id) != base.object_id() && *id != target.id);
        if sources
            .grips
            .iter()
            .any(|g| Some(g.object) == base.object_id() || g.object == target.id)
        {
            return Err(CommandError::Usage(
                "FlowAlongSrf references cannot supply source grips",
            ));
        }
        if sources.ids.is_empty() && sources.grips.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let morph = prepare(doc, base, target, options)?;
        let staged = deformed_with_grips(doc, &sources.ids, &sources.grips, &morph, options)?;
        let count = staged.len()
            + sources
                .grips
                .iter()
                .map(|g| g.object)
                .collect::<BTreeSet<_>>()
                .len();
        let ids = staged.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        if sources.release_on_replay {
            doc.release_command_selection_on_history_replay(sources.ids.iter().copied())?;
            if options.copy || sources.grips.is_empty() {
                doc.clear_selection();
            }
        }
        if !sources.grips.is_empty() {
            doc.morph_grips_with_object_geometries(
                staged,
                sources.grips.iter().copied(),
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
            doc.move_objects_to_end_in_order(ids)?;
        }
        doc.select_objects_direct(sources.ids.iter().copied(), SelectionMode::Replace)?;
        if sources.postselected {
            doc.clear_selection();
        }
        self.remember_surface_flow_options(options);
        Ok(format!(
            "Flowed {count} object(s) along surface{}",
            if options.copy {
                ", creating copies"
            } else {
                ""
            }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn setup() -> (CommandRegistry, Document, [ObjectId; 2], ObjectId) {
        let mut d = Document::default();
        let guides = [0., 20.].map(|x| {
            d.add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    p(x, 0., 0.),
                    p(x + 10., 0., 0.),
                    p(x + 10., 10., 0.),
                    p(x, 10., 0.),
                ])
                .unwrap(),
            ))
            .unwrap()
        });
        let source = d.add_geometry(Geometry::Point(p(2., 3., 1.))).unwrap();
        d.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        d.clear_history().unwrap();
        (CommandRegistry::with_builtins(), d, guides, source)
    }
    #[test]
    fn surface_flow_duplicate_references_invalid_options_and_singular_targets_are_atomic() {
        let (r, mut d, [a, b], source) = setup();
        for tail in [
            format!("Base={a} BaseFace={a},0 Target={b}"),
            format!("Base={a} Target={b} BaseCorner=4"),
            format!("Base={a} Target={b} ConstrainNormal=Yes"),
            format!("Base={a} Target={b} Normal=0,0,0"),
        ] {
            let before = format!("{d:?}");
            assert!(r.execute(&mut d, &format!("FlowAlongSrf {tail}")).is_err());
            assert_eq!(format!("{d:?}"), before);
            assert_eq!(r.surface_flow_options(), Options::default());
        }
        let singular = d
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([p(0., 0., 0.); 4]).unwrap(),
            ))
            .unwrap();
        d.clear_history().unwrap();
        let before = format!("{d:?}");
        assert!(
            r.execute(
                &mut d,
                &format!("FlowAlongSrf Base={a} Target={singular} Copy=No")
            )
            .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
        assert!(d.is_selected(source));
    }
    #[test]
    fn surface_flow_normal_and_all_frames_round_trip_and_preferences_reset_per_getter() {
        for corner in 0..4 {
            for swap in ["Yes", "No"] {
                let args = format!(
                    "BaseCorner={corner} BaseSwap={swap} TargetCorner={corner} TargetSwap={swap} ConstrainNormal=Yes Normal=0,0,2"
                );
                let (o, _) = options(
                    &args.split_whitespace().collect::<Vec<_>>(),
                    Options::default(),
                )
                .unwrap();
                assert_eq!(o.normal, Some(Vector3::try_new(0., 0., 1.).unwrap()));
                assert_eq!(
                    options(
                        &o.arguments().split_whitespace().collect::<Vec<_>>(),
                        Options::default()
                    )
                    .unwrap()
                    .0,
                    o
                );
            }
        }
        let (r, mut d, [a, b], _) = setup();
        r.execute(&mut d,&format!("FlowAlongSurface Base={a} Target={b} Copy=No ConstrainNormal=Yes Normal=0,0,2 BaseCorner=1")).unwrap();
        let o = r.surface_flow_options();
        assert!(o.copy && o.constrain_normal);
        assert!(o.normal.is_none());
        assert_eq!(o.base_frame, SurfaceFlowFrame::default());
        assert_eq!(d.undo_label(), Some("FlowAlongSrf"));
    }
}
