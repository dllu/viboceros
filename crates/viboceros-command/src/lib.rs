//! Extensible command registry and the first model-editing commands.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;
use viboceros_document::{ColorRgb, Document, DocumentError, Geometry};
use viboceros_geometry::{GeometryError, LineSegment, NurbsCurve, Point3, Real, TriangleMesh};
use viboceros_io::{StlError, StlFormat, read_stl_file, write_stl_file};

pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether successful mutations should be grouped into one undo step.
    fn records_history(&self) -> bool {
        true
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError>;
}

#[derive(Default)]
pub struct CommandRegistry {
    commands: Vec<Box<dyn Command>>,
    lookup: BTreeMap<String, usize>,
}

impl CommandRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        registry
            .register(PointCommand)
            .expect("unique built-in command");
        registry
            .register(LineCommand)
            .expect("unique built-in command");
        registry
            .register(ControlPointCurveCommand)
            .expect("unique built-in command");
        registry
            .register(LayerCommand)
            .expect("unique built-in command");
        registry
            .register(ClearCommand)
            .expect("unique built-in command");
        registry
            .register(UndoCommand)
            .expect("unique built-in command");
        registry
            .register(RedoCommand)
            .expect("unique built-in command");
        registry
            .register(ImportStlCommand)
            .expect("unique built-in command");
        registry
            .register(ExportStlCommand)
            .expect("unique built-in command");
        registry
    }

    pub fn register(&mut self, command: impl Command + 'static) -> Result<(), CommandError> {
        let index = self.commands.len();
        let keys = std::iter::once(command.name())
            .chain(command.aliases().iter().copied())
            .map(normalize_command_name)
            .collect::<BTreeSet<_>>();
        if let Some(duplicate) = keys.iter().find(|key| self.lookup.contains_key(*key)) {
            return Err(CommandError::DuplicateCommand(duplicate.clone()));
        }

        for key in keys {
            self.lookup.insert(key, index);
        }
        self.commands.push(Box::new(command));
        Ok(())
    }

    pub fn execute(&self, document: &mut Document, input: &str) -> Result<String, CommandError> {
        let mut tokens = input.split_whitespace();
        let name = tokens.next().ok_or(CommandError::EmptyInput)?;
        let name = normalize_command_name(name);

        if name == "help" || name == "?" {
            return Ok(format!("Commands: {}", self.command_names().join(", ")));
        }

        let index = self
            .lookup
            .get(&name)
            .copied()
            .ok_or_else(|| CommandError::UnknownCommand(name.clone()))?;
        let arguments: Vec<_> = tokens.collect();
        let command = &self.commands[index];
        if !command.records_history() {
            return command.run(document, &arguments);
        }

        document.begin_transaction(command.name())?;
        match command.run(document, &arguments) {
            Ok(message) => {
                document.commit_transaction()?;
                Ok(message)
            }
            Err(error) => {
                document.rollback_transaction()?;
                Err(error)
            }
        }
    }

    pub fn command_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.commands.iter().map(|command| command.name()).collect();
        names.sort_unstable_by_key(|name| name.to_ascii_lowercase());
        names
    }
}

fn normalize_command_name(name: &str) -> String {
    name.trim_start_matches(['_', '-']).to_ascii_lowercase()
}

struct PointCommand;

impl Command for PointCommand {
    fn name(&self) -> &'static str {
        "Point"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Pt"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (point, consumed) = parse_point(arguments)?;
        require_consumed(arguments, consumed, "Point x,y,z")?;
        let id = document.add_geometry(Geometry::Point(point))?;
        Ok(format!("Added point {id} at {}", format_point(point)))
    }
}

struct LineCommand;

impl Command for LineCommand {
    fn name(&self) -> &'static str {
        "Line"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["L"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (start, consumed) = parse_point(arguments)?;
        let (end, end_consumed) = parse_point(&arguments[consumed..])?;
        require_consumed(arguments, consumed + end_consumed, "Line x1,y1,z1 x2,y2,z2")?;
        let line = LineSegment::try_new(start, end, document.tolerance())?;
        let length = line.length()?;
        let id = document.add_geometry(Geometry::Line(line))?;
        Ok(format!("Added line {id} (length {length:.6})"))
    }
}

struct LayerCommand;

struct ControlPointCurveCommand;

impl Command for ControlPointCurveCommand {
    fn name(&self) -> &'static str {
        "ControlPointCurve"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["CPCurve"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let degree_text = arguments.first().ok_or(CommandError::Usage(
            "ControlPointCurve degree point1 point2 ...",
        ))?;
        let degree = degree_text
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidInteger((*degree_text).to_owned()))?;

        let mut control_points = Vec::new();
        let mut consumed = 1;
        while consumed < arguments.len() {
            let (point, point_tokens) = parse_point(&arguments[consumed..])?;
            control_points.push(point);
            consumed += point_tokens;
        }

        let control_point_count = control_points.len();
        let curve = NurbsCurve::try_clamped_uniform(degree, control_points)?;
        let id = document.add_geometry(Geometry::NurbsCurve(curve))?;
        Ok(format!(
            "Added degree {degree} control-point curve {id} ({control_point_count} control points)"
        ))
    }
}

