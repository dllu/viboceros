//! Rigid curve placement on an immutable surface boundary.
use super::*;
use crate::curve_reference::CapturedCurve;
use crate::orient_surface::Target;

pub const USAGE: &str = "OrientCrvToEdge target [Edge=id,index] [Face=index] [PickPoint=x,y,z] [Copy=Yes|No] [FlipSurface=Yes|No] [ReverseCurve=Yes|No] [Sources=id|PickedSources=id]";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub copy: bool,
    pub flip_surface: bool,
    pub reverse_curve: bool,
}
impl Options {
    pub fn arguments(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        format!(
            "Copy={} FlipSurface={} ReverseCurve={}",
            yes(self.copy),
            yes(self.flip_surface),
            yes(self.reverse_curve)
        )
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reference {
    pub edge: Option<(ObjectId, usize)>,
    pub face: Option<usize>,
    pub pick: Option<Point3>,
}
pub fn options<'a>(
    args: &[&'a str],
    mut options: Options,
) -> Result<(Vec<&'a str>, Options, Reference), CommandError> {
    let mut reference = Reference::default();
    let mut rest = Vec::new();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i]
            .split('=')
            .next()
            .unwrap()
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase();
        if ![
            "copy",
            "flipsurface",
            "reversecurve",
            "edge",
            "face",
            "pickpoint",
        ]
        .contains(&key.as_str())
        {
            rest.push(args[i]);
            i += 1;
            continue;
        }
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        let (_, value, n) = orient_option(args, i, USAGE)?;
        match key.as_str() {
            "copy" => options.copy = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
            "flipsurface" => {
                options.flip_surface = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "reversecurve" => {
                options.reverse_curve = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "edge" => {
                reference.edge =
                    Some(curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?)
            }
            "face" => reference.face = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
            "pickpoint" => reference.pick = Some(parse_single_option_point(value, USAGE)?),
            _ => unreachable!(),
        }
        i += n;
    }
    Ok((rest, options, reference))
}

/// The curve and its picked endpoint remain fixed across repeated placements.
#[derive(Clone, Debug)]
pub struct Source {
    pub curve: CapturedCurve,
    pub at_end: bool,
}
impl Source {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        pick: Option<Point3>,
    ) -> Result<Self, CommandError> {
        let curve = CapturedCurve::capture(doc, id, None)?;
        let c = curve.curve_ref();
        if c.is_closed()? {
            return Err(CommandError::Usage("Select an open curve near an end"));
        }
        let d = c.domain();
        // Public command captures retain the end opposite the located source pick.
        let at_end = pick
            .map(|p| {
                c.closest_parameter(p, doc.tolerance())
                    .map(|t| t <= d.start() + (d.end() - d.start()) * 0.5)
            })
            .transpose()?
            .unwrap_or(false);
        Ok(Self { curve, at_end })
    }
    pub fn base(&self) -> Result<CurveSample, GeometryError> {
        let c = self.curve.curve_ref();
        let d = c.domain();
        c.evaluate_with_tangent(if self.at_end { *d.end() } else { *d.start() })
    }
}

/// The chosen edge and face must belong to the same captured parent topology.
#[derive(Clone, Debug)]
pub struct Edge {
    pub guide: CapturedCurve,
    pub target: Target,
    pub reversed_trim: bool,
}
impl Edge {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        index: usize,
        face: Option<usize>,
    ) -> Result<Self, CommandError> {
        let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
        let converted;
        let brep = match object.geometry() {
            Geometry::Brep(b) => b,
            Geometry::NurbsSurface(s) => {
                converted = Brep::try_surface_face(s.clone(), doc.tolerance())?;
                &converted
            }
            _ => return Err(CommandError::Usage("Select a surface edge")),
        };
        let adjacent = brep
            .faces()
            .iter()
            .enumerate()
            .flat_map(|(i, f)| {
                f.loops().iter().flat_map(move |l| {
                    l.trims()
                        .iter()
                        .filter(move |t| t.edge() == Some(index))
                        .map(move |t| (i, t.is_reversed_3d()))
                })
            })
            .collect::<Vec<_>>();
        let (face, reversed_trim) = if let Some(face) = face {
            adjacent
                .iter()
                .find(|(i, _)| *i == face)
                .copied()
                .ok_or(CommandError::Usage(
                    "Face must be adjacent to the selected edge",
                ))?
        } else {
            if adjacent
                .iter()
                .any(|(f, _)| Some(*f) != adjacent.first().map(|(f, _)| *f))
            {
                return Err(CommandError::Usage(
                    "Specify Face=index to choose the surface on a shared edge",
                ));
            }
            *adjacent
                .first()
                .ok_or(CommandError::Usage("Select a surface edge"))?
        };
        if adjacent.iter().filter(|(i, _)| *i == face).count() != 1 {
            return Err(CommandError::Usage(
                "Seam edges with multiple trims on one face are not supported yet",
            ));
        }
        Ok(Self {
            guide: CapturedCurve::capture(doc, id, Some(index))?,
            target: Target::capture(doc, id, Some(face))?,
            reversed_trim,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.guide.current(doc) && self.target.current(doc)
    }
}

#[derive(Clone, Debug)]
pub struct Placement {
    pub source: Source,
    pub edge: Edge,
    pub options: Options,
    plane: Frame3,
    tolerance: Tolerance,
}
impl Placement {
    pub fn new(
        source: Source,
        edge: Edge,
        options: Options,
        plane: Frame3,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            source,
            edge,
            options,
            plane,
            tolerance,
        }
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.source.curve.current(doc) && self.edge.current(doc)
    }
    pub fn curve(&self) -> CurveRef<'_> {
        self.edge.guide.curve_ref()
    }
    pub fn locate(&self, point: Point3) -> Result<CurveSample, GeometryError> {
        let c = self.curve();
        c.evaluate_with_tangent(c.closest_parameter(point, self.tolerance)?)
    }
    fn edge_frame(&self, sample: CurveSample) -> Result<Frame3, CommandError> {
        let normal = self.edge.target.source_normal(sample.point(), true)?;
        let tangent = sample.tangent().as_vector().scaled(
            if self.edge.reversed_trim ^ self.edge.target.reversed {
                -1.
            } else {
                1.
            },
        )?;
        Ok(Frame3::try_from_x_and_normal(
            sample.point(),
            normal.cross(tangent)?,
            normal,
            Tolerance::NUMERICAL_VALIDATION,
        )?)
    }
    pub fn transform_at(&self, point: Point3) -> Result<AffineTransform3, CommandError> {
        let source = self.source.base()?;
        let nearest = self.locate(source.point())?;
        let source_frame = if nearest.point().distance_to(source.point())?
            <= self.tolerance.absolute()
        {
            self.edge_frame(nearest)?.with_origin(source.point())
        } else {
            let tangent =
                source
                    .tangent()
                    .as_vector()
                    .scaled(if self.source.at_end { 1. } else { -1. })?;
            let up = self.plane.z_axis().as_vector().scaled(-1.)?;
            if tangent.dot(up)?.abs() > self.tolerance.angular() {
                return Err(CommandError::Usage(
                    "This placement requires a source tangent in the CPlane, or a source endpoint on the target edge",
                ));
            }
            Frame3::try_from_x_and_normal(
                source.point(),
                tangent,
                up,
                Tolerance::NUMERICAL_VALIDATION,
            )?
        };
        // Public assignments and bare toggles leave these qualified mappings
        // unchanged in Rhino 8.32; do not infer a new roll or endpoint from them.
        let target = self.edge_frame(self.locate(point)?)?;
        Ok(AffineTransform3::try_frame_mapping(
            source_frame,
            target,
            [1.; 3],
        )?)
    }
}

