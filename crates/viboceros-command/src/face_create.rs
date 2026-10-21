//! Direct mesh-face drafts are shared by command scripts and point getters.
use super::*;
use viboceros_geometry::{MeshFaceBoundaryGraph, UnitVector3};

pub const USAGE: &str = "3DFace [Output=Mesh|SubD] [Interpolated=Yes|No] [PolygonType=Triangle|Quad|Ngon] [InPlane=Yes|No] [WeldAngle=degrees] [Mode=SingleFace|MultipleFaces] [Append=id] [FromEdge=Yes|No Chain=Yes|No Edge=n] points...";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polygon {
    Triangle,
    Quad,
    Ngon,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Mesh,
    SubD,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub output: Output,
    pub interpolated: bool,
    pub polygon: Polygon,
    pub in_plane: bool,
    pub weld_angle: Real,
    pub multiple: bool,
    pub from_edge: bool,
    pub chain: bool,
    pub from_edge_reset: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            output: Output::Mesh,
            interpolated: false,
            polygon: Polygon::Quad,
            in_plane: false,
            weld_angle: 180.,
            multiple: false,
            from_edge: false,
            chain: false,
            from_edge_reset: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Point(Point3),
    Edge(usize),
    Undo,
    Enter,
    Cancel,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub preferences: Preferences,
    pub output_explicit: bool,
    pub append: Option<ObjectId>,
    pub edge: Option<usize>,
    pub tokens: Vec<Token>,
}

