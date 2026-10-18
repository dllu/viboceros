//! Rigid curve-guided placement shared by complete commands and cursor previews.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{Curve3, ParameterSide};

pub const USAGE: &str = "OrientOnCrv base-point target-point (Curve=object-id|Edge=parent-id,index) [SubCrv=start-parameter,end-parameter] [OnCurve=Yes|No] [Copy=Yes|No] [Perpendicular=Yes|No] [Flip=Yes|No] [XFlip=Yes|No] [YFlip=Yes|No] [Rotation=degrees]";

/// Extract a single temporary guide interval; parameters belong to the original curve.
pub fn range_arguments<'a>(
    args: &[&'a str],
) -> Result<(Vec<&'a str>, Option<[Real; 2]>), CommandError> {
    let mut rest = Vec::new();
    let mut range = None;
    let mut i = 0;
    while i < args.len() {
        let token = args[i];
        let (name, value) = token
            .split_once('=')
            .map_or((token, None), |(k, v)| (k, Some(v)));
        if name
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("SubCrv")
        {
            if range.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let value = match value {
                Some(v) => v,
                None => {
                    i += 1;
                    *args.get(i).ok_or(CommandError::Usage(USAGE))?
                }
            };
            let values = value.split(',').collect::<Vec<_>>();
            let [start, end] = values.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            let interval = [parse_finite_real(start)?, parse_finite_real(end)?];
            if interval[0] == interval[1] {
                return Err(CommandError::Usage(USAGE));
            }
            range = Some(interval);
        } else {
            rest.push(token);
        }
        i += 1;
    }
    Ok((rest, range))
}

/// Extract one explicit surface-edge reference without changing the document.
pub fn edge_arguments<'a>(
    args: &[&'a str],
) -> Result<(Vec<&'a str>, Option<curve_reference::EdgeReference>), CommandError> {
    let mut rest = Vec::new();
    let mut edge = None;
    let mut i = 0;
    while i < args.len() {
        let token = args[i];
        let (key, value) = token
            .split_once('=')
            .map_or((token, None), |(k, v)| (k, Some(v)));
        if key
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Edge")
        {
            if edge.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let value = match value {
                Some(v) => v,
                None => {
                    i += 1;
                    *args.get(i).ok_or(CommandError::Usage(USAGE))?
                }
            };
            edge = Some(curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?);
        } else {
            rest.push(token);
        }
        i += 1;
    }
    Ok((rest, edge))
}

/// Prepare privately with SubCrv's shared backend (usd-brep when enabled).
pub fn prepare_guide(curve: &Curve3, range: Option<[Real; 2]>) -> Result<Curve3, CommandError> {
    match range {
        Some([start, end]) => curve_segments::directed(curve, start, end),
        None => Ok(curve.clone()),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub on_curve: bool,
    pub perpendicular: bool,
    pub flip: bool,
    pub x_flip: bool,
    pub y_flip: bool,
    pub rotation: Real,
}
impl Options {
    fn validate(self) -> Result<(), CommandError> {
        if !self.rotation.is_finite()
            || !self.perpendicular && (self.x_flip || self.y_flip || self.rotation != 0.)
        {
            Err(CommandError::Usage(USAGE))
        } else {
            Ok(())
        }
    }
    pub fn arguments(self) -> String {
        let yes = |x| if x { "Yes" } else { "No" };
        format!(
            "Copy={} OnCurve={} Perpendicular={} Flip={} XFlip={} YFlip={} Rotation={}",
            yes(self.copy),
            yes(self.on_curve),
            yes(self.perpendicular),
            yes(self.flip),
            yes(self.x_flip),
            yes(self.y_flip),
            self.rotation
        )
    }
}

/// Parse placement options without accepting preferences or model edits.
pub fn options<'a>(
    args: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Options, Option<ObjectId>), CommandError> {
    let mut rest = Vec::new();
    let mut seen = BTreeSet::new();
    let mut curve = None;
    let mut i = 0;
    while i < args.len() {
        let token = args[i];
        let (key, inline) = token
            .split_once('=')
            .map_or((token, None), |(a, b)| (a, Some(b)));
        let key = key.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if [
            "copy",
            "oncurve",
            "perpendicular",
            "flip",
            "xflip",
            "yflip",
            "rotation",
            "curve",
        ]
        .contains(&key.as_str())
        {
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            let value = if let Some(v) = inline {
                v
            } else {
                i += 1;
                *args.get(i).ok_or(CommandError::Usage(USAGE))?
            };
            let yes = || parse_yes_no(value).ok_or(CommandError::Usage(USAGE));
            match key.as_str() {
                "copy" => options.copy = yes()?,
                "oncurve" => options.on_curve = yes()?,
                "perpendicular" => options.perpendicular = yes()?,
                "flip" => options.flip = yes()?,
                "xflip" => options.x_flip = yes()?,
                "yflip" => options.y_flip = yes()?,
                "rotation" => options.rotation = parse_finite_real(value)?,
                "curve" => curve = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                _ => unreachable!(),
            }
        } else {
            rest.push(token);
        }
        i += 1;
    }
    options.validate()?;
    Ok((rest, options, curve))
}