pub(super) struct OrientCurveEdgeCommand;
impl Command for OrientCurveEdgeCommand {
    fn name(&self) -> &'static str {
        "OrientCrvToEdge"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn history_policy(&self) -> CommandHistoryPolicy {
        CommandHistoryPolicy::TransformedObjects
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
        let (args, sources) = history_policy::transform_arguments(doc, args, USAGE)?;
        let [id] = sources.ids.as_slice() else {
            return Err(CommandError::Usage("Select one open curve near an end"));
        };
        let (rest, options, reference) = options(&args, Options::default())?;
        let (point, n) = parse_point(&rest)?;
        if n != rest.len() {
            return Err(CommandError::Usage(USAGE));
        }
        let (target, index) = reference.edge.ok_or(CommandError::Usage(USAGE))?;
        if *id == target {
            return Err(CommandError::Usage("Select a separate surface edge"));
        }
        let source = Source::capture(doc, *id, reference.pick)?;
        let edge = Edge::capture(doc, target, index, reference.face)?;
        let placement = Placement::new(
            source,
            edge,
            options,
            context.construction_plane,
            doc.tolerance(),
        );
        let map = placement.transform_at(point)?;
        if options.copy {
            // This command adds copies to the existing source groups, unlike
            // ordinary affine Copy, which allocates new group definitions.
            let geometry = doc
                .object(*id)
                .unwrap()
                .geometry()
                .transformed(map, doc.tolerance())?;
            if sources.release_on_replay {
                doc.release_command_selection_on_history_replay([*id])?;
            }
            doc.copy_object_geometries_in_source_groups([(*id, geometry)])?;
            doc.select_command_results([*id])?;
        } else {
            history_policy::apply_transform_with_renewal(doc, &sources, map, false)?;
        }
        Ok(if options.copy {
            "Copied curve to surface edge"
        } else {
            "Oriented curve to surface edge"
        }
        .into())
    }
}