pub fn parse(args: &[&str], preferences: Preferences) -> Result<Input, CommandError> {
    let mut input = Input {
        preferences,
        output_explicit: false,
        append: None,
        edge: None,
        tokens: vec![],
    };
    let mut seen = BTreeSet::new();
    for word in args {
        let word = word.trim_start_matches('_');
        if let Some((key, value)) = word.split_once('=') {
            if key.eq_ignore_ascii_case("Edge")
                && (!input.tokens.is_empty() || input.edge.is_some())
            {
                if input.tokens.len() >= viboceros_geometry::MAX_MESH_PATCH_POINTS {
                    return Err(GeometryError::MeshPatchLimit.into());
                }
                input.tokens.push(Token::Edge(
                    value.parse().map_err(|_| CommandError::Usage(USAGE))?,
                ));
                continue;
            }
            if !input.tokens.is_empty() || !seen.insert(key.to_ascii_lowercase()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.to_ascii_lowercase().as_str() {
                "output" => {
                    input.output_explicit = true;
                    input.preferences.output = match value.to_ascii_lowercase().as_str() {
                        "mesh" => Output::Mesh,
                        "subd" => Output::SubD,
                        _ => return Err(CommandError::Usage(USAGE)),
                    }
                }
                "interpolated" => {
                    input.preferences.interpolated =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "polygontype" => {
                    input.preferences.polygon = match value.to_ascii_lowercase().as_str() {
                        "triangle" => Polygon::Triangle,
                        "quad" => Polygon::Quad,
                        "ngon" => Polygon::Ngon,
                        _ => return Err(CommandError::Usage(USAGE)),
                    }
                }
                "inplane" => {
                    input.preferences.in_plane =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "weldangle" => {
                    let angle = parse_finite_real(value)?;
                    if !(0. ..=180.).contains(&angle) {
                        return Err(CommandError::Usage(USAGE));
                    }
                    input.preferences.weld_angle = angle;
                }
                "mode" => {
                    input.preferences.multiple = match value.to_ascii_lowercase().as_str() {
                        "singleface" => false,
                        "multiplefaces" => true,
                        _ => return Err(CommandError::Usage(USAGE)),
                    }
                }
                "fromedge" => {
                    input.preferences.from_edge =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "fromedgereset" => {
                    input.preferences.from_edge_reset =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                "append" => {
                    input.append = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?)
                }
                "edge" => input.edge = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?),
                "chain" => {
                    input.preferences.chain =
                        parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
                }
                _ => return Err(CommandError::Usage(USAGE)),
            }
        } else {
            let token = match word.to_ascii_lowercase().as_str() {
                "undo" => Token::Undo,
                "enter" => Token::Enter,
                "cancel" => Token::Cancel,
                _ => {
                    let word = word.trim_start_matches(['w', 'W']);
                    Token::Point(parse_point(&[word])?.0)
                }
            };
            if input.tokens.len() >= viboceros_geometry::MAX_MESH_PATCH_POINTS {
                return Err(GeometryError::MeshPatchLimit.into());
            }
            input.tokens.push(token);
        }
    }
    if input.edge.is_some() && !input.preferences.from_edge {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(input)
}

#[derive(Clone, Debug)]
pub struct CompletedFace {
    pub points: Vec<Point3>,
    pub in_plane: bool,
    pub weld_angle: Real,
    resume: Resume,
    flipped: bool,
    part: TriangleMesh,
    input_last: Point3,
    start_swapped: bool,
}
#[derive(Clone, Debug)]
struct Chain {
    reverse: Option<bool>,
    reference: UnitVector3,
}

#[derive(Clone, Debug)]
struct Resume {
    seed: usize,
    chain: Option<Chain>,
    from_edge: bool,
    edge_same_order: bool,
    boundary_checkpoint: usize,
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub preferences: Preferences,
    pub points: Vec<Point3>,
    faces: Vec<CompletedFace>,
    source: Option<TriangleMesh>,
    source_subd: Option<viboceros_geometry::SubD>,
    boundaries: MeshFaceBoundaryGraph,
    edge_same_order: bool,
    seed: usize,
    chain: Option<Chain>,
    waiting_edge: bool,
    total: usize,
    pub finished: bool,
}
impl Draft {
    pub fn new(
        preferences: Preferences,
        source: Option<TriangleMesh>,
        edge: Option<usize>,
    ) -> Result<Self, CommandError> {
        Self::with_source(preferences, source.map(Geometry::Mesh), edge)
    }
    pub fn with_source(
        preferences: Preferences,
        source: Option<Geometry>,
        edge: Option<usize>,
    ) -> Result<Self, CommandError> {
        let (source, source_subd) = match source {
            Some(Geometry::SubD(subd)) => (Some(subd.control_mesh()?), Some(subd)),
            Some(Geometry::Mesh(mesh)) => (Some(mesh), None),
            None => (None, None),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        if !preferences.weld_angle.is_finite() || !(0. ..=180.).contains(&preferences.weld_angle) {
            return Err(CommandError::Usage(USAGE));
        }
        let boundaries = source
            .as_ref()
            .map(MeshFaceBoundaryGraph::from_mesh)
            .transpose()?
            .unwrap_or_default();
        let mut draft = Self {
            preferences,
            points: vec![],
            faces: vec![],
            source,
            source_subd,
            boundaries,
            edge_same_order: true,
            seed: 0,
            chain: None,
            waiting_edge: preferences.from_edge,
            total: 0,
            finished: false,
        };
        if let Some(edge) = edge {
            draft.pick_edge(edge)?;
        }
        Ok(draft)
    }
    pub fn waiting_edge(&self) -> bool {
        self.waiting_edge
    }
    pub fn current_mesh(&self) -> Result<Option<TriangleMesh>, CommandError> {
        if self.faces.is_empty() {
            Ok(self.source.clone())
        } else {
            self.mesh()
        }
    }
    pub fn pick_edge(&mut self, index: usize) -> Result<(), CommandError> {
        if !self.preferences.from_edge || !self.waiting_edge || self.finished {
            return Err(CommandError::Usage(USAGE));
        }
        let mesh = self.current_mesh()?.ok_or(CommandError::Usage(USAGE))?;
        let seed = mesh.boundary_face_seed(index)?;
        let physical = mesh
            .diagnostic_edges()
            .into_iter()
            .find(|e| e.index == index)
            .unwrap()
            .points;
        self.edge_same_order = seed[0] == physical[0];
        let chain = if self.preferences.chain && self.preferences.polygon == Polygon::Quad {
            mesh.boundary_face_loop(index)?;
            let reference = self
                .boundaries
                .boundary_normal(seed)
                .ok_or(CommandError::Usage(USAGE))?;
            Some(Chain {
                reverse: None,
                reference,
            })
        } else {
            None
        };
        if self.total + 2 > viboceros_geometry::MAX_MESH_PATCH_POINTS {
            return Err(GeometryError::MeshPatchLimit.into());
        }
        self.points = seed.to_vec();
        self.seed = 2;
        self.chain = chain;
        self.waiting_edge = false;
        self.total += 2;
        Ok(())
    }
    pub fn update_preferences(&mut self, preferences: Preferences) -> Result<(), CommandError> {
        if preferences.output != self.preferences.output
            && (!self.faces.is_empty() || !self.points.is_empty())
        {
            return Err(CommandError::Usage(
                "Clear the face draft before changing Output",
            ));
        }

        if !preferences.weld_angle.is_finite() || !(0. ..=180.).contains(&preferences.weld_angle) {
            return Err(CommandError::Usage(USAGE));
        }
        if preferences.from_edge != self.preferences.from_edge
            || preferences.chain != self.preferences.chain
            || preferences.polygon != self.preferences.polygon
        {
            if !self.points.is_empty() {
                return Err(CommandError::Usage(
                    "Clear the unfinished face before changing its edge or polygon mode",
                ));
            }
            self.chain = None;
            self.seed = 0;
            self.waiting_edge = preferences.from_edge;
        }
        self.preferences = preferences;
        Ok(())
    }
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
    pub fn completed(&self) -> &[CompletedFace] {
        &self.faces
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.points
            .last()
            .copied()
            .or_else(|| self.faces.last().map(|face| face.input_last))
    }
    pub fn clear_points(&mut self) {
        self.total -= self.points.len();
        self.points.clear();
        self.seed = 0;
        self.chain = None;
        self.waiting_edge = self.preferences.from_edge;
    }
    /// Use the same corner constraint for previews and accepted picks.
    pub fn constrained_corner(&self, point: Point3) -> Result<Point3, CommandError> {
        if self.preferences.in_plane && self.points.len() >= 3 {
            let frame = Frame3::try_from_directions(
                self.points[0],
                self.points[0].vector_to(self.points[1])?,
                self.points[0].vector_to(self.points[2])?,
                Tolerance::NUMERICAL_VALIDATION,
            )?;
            let [u, v] = frame.projected_coordinates_of(point)?;
            return Ok(frame.point_at([u, v, 0.])?);
        }
        Ok(point)
    }
    pub fn accept(&mut self, token: Token) -> Result<(), CommandError> {
        if self.finished {
            return if matches!(token, Token::Enter | Token::Cancel) {
                Ok(())
            } else {
                Err(CommandError::Usage(USAGE))
            };
        }
        match token {
            Token::Edge(index) => return self.pick_edge(index),
            Token::Point(p) => {
                if self.waiting_edge {
                    return Err(CommandError::Usage("3DFace: select a boundary edge"));
                }
                if self.total >= viboceros_geometry::MAX_MESH_PATCH_POINTS {
                    return Err(GeometryError::MeshPatchLimit.into());
                }
                if self.chain.is_some()
                    && self.preferences.multiple
                    && self.points.len() == 3
                    && self.total + 4 > viboceros_geometry::MAX_MESH_PATCH_POINTS
                {
                    return Err(GeometryError::MeshPatchLimit.into());
                }
                let p = self.constrained_corner(p)?;
                if self.points.last() == Some(&p) {
                    return Err(CommandError::Usage("3DFace corners must be distinct"));
                }
                if self.preferences.polygon == Polygon::Triangle
                    || self.preferences.polygon == Polygon::Quad && self.points.len() == 3
                    || self.preferences.in_plane && self.points.len() == 2
                {
                    let mut candidate = self.points.clone();
                    candidate.push(p);
                    if candidate.len() >= 3 {
                        let (ordered, _, _) = self.ordered_face(&candidate)?;
                        TriangleMesh::try_face_from_points(&ordered, self.preferences.in_plane)?;
                    }
                }
                self.points.push(p);
                self.total += 1;
                let count = match self.preferences.polygon {
                    Polygon::Triangle => 3,
                    Polygon::Quad => 4,
                    Polygon::Ngon => usize::MAX,
                };
                if self.points.len() == count {
                    self.complete()?;
                }
            }
            Token::Undo => {
                if self.preferences.from_edge && !self.points.is_empty() {
                    if self.points.len() == 2 && self.seed == 2 && !self.edge_same_order {
                        self.points.remove(0);
                    } else {
                        self.points.pop();
                    }
                    self.total -= 1;
                    self.seed = self.seed.min(self.points.len());
                    if self.seed < 2 {
                        self.chain = None;
                    }
                    if self.points.is_empty() {
                        self.waiting_edge = true;
                    }
                } else if self.points.len() > self.seed {
                    self.points.pop();
                    self.total -= 1;
                } else if let Some(mut previous) = self.faces.pop() {
                    if previous.flipped {
                        previous.points.rotate_right(1);
                        previous.points.swap(2, 3);
                    }
                    if previous.start_swapped {
                        previous.points.swap(0, 1);
                    }
                    self.total -= self.points.len() + 1;
                    previous.points.pop();
                    self.points = previous.points;
                    self.boundaries
                        .rollback(previous.resume.boundary_checkpoint)?;
                    self.edge_same_order = previous.resume.edge_same_order;
                    self.seed = previous.resume.seed;
                    self.chain = previous.resume.chain;
                    self.preferences.from_edge = previous.resume.from_edge;
                    self.waiting_edge = false;
                }
            }
            Token::Enter => {
                if self.chain.is_some() && self.points.len() <= 3 {
                    self.clear_points();
                } else if self.points.len() >= 3 {
                    self.complete()?;
                } else {
                    self.points.clear();
                    self.finished = true;
                }
            }
            Token::Cancel => {
                self.faces.clear();
                self.points.clear();
                self.finished = true;
            }
        }
        Ok(())
    }
    pub fn preview_corners(&self, cursor: Option<Point3>) -> Result<Vec<Point3>, CommandError> {
        let mut points = self.points.clone();
        if let Some(point) = cursor {
            let point = self.constrained_corner(point)?;
            if points.last() != Some(&point) {
                points.push(point);
            }
        }
        self.ordered_face(&points).map(|(points, _, _)| points)
    }
    fn ordered_face(&self, input: &[Point3]) -> Result<(Vec<Point3>, bool, bool), CommandError> {
        let mut points = input.to_vec();
        let flipped = if self.preferences.from_edge && self.seed >= 2 && points.len() == 4 {
            if let Some(chain) = self.chain.as_ref().filter(|c| c.reverse.is_some()) {
                let positive = TriangleMesh::face_corner_side(
                    [points[0], points[1], points[2]],
                    chain.reference,
                )?
                .is_gt();
                let reverse = if positive {
                    chain.reverse.unwrap()
                } else {
                    !chain.reverse.unwrap()
                };
                reverse
                    || TriangleMesh::face_quad_crosses([
                        points[0], points[1], points[2], points[3],
                    ])?
            } else {
                let mut physical = [points[0], points[1], points[2], points[3]];
                if !self.edge_same_order {
                    physical.swap(0, 1);
                }
                // Both orders can be simple when the last corner lies inside
                // the first triangle. Keep coherent boundary winding in that
                // concave case. Collinear starts retain the native physical
                // edge ordering, including overlapping and reversed sources.
                if points[0]
                    .vector_to(points[1])?
                    .unit_cross_point_difference(points[0], points[2])?
                    .is_some()
                {
                    TriangleMesh::face_quad_crosses([points[0], points[1], points[2], points[3]])?
                } else {
                    TriangleMesh::face_quad_crosses(physical)? ^ !self.edge_same_order
                }
            }
        } else {
            false
        };
        if flipped {
            points.swap(2, 3);
            points.rotate_left(1);
        }
        let start_swapped = points.len() == 4
            && (!self.preferences.from_edge || self.seed < 2)
            && TriangleMesh::face_quad_reverses_start([
                points[0], points[1], points[2], points[3],
            ])?;
        if start_swapped {
            points.swap(0, 1);
        }
        Ok((points, flipped, start_swapped))
    }
    fn complete(&mut self) -> Result<(), CommandError> {
        let (points, flipped, start_swapped) = self.ordered_face(&self.points)?;
        let part = TriangleMesh::try_face_from_points(&points, self.preferences.in_plane)?;
        let resume = Resume {
            seed: self.seed,
            chain: self.chain.clone(),
            from_edge: self.preferences.from_edge,
            edge_same_order: self.edge_same_order,
            boundary_checkpoint: self.boundaries.checkpoint(),
        };
        self.boundaries.append(&part)?;
        let corner = self.points[usize::from(flipped)];
        let carry = *self.points.last().unwrap();
        self.faces.push(CompletedFace {
            points,
            in_plane: self.preferences.in_plane,
            weld_angle: self.preferences.weld_angle,
            resume,
            flipped,
            part,
            input_last: carry,
            start_swapped,
        });
        self.points.clear();
        self.finished = !self.preferences.multiple;
        if self.finished {
            return Ok(());
        }
        self.waiting_edge = self.preferences.from_edge;
        if let Some(chain) = self.chain.as_mut() {
            chain.reverse.get_or_insert(flipped);
            if let Some((seed, reference)) = self.boundaries.next_seed(corner, !flipped) {
                if self.total + 3 > viboceros_geometry::MAX_MESH_PATCH_POINTS {
                    return Err(GeometryError::MeshPatchLimit.into());
                }
                self.points = vec![seed[0], seed[1], carry];
                self.seed = 2;
                self.edge_same_order = true;
                self.total += 3;
                chain.reference = if chain.reverse != Some(flipped) {
                    let normal = self.faces.last().unwrap().part.polygon_face_normal(0)?;
                    normal.as_vector().scaled(-1.)?.normalized_nonzero()?
                } else {
                    reference
                };
                self.waiting_edge = false;
            } else {
                self.chain = None;
                self.seed = 0;
            }
        }
        Ok(())
    }
    /// Source replacement and new objects are staged together. Exact-edge
    /// SubD components that do not touch the source become separate objects.
    pub fn output(&self, tolerance: Tolerance) -> Result<Option<FaceOutput>, CommandError> {
        if self.faces.is_empty() {
            return Ok(None);
        }
        if self.preferences.output == Output::Mesh {
            let geometry = Geometry::Mesh(self.mesh()?.unwrap());
            return Ok(Some(
                if self.source.is_some() && self.source_subd.is_none() {
                    FaceOutput {
                        replacement: Some(geometry),
                        additions: vec![],
                    }
                } else {
                    FaceOutput {
                        replacement: None,
                        additions: vec![geometry],
                    }
                },
            ));
        }
        let faces = self
            .faces
            .iter()
            .map(|f| f.points.clone())
            .collect::<Vec<_>>();
        let output = if let Some(source) = &self.source_subd {
            let result = source.try_append_face_components(&faces)?;
            FaceOutput {
                replacement: if result.attached_faces > 0 || self.preferences.interpolated {
                    Some(Geometry::SubD(self.fitted_subd(
                        result.source,
                        source.control_points().len(),
                        tolerance,
                    )?))
                } else {
                    None
                },
                additions: result
                    .detached
                    .into_iter()
                    .map(|s| self.fitted_subd(s, 0, tolerance).map(Geometry::SubD))
                    .collect::<Result<Vec<_>, _>>()?,
            }
        } else {
            FaceOutput {
                replacement: None,
                additions: viboceros_geometry::SubD::try_from_face_components(&faces)?
                    .into_iter()
                    .map(|s| self.fitted_subd(s, 0, tolerance).map(Geometry::SubD))
                    .collect::<Result<Vec<_>, _>>()?,
            }
        };
        Ok(Some(output))
    }
    fn fitted_subd(
        &self,
        subd: viboceros_geometry::SubD,
        original_vertices: usize,
        tolerance: Tolerance,
    ) -> Result<viboceros_geometry::SubD, CommandError> {
        if !self.preferences.interpolated {
            return Ok(subd);
        }
        let mut targets = subd.control_points().to_vec();
        if original_vertices > 0 {
            targets[..original_vertices]
                .copy_from_slice(self.source_subd.as_ref().unwrap().surface_points());
        }
        Ok(subd.try_with_limit_points(&targets, tolerance)?)
    }
    pub fn mesh(&self) -> Result<Option<TriangleMesh>, CommandError> {
        if self.faces.is_empty() {
            return Ok(None);
        }
        let parts = self.faces.iter().map(|face| &face.part).collect::<Vec<_>>();
        if self.preferences.output == Output::SubD {
            return Ok(Some(TriangleMesh::try_append(
                &self
                    .source
                    .iter()
                    .chain(parts.iter().copied())
                    .collect::<Vec<_>>(),
            )?));
        }
        if self
            .faces
            .iter()
            .any(|f| f.weld_angle != self.faces[0].weld_angle)
        {
            let mut mesh = self.source.clone().filter(|_| self.source_subd.is_none());
            for (face, part) in self.faces.iter().zip(parts) {
                let count = mesh.as_ref().map_or(0, |m| m.faces().len());
                let added = if let Some(original) = mesh {
                    TriangleMesh::try_append(&[&original, part])?
                } else {
                    part.clone()
                };
                mesh = Some(
                    added
                        .welded_vertices_after(face.weld_angle.to_radians(), count)?
                        .0,
                );
            }
            return Ok(mesh);
        }
        let included_source = self.source.iter().filter(|_| self.source_subd.is_none());
        let refs = included_source
            .chain(parts.iter().copied())
            .collect::<Vec<_>>();
        let mesh = TriangleMesh::try_append(&refs)?;
        let first = if self.source_subd.is_some() {
            0
        } else {
            self.source.as_ref().map_or(0, |m| m.faces().len())
        };
        let angle = self.faces[0].weld_angle;
        let mesh = mesh.welded_vertices_after(angle.to_radians(), first)?.0;
        Ok(Some(mesh))
    }
}

pub fn target(
    document: &Document,
    explicit: Option<ObjectId>,
) -> Result<Option<ObjectId>, CommandError> {
    if let Some(id) = explicit {
        document
            .object(id)
            .filter(|o| {
                document.is_object_selectable(id)
                    && matches!(o.geometry(), Geometry::Mesh(_) | Geometry::SubD(_))
            })
            .ok_or(CommandError::Usage(USAGE))?;
        return Ok(Some(id));
    }
    let mut selected = document
        .selected_objects()
        .filter(|o| matches!(o.geometry(), Geometry::Mesh(_) | Geometry::SubD(_)))
        .map(|o| o.id());
    let first = selected.next();
    if selected.next().is_some() {
        return Err(CommandError::Usage("3DFace requires one append target"));
    }
    Ok(first)
}
pub fn target_preferences(
    document: &Document,
    target: Option<ObjectId>,
    preferences: Preferences,
    explicit_output: bool,
) -> Preferences {
    if explicit_output {
        return preferences;
    }
    let output = target.and_then(|id| document.object(id)).map(|o| {
        if matches!(o.geometry(), Geometry::SubD(_)) {
            Output::SubD
        } else {
            Output::Mesh
        }
    });
    Preferences {
        output: output.unwrap_or(preferences.output),
        ..preferences
    }
}
pub fn source(document: &Document, target: Option<ObjectId>) -> Option<Geometry> {
    target
        .and_then(|id| document.object(id))
        .and_then(|o| match o.geometry() {
            Geometry::Mesh(_) | Geometry::SubD(_) => Some(o.geometry().clone()),
            _ => None,
        })
}
#[derive(Clone, Debug)]
pub struct FaceOutput {
    pub replacement: Option<Geometry>,
    pub additions: Vec<Geometry>,
}
pub fn apply_output(
    document: &mut Document,
    target: Option<ObjectId>,
    output: FaceOutput,
) -> Result<Vec<ObjectId>, CommandError> {
    if output.replacement.is_some() && target.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    let mut ids = Vec::new();
    if let Some(id) = target {
        document.select_objects_direct([id], SelectionMode::Remove)?;
        if let Some(geometry) = output.replacement {
            document.replace_object_geometries(vec![(id, geometry)])?;
        }
        ids.push(id);
    }
    for geometry in output.additions {
        ids.push(document.add_geometry(geometry)?);
    }
    Ok(ids)
}
pub fn apply(
    document: &mut Document,
    target: Option<ObjectId>,
    mesh: TriangleMesh,
) -> Result<ObjectId, CommandError> {
    apply_geometry(document, target, Geometry::Mesh(mesh))
}
pub fn apply_geometry(
    document: &mut Document,
    target: Option<ObjectId>,
    geometry: Geometry,
) -> Result<ObjectId, CommandError> {
    if let Some(id) = target {
        document.select_objects_direct([id], SelectionMode::Remove)?;
        document.replace_object_geometries(vec![(id, geometry)])?;
        Ok(id)
    } else {
        document.add_geometry(geometry).map_err(Into::into)
    }
}
pub(super) struct FaceCreateCommand(pub std::sync::Arc<remembered::Remembered<Preferences>>);
impl Command for FaceCreateCommand {
    fn name(&self) -> &'static str {
        "3DFace"
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut preferences = self.0.get();
        if preferences.from_edge_reset {
            preferences.from_edge = false;
        }
        let input = parse(args, preferences)?;
        let target = target(document, input.append)?;
        let preferences =
            target_preferences(document, target, input.preferences, input.output_explicit);
        let mut draft = Draft::with_source(preferences, source(document, target), input.edge)?;
        for token in input.tokens {
            draft.accept(token)?;
        }
        if !draft.finished {
            draft.accept(Token::Enter)?;
        }
        let Some(output) = draft.output(document.tolerance())? else {
            return Err(CommandError::Usage(USAGE));
        };
        let ids = apply_output(document, target, output)?;
        self.0.set(draft.preferences);
        Ok(format!(
            "Created {} face(s) on object(s) {}",
            draft.face_count(),
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}
impl CommandRegistry {
    pub fn face_create_input(&self, args: &[&str]) -> Result<Input, CommandError> {
        let mut preferences = self.face_create_preferences.get();
        if preferences.from_edge_reset {
            preferences.from_edge = false;
        }
        parse(args, preferences)
    }
    pub fn accept_face_create_preferences(&self, preferences: Preferences) {
        self.face_create_preferences.set(preferences);
    }
}
