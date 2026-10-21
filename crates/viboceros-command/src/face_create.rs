//! Direct mesh-face drafts are shared by command scripts and point getters.
use super::*;

pub const USAGE: &str = "3DFace [Output=Mesh] [PolygonType=Triangle|Quad|Ngon] [InPlane=Yes|No] [WeldAngle=degrees] [Mode=SingleFace|MultipleFaces] [Append=id] [FromEdge=Yes|No Edge=n] points...";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polygon {
    Triangle,
    Quad,
    Ngon,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub polygon: Polygon,
    pub in_plane: bool,
    pub weld_angle: Real,
    pub multiple: bool,
    pub from_edge: bool,
    pub from_edge_reset: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            polygon: Polygon::Quad,
            in_plane: false,
            weld_angle: 180.,
            multiple: false,
            from_edge: false,
            from_edge_reset: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Point(Point3),
    Undo,
    Enter,
    Cancel,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub preferences: Preferences,
    pub append: Option<ObjectId>,
    pub edge: Option<usize>,
    pub tokens: Vec<Token>,
}

pub fn parse(args: &[&str], preferences: Preferences) -> Result<Input, CommandError> {
    let mut input = Input {
        preferences,
        append: None,
        edge: None,
        tokens: vec![],
    };
    let mut seen = BTreeSet::new();
    for word in args {
        let word = word.trim_start_matches('_');
        if let Some((key, value)) = word.split_once('=') {
            if !input.tokens.is_empty() || !seen.insert(key.to_ascii_lowercase()) {
                return Err(CommandError::Usage(USAGE));
            }
            match key.to_ascii_lowercase().as_str() {
                "output" if value.eq_ignore_ascii_case("Mesh") => (),
                "output" => {
                    return Err(CommandError::Usage(
                        "3DFace currently supports Output=Mesh; SubD geometry is not implemented",
                    ));
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
                "chain" if parse_yes_no(value) == Some(false) => (),
                "chain" => {
                    return Err(CommandError::Usage(
                        "3DFace edge-chain drafting is not implemented",
                    ));
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
    if input.preferences.from_edge && input.preferences.multiple {
        return Err(CommandError::Usage(
            "3DFace FromEdge currently requires Mode=SingleFace",
        ));
    }
    Ok(input)
}

#[derive(Clone, Debug)]
pub struct CompletedFace {
    pub points: Vec<Point3>,
    pub in_plane: bool,
    pub weld_angle: Real,
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub preferences: Preferences,
    pub points: Vec<Point3>,
    faces: Vec<CompletedFace>,
    source: Option<TriangleMesh>,
    seed: usize,
    total: usize,
    pub finished: bool,
}
impl Draft {
    pub fn new(
        preferences: Preferences,
        source: Option<TriangleMesh>,
        edge: Option<usize>,
    ) -> Result<Self, CommandError> {
        if !preferences.weld_angle.is_finite()
            || !(0. ..=180.).contains(&preferences.weld_angle)
            || (preferences.from_edge && preferences.multiple)
        {
            return Err(CommandError::Usage(USAGE));
        }
        let points = if preferences.from_edge {
            source
                .as_ref()
                .ok_or(CommandError::Usage(USAGE))?
                .boundary_face_seed(edge.ok_or(CommandError::Usage(USAGE))?)?
                .to_vec()
        } else {
            vec![]
        };
        let seed = points.len();
        Ok(Self {
            preferences,
            points,
            faces: vec![],
            source,
            seed,
            total: seed,
            finished: false,
        })
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
            .or_else(|| self.faces.last()?.points.last().copied())
    }
    pub fn clear_points(&mut self) {
        self.total -= self.points.len();
        self.points.clear();
        self.seed = 0;
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
            Token::Point(p) => {
                if self.total >= viboceros_geometry::MAX_MESH_PATCH_POINTS {
                    return Err(GeometryError::MeshPatchLimit.into());
                }
                let p = self.constrained_corner(p)?;
                if self.points.last() == Some(&p) {
                    return Err(CommandError::Usage("3DFace corners must be distinct"));
                }
                if self.preferences.polygon != Polygon::Ngon
                    || (self.preferences.in_plane && self.points.len() == 2)
                {
                    let mut candidate = self.points.clone();
                    candidate.push(p);
                    if candidate.len() >= 3 {
                        TriangleMesh::try_face_from_points(&candidate, self.preferences.in_plane)?;
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
                if self.points.len() > self.seed {
                    self.points.pop();
                    self.total -= 1;
                } else if let Some(mut previous) = self.faces.pop() {
                    previous.points.pop();
                    self.points = previous.points;
                    self.total -= 1;
                }
            }
            Token::Enter => {
                if self.points.len() >= 3 {
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
    fn complete(&mut self) -> Result<(), CommandError> {
        TriangleMesh::try_face_from_points(&self.points, self.preferences.in_plane)?;
        self.faces.push(CompletedFace {
            points: std::mem::take(&mut self.points),
            in_plane: self.preferences.in_plane,
            weld_angle: self.preferences.weld_angle,
        });
        self.finished = !self.preferences.multiple;
        if !self.finished && self.preferences.from_edge {
            self.finished = true;
        }
        Ok(())
    }
    pub fn mesh(&self) -> Result<Option<TriangleMesh>, CommandError> {
        if self.faces.is_empty() {
            return Ok(None);
        }
        let parts = self
            .faces
            .iter()
            .map(|f| TriangleMesh::try_face_from_points(&f.points, f.in_plane))
            .collect::<Result<Vec<_>, _>>()?;
        if self
            .faces
            .iter()
            .any(|f| f.weld_angle != self.faces[0].weld_angle)
        {
            let mut mesh = self.source.clone();
            for (face, part) in self.faces.iter().zip(parts) {
                let count = mesh.as_ref().map_or(0, |m| m.faces().len());
                let added = if let Some(original) = mesh {
                    TriangleMesh::try_append(&[&original, &part])?
                } else {
                    part
                };
                mesh = Some(
                    added
                        .welded_vertices_after(face.weld_angle.to_radians(), count)?
                        .0,
                );
            }
            return Ok(mesh);
        }
        let refs = self.source.iter().chain(parts.iter()).collect::<Vec<_>>();
        let mesh = TriangleMesh::try_append(&refs)?;
        let first = self.source.as_ref().map_or(0, |m| m.faces().len());
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
                document.is_object_selectable(id) && matches!(o.geometry(), Geometry::Mesh(_))
            })
            .ok_or(CommandError::Usage(USAGE))?;
        return Ok(Some(id));
    }
    let mut selected = document
        .selected_objects()
        .filter(|o| matches!(o.geometry(), Geometry::Mesh(_)))
        .map(|o| o.id());
    let first = selected.next();
    if selected.next().is_some() {
        return Err(CommandError::Usage("3DFace requires one append target"));
    }
    Ok(first)
}
pub fn source(document: &Document, target: Option<ObjectId>) -> Option<TriangleMesh> {
    target
        .and_then(|id| document.object(id))
        .and_then(|o| match o.geometry() {
            Geometry::Mesh(m) => Some(m.clone()),
            _ => None,
        })
}
pub fn apply(
    document: &mut Document,
    target: Option<ObjectId>,
    mesh: TriangleMesh,
) -> Result<ObjectId, CommandError> {
    if let Some(id) = target {
        document.select_objects_direct([id], SelectionMode::Remove)?;
        document.replace_object_geometries(vec![(id, Geometry::Mesh(mesh))])?;
        Ok(id)
    } else {
        document
            .add_geometry(Geometry::Mesh(mesh))
            .map_err(Into::into)
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
        let mut draft = Draft::new(input.preferences, source(document, target), input.edge)?;
        for token in input.tokens {
            draft.accept(token)?;
        }
        if !draft.finished {
            draft.accept(Token::Enter)?;
        }
        let Some(mesh) = draft.mesh()? else {
            return Err(CommandError::Usage(USAGE));
        };
        let id = apply(document, target, mesh)?;
        self.0.set(input.preferences);
        Ok(format!(
            "Created {} mesh face(s) on object {id}",
            draft.face_count()
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
