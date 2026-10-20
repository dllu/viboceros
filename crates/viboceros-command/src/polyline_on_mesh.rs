//! Immutable mesh polyline plans shared by scripted and interactive commands.
use super::*;
use std::sync::Arc;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{MAX_CURVE_INTERPOLATION_POINTS, MeshProjectionIndex};
pub const USAGE: &str = "PolylineOnMesh points... [Mesh=id] [Close]";
pub const MAX_POINTS: usize = MAX_CURVE_INTERPOLATION_POINTS;
#[derive(Default)]
pub struct Input<'a> {
    pub points: Vec<&'a str>,
    pub mesh: Option<ObjectId>,
    pub close: bool,
}
pub fn input<'a>(args: &[&'a str]) -> Result<Input<'a>, CommandError> {
    let mut result = Input::default();
    let mut seen = BTreeSet::new();
    for arg in args {
        let word = arg.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Close") {
            if !seen.insert("close".into()) {
                return Err(CommandError::Usage(USAGE));
            }
            result.close = true;
            continue;
        }
        if let Some((key, value)) = word.split_once('=') {
            let key = key.to_ascii_lowercase();
            if !seen.insert(key.clone()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.as_str() {
                "mesh" => {
                    result.mesh = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "close" => result.close = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            result.points.push(*arg);
        }
    }
    Ok(result)
}
pub fn selected_target(doc: &Document) -> Option<ObjectId> {
    let mut ids = doc
        .selected_objects()
        .filter(|o| matches!(o.geometry(), Geometry::Mesh(_)))
        .map(|o| o.id());
    let first = ids.next()?;
    ids.next().is_none().then_some(first)
}
#[derive(Clone, Debug)]
pub struct Target {
    pub id: ObjectId,
    pub index: Arc<MeshProjectionIndex<'static>>,
    parent: GeometrySnapshot,
    tolerance: Tolerance,
}
impl Target {
    pub fn capture(doc: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let object = doc
            .object(id)
            .filter(|_| doc.is_object_selectable(id))
            .ok_or(DocumentError::ObjectNotFound(id))?;
        let Geometry::Mesh(mesh) = object.geometry() else {
            return Err(CommandError::Usage("Select a polygon mesh"));
        };
        let index = MeshProjectionIndex::new_owned(mesh.clone())?;
        if index.triangle_points(0).is_none() {
            return Err(CommandError::Usage("Select a nonempty polygon mesh"));
        }
        Ok(Self {
            id,
            index: Arc::new(index),
            parent: object.geometry_snapshot().clone(),
            tolerance: doc.tolerance(),
        })
    }
    pub fn project(&self, point: Point3, direction: Vector3) -> Result<Point3, CommandError> {
        self.index
            .project_point(point, direction, self.tolerance)?
            .into_iter()
            .next()
            .ok_or(CommandError::Usage("Point does not project onto the mesh"))
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.id)
            && doc
                .object(self.id)
                .is_some_and(|o| self.parent.shares_storage_with(o.geometry_snapshot()))
    }
    pub fn location(&self, point: Point3) -> Result<Point3, CommandError> {
        self.index
            .closest_point(point)?
            .ok_or(CommandError::Usage("Select a nonempty polygon mesh"))
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub target: Target,
    pub polyline: Polyline3,
    pub preview: NurbsCurve,
}
impl Plan {
    pub fn prepare(
        doc: &Document,
        target: Target,
        points: &[Point3],
        closed: bool,
    ) -> Result<Self, CommandError> {
        if !target.current(doc) {
            return Err(CommandError::Usage("Select the mesh again"));
        }
        if !(2..=MAX_POINTS).contains(&points.len()) {
            return Err(CommandError::Usage(USAGE));
        }
        let mut locations = points
            .iter()
            .map(|p| target.location(*p))
            .collect::<Result<Vec<_>, _>>()?;
        if closed && locations.first() != locations.last() {
            if locations.len() < 3 {
                return Err(CommandError::Usage("Close requires three mesh locations"));
            }
            locations.push(locations[0]);
        }
        let mut output = Vec::new();
        for pair in locations.windows(2) {
            let segment =
                target
                    .index
                    .polyline_on_mesh_segment(pair[0], pair[1], doc.tolerance())?;
            output.extend_from_slice(&segment.vertices()[if output.is_empty() { 0 } else { 1 }..]);
        }
        if output.len() > viboceros_geometry::MAX_CURVE_DIVISION_POINTS {
            return Err(viboceros_geometry::GeometryError::MeshPullLimit.into());
        }
        let polyline = Polyline3::try_new(output, Tolerance::NUMERICAL_VALIDATION)?;
        let preview = polyline.to_nurbs()?;
        Ok(Self {
            target,
            polyline,
            preview,
        })
    }
    pub fn extended(&self, doc: &Document, next: Point3) -> Result<Self, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("Select the mesh again"));
        }
        let last = *self.polyline.vertices().last().unwrap();
        let segment = self
            .target
            .index
            .polyline_on_mesh_segment(last, next, doc.tolerance())?;
        let mut points = self.polyline.vertices().to_vec();
        points.extend_from_slice(&segment.vertices()[1..]);
        if points.len() > viboceros_geometry::MAX_CURVE_DIVISION_POINTS {
            return Err(viboceros_geometry::GeometryError::MeshPullLimit.into());
        }
        let polyline = Polyline3::try_new(points, Tolerance::NUMERICAL_VALIDATION)?;
        let preview = polyline.to_nurbs()?;
        Ok(Self {
            target: self.target.clone(),
            polyline,
            preview,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.target.current(doc)
    }
    pub fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("Select the mesh again"));
        }
        doc.add_geometry(Geometry::Polyline(self.polyline.clone()))?;
        Ok("Created polyline on mesh".into())
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
            Err(e) => {
                doc.rollback_transaction()?;
                Err(e)
            }
        }
    }
}
pub struct PolylineOnMeshCommand;
impl Command for PolylineOnMeshCommand {
    fn name(&self) -> &'static str {
        "PolylineOnMesh"
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
        let input = input(args)?;
        let mut points = Vec::new();
        let mut consumed = 0;
        while consumed < input.points.len() {
            let (p, n) = parse_point(&input.points[consumed..])?;
            points.push(p);
            consumed += n;
            if points.len() > MAX_POINTS {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let id = input
            .mesh
            .or_else(|| selected_target(doc))
            .ok_or(CommandError::Usage(USAGE))?;
        let target = Target::capture(doc, id)?;
        let direction = context
            .construction_plane
            .z_axis()
            .as_vector()
            .scaled(-1.)?;
        let points = points
            .into_iter()
            .map(|p| target.project(p, direction))
            .collect::<Result<Vec<_>, _>>()?;
        Plan::prepare(doc, target, &points, input.close)?.apply(doc)
    }
}