impl Command for LayerCommand {
    fn name(&self) -> &'static str {
        "Layer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("Layer name"));
        }
        let name = arguments.join(" ");
        let color = layer_color(document.layers().len());
        let id = document.add_layer(name.clone(), color)?;
        document.set_current_layer(id)?;
        Ok(format!("Created current layer '{name}'"))
    }
}

struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &'static str {
        "Clear"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["DeleteAll"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Clear")?;
        let count = document.clear_objects();
        Ok(format!("Deleted {count} object(s)"))
    }
}

struct UndoCommand;

impl Command for UndoCommand {
    fn name(&self) -> &'static str {
        "Undo"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Undo")?;
        Ok(match document.undo()? {
            Some(label) => format!("Undid {label}"),
            None => "Nothing to undo".to_owned(),
        })
    }
}

struct RedoCommand;

impl Command for RedoCommand {
    fn name(&self) -> &'static str {
        "Redo"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Redo")?;
        Ok(match document.redo()? {
            Some(label) => format!("Redid {label}"),
            None => "Nothing to redo".to_owned(),
        })
    }
}

struct ImportStlCommand;

impl Command for ImportStlCommand {
    fn name(&self) -> &'static str {
        "ImportStl"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ImportStl path"));
        }
        let path = arguments.join(" ");
        let mesh = read_stl_file(&path, document.tolerance())?;
        let triangle_count = mesh.triangles().len();
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Imported STL mesh {id} ({triangle_count} triangles) from '{path}'"
        ))
    }
}

struct ExportStlCommand;

impl Command for ExportStlCommand {
    fn name(&self) -> &'static str {
        "ExportStl"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("ExportStl [Ascii|Binary] path"));
        }
        let (format, path_arguments) = if arguments[0].eq_ignore_ascii_case("ascii") {
            (StlFormat::Ascii, &arguments[1..])
        } else if arguments[0].eq_ignore_ascii_case("binary") {
            (StlFormat::Binary, &arguments[1..])
        } else {
            (StlFormat::Binary, arguments)
        };
        if path_arguments.is_empty() {
            return Err(CommandError::Usage("ExportStl [Ascii|Binary] path"));
        }
        let path = path_arguments.join(" ");
        let mesh = combined_document_mesh(document)?;
        let triangle_count = mesh.triangles().len();
        write_stl_file(&path, &mesh, format)?;
        Ok(format!(
            "Exported {triangle_count} triangles as {format:?} STL to '{path}'"
        ))
    }
}

fn combined_document_mesh(document: &Document) -> Result<TriangleMesh, CommandError> {
    let meshes: Vec<_> = document
        .objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Mesh(mesh)
                if object.attributes().is_visible()
                    && document
                        .layer(object.attributes().layer_id())
                        .is_some_and(|layer| layer.is_visible()) =>
            {
                Some(mesh)
            }
            _ => None,
        })
        .collect();
    if meshes.is_empty() {
        return Err(CommandError::NoMeshToExport);
    }

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for mesh in meshes {
        let offset =
            u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.extend_from_slice(mesh.vertices());
        for triangle in mesh.triangles() {
            triangles.push([
                triangle[0]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
                triangle[1]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
                triangle[2]
                    .checked_add(offset)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
            ]);
        }
    }
    Ok(TriangleMesh::try_new(
        vertices,
        triangles,
        document.tolerance(),
    )?)
}

fn layer_color(index: usize) -> ColorRgb {
    const PALETTE: [ColorRgb; 6] = [
        ColorRgb::new(40, 110, 220),
        ColorRgb::new(220, 75, 60),
        ColorRgb::new(30, 155, 85),
        ColorRgb::new(190, 125, 20),
        ColorRgb::new(145, 80, 190),
        ColorRgb::new(20, 155, 165),
    ];
    PALETTE[index % PALETTE.len()]
}

fn parse_point(arguments: &[&str]) -> Result<(Point3, usize), CommandError> {
    let first = arguments
        .first()
        .ok_or(CommandError::Usage("expected a point"))?;
    let (coordinates, consumed) = if first.contains(',') {
        let coordinates: Vec<_> = first.split(',').collect();
        if !(2..=3).contains(&coordinates.len()) {
            return Err(CommandError::Usage("point syntax is x,y or x,y,z"));
        }
        (coordinates, 1)
    } else {
        if arguments.len() < 3 {
            return Err(CommandError::Usage("point syntax is x y z or x,y,z"));
        }
        (arguments[..3].to_vec(), 3)
    };

    let mut parsed = [0.0; 3];
    for (index, coordinate) in coordinates.iter().enumerate() {
        parsed[index] = coordinate
            .parse::<Real>()
            .map_err(|_| CommandError::InvalidNumber((*coordinate).to_owned()))?;
    }
    Ok((Point3::try_from(parsed)?, consumed))
}