#[derive(Clone, Debug)]
pub struct Placement {
    curve: Arc<Curve3>,
    reversed: Arc<Curve3>,
    normal: Option<UnitVector3>,
    reversed_normal: Option<UnitVector3>,
    source: Frame3,
    options: Options,
    tolerance: Tolerance,
}
impl Placement {
    pub fn try_new(
        curve: Curve3,
        base: Point3,
        plane: Frame3,
        options: Options,
        tolerance: Tolerance,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        let normal = curve.as_ref().supporting_plane_normal(tolerance)?;
        let reversed = curve.reversed(tolerance)?;
        let reversed_normal = reversed.as_ref().supporting_plane_normal(tolerance)?;
        let source = if options.on_curve {
            let t = curve.as_ref().closest_parameter(base, tolerance)?;
            curve_frame(curve.as_ref(), normal, t, tolerance)?
        } else {
            plane.with_origin(base)
        };
        Ok(Self {
            curve: Arc::new(curve),
            reversed: Arc::new(reversed),
            normal,
            reversed_normal,
            source,
            options,
            tolerance,
        })
    }
    pub fn options(&self) -> Options {
        self.options
    }
    pub fn curve(&self) -> CurveRef<'_> {
        if self.options.flip {
            self.reversed.as_ref().as_ref()
        } else {
            self.curve.as_ref().as_ref()
        }
    }
    pub fn with_options(&self, options: Options) -> Result<Self, CommandError> {
        options.validate()?;
        if self.options.on_curve != options.on_curve {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self {
            options,
            ..self.clone()
        })
    }
    pub fn transform_at(&self, point: Point3) -> Result<AffineTransform3, CommandError> {
        Ok(AffineTransform3::try_frame_mapping(
            self.source,
            self.target_frame(point)?,
            [1.; 3],
        )?)
    }
    pub fn target_frame(&self, point: Point3) -> Result<Frame3, CommandError> {
        let t = self.curve().closest_parameter(point, self.tolerance)?;
        let frame = curve_frame(
            self.curve(),
            if self.options.flip {
                self.reversed_normal
            } else {
                self.normal
            },
            t,
            self.tolerance,
        )?;
        let mut x = frame.x_axis();
        let mut y = frame.y_axis();
        let mut z = frame.z_axis();
        if self.options.perpendicular {
            (x, y, z) = (y, z, x);
        }
        if self.options.x_flip {
            x = x.opposite();
            z = z.opposite();
        }
        if self.options.y_flip {
            y = y.opposite();
            z = z.opposite();
        }
        let mut target = Frame3::try_from_unit_directions(frame.origin(), x, y, self.tolerance)?;
        if self.options.rotation != 0. {
            let rotation = AffineTransform3::try_rotation(
                frame.origin(),
                z,
                self.options.rotation.to_radians(),
            )?;
            target = Frame3::try_from_unit_directions(
                frame.origin(),
                rotation
                    .transform_vector(target.x_axis().as_vector())?
                    .normalized_nonzero()?,
                rotation
                    .transform_vector(target.y_axis().as_vector())?
                    .normalized_nonzero()?,
                self.tolerance,
            )?;
        }
        Ok(target)
    }
    pub fn rotation_from_points(
        &self,
        target: Point3,
        reference: Point3,
        point: Point3,
    ) -> Result<Real, CommandError> {
        let neutral = self.with_options(Options {
            rotation: 0.,
            ..self.options
        })?;
        let frame = neutral.target_frame(target)?;
        Ok(
            plane_transforms::plane_angle(frame, frame.origin(), reference, point, self.tolerance)?
                .to_degrees(),
        )
    }
    pub fn rotated_at(
        &self,
        target: Point3,
        reference: Point3,
        point: Point3,
    ) -> Result<AffineTransform3, CommandError> {
        let rotation = self.rotation_from_points(target, reference, point)?;
        self.with_options(Options {
            rotation,
            ..self.options
        })?
        .transform_at(target)
    }
}

