//! MatchSrf: captured complete boundary picks, readonly fitting and one acceptance.
use super::*;
use viboceros_document::{Object, ReplacementHistory};
use viboceros_geometry::{NurbsSurface, SurfaceExtensionEdge};
mod options;
pub use options::{Options, parse};
pub const USAGE: &str = "MatchSrf object-id edge-index pick-x,y,z object-id edge-index pick-x,y,z [Mode=Position|Tangency|Curvature] [PreserveOtherEnd=Position] [MatchByClosestPoints=Yes|No] [RefineMatch=Yes|No] [IsocurveDirection=Automatic|Keep|MatchTarget|Perpendicular]";

#[derive(Clone, Debug)]
pub struct Edge {
    source: Object,
    index: usize,
    point: Point3,
    side: SurfaceExtensionEdge,
    reversed: bool,
    tolerance: Tolerance,
    units: viboceros_geometry::LengthUnitSystem,
}
impl Edge {
    /// Only complete rectangular boundaries on single untrimmed faces are
    /// currently supported. Capture also checks editability and topology.
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        index: usize,
        point: Point3,
    ) -> Result<Self, CommandError> {
        let source = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
        if !doc.is_object_selectable(id) {
            return Err(CommandError::Usage("Pick an editable surface edge"));
        }
        let converted;
        let brep = match source.geometry() {
            Geometry::Brep(b) => b,
            Geometry::NurbsSurface(s) => {
                converted = Brep::try_surface_face(s.clone(), doc.tolerance())?;
                &converted
            }
            _ => return Err(CommandError::Usage("MatchSrf requires a surface edge")),
        };
        if brep.faces().len() != 1 || !brep.faces()[0].is_untrimmed(doc.tolerance())? {
            return Err(CommandError::Usage(
                "MatchSrf currently requires single untrimmed surfaces",
            ));
        }
        let face = &brep.faces()[0];
        let s = face.surface();
        let u = [*s.domain_u().start(), *s.domain_u().end()];
        let v = [*s.domain_v().start(), *s.domain_v().end()];
        let trims = face
            .loops()
            .iter()
            .flat_map(|l| l.trims())
            .filter(|t| t.edge() == Some(index))
            .collect::<Vec<_>>();
        let [trim] = trims.as_slice() else {
            return Err(CommandError::Usage("Pick a complete naked boundary edge"));
        };
        let a = trim.curve().start_point()?;
        let b = trim.curve().end_point()?;
        let side = if a.x() == b.x() && [a.y().min(b.y()), a.y().max(b.y())] == v {
            if a.x() == u[0] {
                SurfaceExtensionEdge::West
            } else if a.x() == u[1] {
                SurfaceExtensionEdge::East
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        } else if a.y() == b.y() && [a.x().min(b.x()), a.x().max(b.x())] == u {
            if a.y() == v[0] {
                SurfaceExtensionEdge::South
            } else if a.y() == v[1] {
                SurfaceExtensionEdge::North
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        } else {
            return Err(CommandError::Usage("Pick a complete rectangular boundary"));
        };
        Ok(Self {
            source: source.clone(),
            index,
            point,
            side,
            reversed: face.is_reversed(),
            tolerance: doc.tolerance(),
            units: doc.units().clone(),
        })
    }
    pub fn object(&self) -> ObjectId {
        self.source.id()
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn current(&self, doc: &Document) -> bool {
        doc.tolerance() == self.tolerance
            && doc.units() == &self.units
            && doc.is_object_selectable(self.object())
            && doc.object(self.object()).is_some_and(|o| {
                self.source
                    .geometry_snapshot()
                    .shares_storage_with(o.geometry_snapshot())
                    && self.source.attributes() == o.attributes()
                    && self.source.group_ids() == o.group_ids()
                    && self.source.geometry_user_text() == o.geometry_user_text()
            })
    }
    fn surface(&self) -> &NurbsSurface {
        match self.source.geometry() {
            Geometry::Brep(b) => b.faces()[0].surface(),
            Geometry::NurbsSurface(s) => s,
            _ => unreachable!(),
        }
    }
    fn picked_high(&self) -> Result<bool, CommandError> {
        let s = self.surface();
        let u = [*s.domain_u().start(), *s.domain_u().end()];
        let v = [*s.domain_v().start(), *s.domain_v().end()];
        let ends = match self.side {
            SurfaceExtensionEdge::West => [[u[0], v[0]], [u[0], v[1]]],
            SurfaceExtensionEdge::East => [[u[1], v[0]], [u[1], v[1]]],
            SurfaceExtensionEdge::South => [[u[0], v[0]], [u[1], v[0]]],
            SurfaceExtensionEdge::North => [[u[0], v[1]], [u[1], v[1]]],
        };
        Ok(self
            .point
            .distance_to(s.evaluate(ends[1][0], ends[1][1])?)?
            < self
                .point
                .distance_to(s.evaluate(ends[0][0], ends[0][1])?)?)
    }
}

#[derive(Clone, Debug)]
pub struct Prepared {
    edges: [Edge; 2],
    result: Brep,
}
pub fn prepare(
    doc: &Document,
    edges: [Edge; 2],
    options: Options,
) -> Result<Prepared, CommandError> {
    if edges[0].object() == edges[1].object() || edges.iter().any(|e| !e.current(doc)) {
        return Err(CommandError::Usage(
            "Pick current edges on two different surfaces",
        ));
    }
    let mut policy = options.policy;
    policy.reverse_match = edges[0].picked_high()?
        ^ edges[1].picked_high()?
        ^ matches!(
            edges[1].side,
            SurfaceExtensionEdge::West | SurfaceExtensionEdge::North
        );
    #[cfg(feature = "native-smlib")]
    let operation = viboceros_smlib::match_surface_supports;
    #[cfg(not(feature = "native-smlib"))]
    let operation = viboceros_geometry::try_match_nurbs_surfaces;
    let surface = operation(
        edges[0].surface(),
        edges[0].side,
        edges[1].surface(),
        edges[1].side,
        policy,
        doc.tolerance(),
    )?;
    let result = Brep::try_rectangular_surface_face_with_surface_edge_domains(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        edges[0].reversed,
        doc.tolerance(),
    )?;
    Ok(Prepared { edges, result })
}
impl Prepared {
    pub fn geometry(&self) -> &Brep {
        &self.result
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.edges.iter().all(|e| e.current(doc))
    }
    pub fn commit(&self, doc: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(doc, "MatchSrf", |d| self.apply(d))
    }
    fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("MatchSrf prepared inputs changed"));
        }
        doc.clear_selection();
        let id = self.edges[0].object();
        doc.replace_object_geometries_with_history(
            [(id, Geometry::Brep(self.result.clone()))],
            ReplacementHistory::EveryReplacement,
        )?;
        doc.move_objects_to_end_in_order([id])?;
        Ok("Matched surface edge".into())
    }
}

pub(super) struct MatchSurfaceCommand(pub std::sync::Arc<remembered::Remembered<Options>>);
impl Command for MatchSurfaceCommand {
    fn name(&self) -> &'static str {
        "MatchSrf"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        if args.len() < 6 {
            return Err(CommandError::Usage(USAGE));
        }
        let options = parse(&args[6..], self.0.get())?;
        let mut edges = Vec::new();
        for i in [0, 3] {
            let id = args[i].parse().map_err(|_| CommandError::Usage(USAGE))?;
            let index = args[i + 1]
                .parse()
                .map_err(|_| CommandError::Usage(USAGE))?;
            let coords = args[i + 2]
                .split(',')
                .map(str::parse::<Real>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CommandError::Usage(USAGE))?;
            let [x, y, z]: [Real; 3] = coords.try_into().map_err(|_| CommandError::Usage(USAGE))?;
            edges.push(Edge::capture(doc, id, index, Point3::try_new(x, y, z)?)?);
        }
        self.0.set(options);
        prepare(doc, edges.try_into().unwrap(), options)?.apply(doc)
    }
}