fn require_consumed(
    arguments: &[&str],
    consumed: usize,
    usage: &'static str,
) -> Result<(), CommandError> {
    if arguments.len() == consumed {
        Ok(())
    } else {
        Err(CommandError::Usage(usage))
    }
}

fn format_point(point: Point3) -> String {
    format!("{:.6},{:.6},{:.6}", point.x(), point.y(), point.z())
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("enter a command")]
    EmptyInput,

    #[error("unknown command '{0}'; enter Help to list commands")]
    UnknownCommand(String),

    #[error("command name or alias '{0}' is already registered")]
    DuplicateCommand(String),

    #[error("usage: {0}")]
    Usage(&'static str),

    #[error("'{0}' is not a valid finite number")]
    InvalidNumber(String),

    #[error("'{0}' is not a valid non-negative integer")]
    InvalidInteger(String),

    #[error("the document contains no visible triangle meshes to export")]
    NoMeshToExport,

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error(transparent)]
    Stl(#[from] StlError),

    #[error(transparent)]
    Document(#[from] DocumentError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_case_insensitively_and_accepts_rhino_prefix() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "_pT 1,2,3").unwrap();
        assert_eq!(document.objects().len(), 1);
    }

    #[test]
    fn creates_lines_from_comma_or_space_points() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0 3,4,0").unwrap();
        registry.execute(&mut document, "Line 0 0 0 0 0 2").unwrap();
        assert_eq!(document.objects().len(), 2);
    }

    #[test]
    fn failed_command_does_not_mutate_document() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert!(registry.execute(&mut document, "Line 0,0,0 0,0,0").is_err());
        assert_eq!(document.objects().len(), 0);
    }

    #[test]
    fn reports_sorted_help() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        assert_eq!(
            registry.execute(&mut document, "Help").unwrap(),
            "Commands: Clear, ControlPointCurve, ExportStl, ImportStl, Layer, Line, Point, Redo, Undo"
        );
    }

    #[test]
    fn creates_a_clamped_control_point_curve() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "ControlPointCurve 3 0,0 2,3 5,3 8,0")
            .unwrap();
        let object = document.objects().next().unwrap();
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("expected a NURBS curve")
        };
        assert_eq!(curve.degree(), 3);
        assert_eq!(curve.control_points().len(), 4);
        assert_eq!(
            curve.evaluate(0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap()
        );
        assert_eq!(
            curve.evaluate(1.0).unwrap(),
            Point3::try_new(8.0, 0.0, 0.0).unwrap()
        );
    }

    #[test]
    fn undo_and_redo_commands_replay_a_model_edit() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        assert_eq!(document.objects().len(), 1);

        assert_eq!(
            registry.execute(&mut document, "Undo").unwrap(),
            "Undid Point"
        );
        assert_eq!(document.objects().len(), 0);
        assert_eq!(
            registry.execute(&mut document, "Redo").unwrap(),
            "Redid Point"
        );
        assert_eq!(document.objects().len(), 1);
    }

    #[test]
    fn compound_layer_command_is_one_undo_step() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let default_layer = document.current_layer_id();
        registry
            .execute(&mut document, "Layer Construction")
            .unwrap();
        assert_eq!(document.layers().len(), 2);
        assert_ne!(document.current_layer_id(), default_layer);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.layers().len(), 1);
        assert_eq!(document.current_layer_id(), default_layer);
    }

    struct MutateThenFailCommand;

    impl Command for MutateThenFailCommand {
        fn name(&self) -> &'static str {
            "MutateThenFail"
        }

        fn run(
            &self,
            document: &mut Document,
            _arguments: &[&str],
        ) -> Result<String, CommandError> {
            document.add_geometry(Geometry::Point(Point3::try_new(9.0, 9.0, 9.0).unwrap()))?;
            Err(CommandError::Usage("this command always fails"))
        }
    }

    #[test]
    fn failed_command_rolls_back_mutations_atomically() {
        let mut registry = CommandRegistry::default();
        registry.register(MutateThenFailCommand).unwrap();
        let mut document = Document::default();
        assert!(registry.execute(&mut document, "MutateThenFail").is_err());
        assert_eq!(document.objects().len(), 0);
        assert!(!document.can_undo());
    }

    #[test]
    fn imports_and_exports_stl_through_commands() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-source model.stl",
            std::process::id()
        ));
        let output = std::env::temp_dir().join(format!(
            "viboceros-{}-{unique}-output model.stl",
            std::process::id()
        ));
        fs::write(
            &source,
            "solid test\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid test\n",
        )
        .unwrap();

        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, &format!("ImportStl {}", source.display()))
            .unwrap();
        assert!(matches!(
            document.objects().next().unwrap().geometry(),
            Geometry::Mesh(_)
        ));
        registry
            .execute(
                &mut document,
                &format!("ExportStl Binary {}", output.display()),
            )
            .unwrap();
        let exported = read_stl_file(&output, document.tolerance()).unwrap();
        assert_eq!(exported.triangles().len(), 1);

        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().len(), 0);
        fs::remove_file(source).unwrap();
        fs::remove_file(output).unwrap();
    }
}