fn curve_frame(
    curve: CurveRef<'_>,
    normal: Option<UnitVector3>,
    parameter: Real,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    if normal.is_none() {
        let start = *curve.domain().start();
        let tangent = curve
            .evaluate_with_tangent_on_side(start, ParameterSide::Right)?
            .tangent();
        let vertical = Vector3::try_new(0., 0., 1.)?;
        let cross = vertical.cross(tangent.as_vector())?;
        let seed = if cross.length()? > 64. * Real::EPSILON {
            cross.normalized_nonzero()?
        } else {
            Frame3::try_from_unit_normal(curve.evaluate(start)?, tangent)?.x_axis()
        };
        let parameters = if parameter == start {
            vec![start]
        } else {
            vec![start, parameter]
        };
        let frame = curve
            .rotation_minimizing_frames(
                &parameters,
                Some(seed),
                viboceros_geometry::FrameTransportOptions {
                    side: ParameterSide::Right,
                    angular_tolerance: 1e-10,
                    ..Default::default()
                },
            )?
            .pop()
            .unwrap();
        return Frame3::try_from_unit_directions(
            frame.origin(),
            frame.z_axis(),
            frame.x_axis(),
            tolerance,
        );
    }
    let side = if parameter == *curve.domain().end() {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    let sample = curve.evaluate_with_tangent_on_side(parameter, side)?;
    let y = if let Some(normal) = normal {
        normal
            .as_vector()
            .cross(sample.tangent().as_vector())?
            .normalized_nonzero()?
    } else {
        unreachable!("spatial curve frames are transported above")
    };
    Frame3::try_from_unit_directions(sample.point(), sample.tangent(), y, tolerance)
}

#[derive(Default)]
pub(super) struct OrientCurveCommand(remembered::Remembered<Real>);
impl Command for OrientCurveCommand {
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
    }
    fn scalar_default(&self) -> Option<Real> {
        Some(self.0.get())
    }
    fn remember_pending_scalar(&self, value: Real) -> bool {
        self.0.set(value);
        true
    }
    fn name(&self) -> &'static str {
        "OrientOnCrv"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["OrientOnCurve"]
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
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
        let (args, selected) = transform_arguments(doc, args, USAGE)?;
        let (args, range) = range_arguments(&args)?;
        let (args, edge) = edge_arguments(&args)?;
        let (rest, options, curve) = options(&args, Options::default())?;
        let (base, n) = parse_point(&rest)?;
        let (target, m) = parse_point(&rest[n..])?;
        require_consumed(&rest, n + m, USAGE)?;
        if curve.is_some() && edge.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let (curve, index) = edge
            .map(|(id, index)| (id, Some(index)))
            .or_else(|| curve.map(|id| (id, None)))
            .ok_or(CommandError::Usage(USAGE))?;
        let object = doc
            .object(curve)
            .filter(|_| index.is_none() || doc.is_object_selectable(curve))
            .ok_or(DocumentError::ObjectNotFound(curve))?;
        let resolved = curve_reference::resolve(object.geometry(), index, doc.tolerance())
            .ok_or(CommandError::Usage(USAGE))?;
        let guide = resolved.curve().to_owned();
        if selected.ids.contains(&curve) {
            return Err(CommandError::Usage(
                "The orientation curve must be separate from the source objects",
            ));
        }
        if selected.ids.is_empty() {
            return Err(CommandError::Usage(
                "OrientOnCrv requires selected source objects",
            ));
        }
        let placement = Placement::try_new(
            if range.is_some() {
                prepare_guide(&guide, range)?
            } else {
                guide
            },
            base,
            context.construction_plane,
            options,
            doc.tolerance(),
        )?;
        let transform = placement.transform_at(target)?;
        let (transformed, copied) =
            apply_transform_with_renewal(doc, &selected, transform, options.copy)?;
        if range.is_some() {
            doc.clear_selection();
        }
        if args
            .iter()
            .any(|a| option_name_eq(a.split('=').next().unwrap(), "Rotation"))
        {
            self.0.set(options.rotation);
        }
        Ok(format!(
            "Oriented {transformed} object(s) along curve; {copied} copies created"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(p: [Real; 3]) -> Point3 {
        Point3::try_from(p).unwrap()
    }
    fn setup() -> (CommandRegistry, Document, ObjectId, ObjectId) {
        let mut doc = Document::default();
        let source = doc
            .add_geometry(Geometry::Point(point([1., 0., 0.])))
            .unwrap();
        let guide = doc
            .add_geometry(Geometry::Line(
                LineSegment::try_new(point([0.; 3]), point([0., 10., 0.]), doc.tolerance())
                    .unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        (CommandRegistry::with_builtins(), doc, source, guide)
    }
    #[test]
    fn orient_on_curve_invalid_arguments_and_failed_geometry_are_atomic() {
        let (registry, mut doc, source, guide) = setup();
        let before = format!("{doc:?}");
        for args in [
            format!("0,0,0 0,3,0 Curve={guide} Copy=Maybe"),
            format!("0,0,0 0,3,0 Curve={guide} Flip=Yes Flip=No"),
            format!("0,0,0 0,3,0 Curve={guide} XFlip=Yes"),
            format!("0,0,0 0,3,0 Curve={guide} Rotation=NaN"),
            format!("0,0,0 0,3,0 Curve={source}"),
            format!("0,0,0 0,3,0 Curve={guide} Garbage=Yes"),
            format!("0,0,0 0,3,0 Curve={guide} SubCrv=1,1"),
            format!("0,0,0 0,3,0 Curve={guide} SubCrv=-1,2"),
            format!("0,0,0 0,3,0 Curve={guide} SubCrv=NaN,4"),
            format!("0,0,0 0,3,0 Curve={guide} SubCrv=1,3 SubCrv=2,4"),
            format!("0,0,0 0,3,0 Curve={guide} Edge={guide},0"),
            format!("0,0,0 0,3,0 Edge={guide},0 Edge={guide},1"),
            format!("0,0,0 0,3,0 Edge={source},0"),
            format!("0,0,0 0,3,0 Edge={guide},-1"),
        ] {
            assert!(
                registry
                    .execute(&mut doc, &format!("OrientOnCrv {args}"))
                    .is_err()
            );
            assert_eq!(format!("{doc:?}"), before);
            assert_eq!(registry.copy_default("OrientOnCrv"), Some(false));
        }
    }
    #[test]
    fn orient_on_curve_edge_reference_retains_parent_and_metadata_with_and_without_copy() {
        for copy in [false, true] {
            let (registry, mut doc, source, _) = setup();
            let parent = doc
                .add_geometry(Geometry::Brep(
                    viboceros_geometry::Brep::try_box(
                        CommandContext::default().construction_plane,
                        [[0., 4.], [0., 6.], [0., 2.]],
                        doc.tolerance(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            doc.set_object_names([(parent, Some("edge parent".into()))])
                .unwrap();
            doc.add_group(Some("parent".into()), [parent]).unwrap();
            let original = doc.object(parent).unwrap().clone();
            doc.clear_history().unwrap();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "OrientOnCrv 0,0,0 2,0,0 Edge={parent},0 Copy={}",
                        if copy { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            assert_eq!(doc.object(parent).unwrap(), &original);
            assert!(
                doc.objects()
                    .any(|o| o.geometry() == &Geometry::Point(point([3., 0., 0.])))
            );
            if copy {
                assert_eq!(
                    doc.object(source).unwrap().geometry(),
                    &Geometry::Point(point([1., 0., 0.]))
                );
            }
            assert_eq!(doc.undo().unwrap().as_deref(), Some("OrientOnCrv"));
            assert_eq!(doc.object(parent).unwrap(), &original);
            assert_eq!(
                doc.object(source).unwrap().geometry(),
                &Geometry::Point(point([1., 0., 0.]))
            );
        }
    }
    #[test]
    fn orient_on_curve_alias_copy_preferences_and_undo_redo_use_the_common_history() {
        let (registry, mut doc, source, guide) = setup();
        registry
            .execute(
                &mut doc,
                &format!("_OrientOnCurve 0,0,0 0,3,0 Curve={guide} Copy=Yes"),
            )
            .unwrap();
        assert_eq!(doc.objects().count(), 3);
        assert_eq!(
            doc.object(source).unwrap().geometry(),
            &Geometry::Point(point([1., 0., 0.]))
        );
        assert!(
            doc.objects()
                .any(|o| o.geometry() == &Geometry::Point(point([0., 4., 0.])))
        );
        assert_eq!(registry.copy_default("OrientOnCrv"), Some(true));
        assert_eq!(doc.undo().unwrap().as_deref(), Some("OrientOnCrv"));
        assert_eq!(doc.objects().count(), 2);
        doc.redo().unwrap();
        assert_eq!(doc.objects().count(), 3);
        assert!(doc.is_selected(source));
        assert!(!doc.is_selected(guide));
    }
    #[test]
    fn orient_on_curve_reference_rotation_rejects_zero_and_preserves_rigid_shape() {
        let (_, doc, _, guide) = setup();
        let p = Placement::try_new(
            doc.object(guide)
                .unwrap()
                .geometry()
                .curve_ref()
                .unwrap()
                .to_owned(),
            point([0.; 3]),
            CommandContext::default().construction_plane,
            Options {
                perpendicular: true,
                ..Default::default()
            },
            doc.tolerance(),
        )
        .unwrap();
        let target = point([0., 3., 0.]);
        assert!(
            p.rotation_from_points(target, target, point([1., 3., 0.]))
                .is_err()
        );
        let angle = p
            .rotation_from_points(target, point([-1., 3., 0.]), point([0., 3., 1.]))
            .unwrap();
        assert!((angle - 90.).abs() < 1e-12);
        let map = p
            .rotated_at(target, point([-1., 3., 0.]), point([0., 3., 1.]))
            .unwrap();
        assert!(
            map.transform_point(point([1., 0., 0.]))
                .unwrap()
                .distance_to(point([0., 3., 1.]))
                .unwrap()
                < 1e-12
        );
        assert!(!map.orientation_reversing().unwrap());
    }
}
