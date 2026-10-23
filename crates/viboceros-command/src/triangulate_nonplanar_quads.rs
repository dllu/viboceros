//! Native selective quad conversion with command-instance option memory.
use super::*;
use viboceros_geometry::{NonPlanarQuadCriterion, QuadSplitMethod};

const USAGE: &str = "TriangulateNonPlanarQuads [SelectBy=Distance|Angle|Both] [Distance=value] [Angle=degrees] [SplitMethod=ShortestDiagonal|LongestDiagonal|MinimumArea|MaximumArea|MinimumAngle|MaximumAngle]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectBy {
    Distance,
    Angle,
    Both,
}
impl SelectBy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Distance => "Distance",
            Self::Angle => "Angle",
            Self::Both => "Both",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub select_by: SelectBy,
    pub distance: Real,
    pub angle: Real,
    pub method: QuadSplitMethod,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            select_by: SelectBy::Distance,
            distance: 0.,
            angle: 0.,
            method: QuadSplitMethod::ShortestDiagonal,
        }
    }
}
impl Options {
    pub fn method_name(self) -> &'static str {
        match self.method {
            QuadSplitMethod::ShortestDiagonal => "ShortestDiagonal",
            QuadSplitMethod::LongestDiagonal => "LongestDiagonal",
            QuadSplitMethod::MinimizeArea => "MinimumArea",
            QuadSplitMethod::MaximizeArea => "MaximumArea",
            QuadSplitMethod::MinimumAngle => "MinimumAngle",
            QuadSplitMethod::MaximumAngle => "MaximumAngle",
        }
    }
    pub fn command(self) -> String {
        format!(
            "TriangulateNonPlanarQuads SelectBy={} Distance={} Angle={} SplitMethod={}",
            self.select_by.name(),
            self.distance,
            self.angle,
            self.method_name()
        )
    }
    fn criterion(self) -> NonPlanarQuadCriterion {
        let angle = self.angle.to_radians();
        match self.select_by {
            SelectBy::Distance => NonPlanarQuadCriterion::Distance(self.distance),
            SelectBy::Angle => NonPlanarQuadCriterion::Angle(angle),
            SelectBy::Both => NonPlanarQuadCriterion::Both {
                distance: self.distance,
                angle,
            },
        }
    }
}
/// Parse a complete update atomically. Mode and Split retain their local aliases.
pub fn parse(arguments: &[&str], defaults: Options) -> Result<Options, CommandError> {
    let mut options = defaults;
    let mut seen = [false; 4];
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        if option_name_eq(argument, "SelectBy") && index + 1 == arguments.len() {
            if seen[0] {
                return Err(CommandError::Usage(USAGE));
            }
            options.select_by = match options.select_by {
                SelectBy::Distance => SelectBy::Angle,
                _ => SelectBy::Distance,
            };
            seen[0] = true;
            index += 1;
            continue;
        }
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            (
                argument,
                *arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        let slot = if option_name_eq(name, "SelectBy") || option_name_eq(name, "Mode") {
            0
        } else if option_name_eq(name, "Distance") {
            1
        } else if option_name_eq(name, "Angle") {
            2
        } else if option_name_eq(name, "SplitMethod") || option_name_eq(name, "Split") {
            3
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if seen[slot] {
            return Err(CommandError::Usage(USAGE));
        }
        seen[slot] = true;
        match slot {
            0 => {
                options.select_by = if option_name_eq(value, "Distance") {
                    SelectBy::Distance
                } else if option_name_eq(value, "Angle") {
                    SelectBy::Angle
                } else if option_name_eq(value, "Both") {
                    SelectBy::Both
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            1 => options.distance = parse_finite_real(value)?,
            2 => options.angle = parse_finite_real(value)?,
            _ => {
                options.method = if option_name_eq(value, "ShortestDiagonal") {
                    QuadSplitMethod::ShortestDiagonal
                } else if option_name_eq(value, "LongestDiagonal") {
                    QuadSplitMethod::LongestDiagonal
                } else if option_name_eq(value, "MinimizeArea")
                    || option_name_eq(value, "MinimumArea")
                {
                    QuadSplitMethod::MinimizeArea
                } else if option_name_eq(value, "MaximizeArea")
                    || option_name_eq(value, "MaximumArea")
                {
                    QuadSplitMethod::MaximizeArea
                } else if option_name_eq(value, "MinimumAngle") {
                    QuadSplitMethod::MinimumAngle
                } else if option_name_eq(value, "MaximumAngle") {
                    QuadSplitMethod::MaximumAngle
                } else {
                    return Err(CommandError::Usage(USAGE));
                }
            }
        }
        index += consumed;
    }
    if !options.distance.is_finite()
        || options.distance < 0.
        || !options.angle.is_finite()
        || !(0. ..=180.).contains(&options.angle)
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(options)
}
pub(super) struct TriangulateNonPlanarQuadsCommand(
    pub std::sync::Arc<remembered::Remembered<Options>>,
);
impl CommandRegistry {
    pub fn triangulate_nonplanar_quads_options(
        &self,
        args: &[&str],
    ) -> Result<Options, CommandError> {
        parse(args, self.triangulate_nonplanar_quads_preferences.get())
    }
}
impl Command for TriangulateNonPlanarQuadsCommand {
    fn name(&self) -> &'static str {
        "TriangulateNonPlanarQuads"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(args, self.0.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: "TriangulateNonPlanarQuads",
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn object_selection_confirmation(
        &self,
        _: &Document,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(args)
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments, self.0.get())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .filter(|o| matches!(o.geometry(), Geometry::Mesh(_)))
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedTriangulateNonPlanarQuadsGeometry);
                };
                let (mesh, count) = mesh.triangulate_nonplanar_quads(
                    options.criterion(),
                    options.method,
                    document.tolerance(),
                )?;
                Ok((object.id(), mesh, count))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if staged.is_empty() {
            return Err(CommandError::UnsupportedTriangulateNonPlanarQuadsGeometry);
        }
        let converted = staged.iter().map(|(_, _, n)| n).sum::<usize>();
        let changed = staged.iter().filter(|(_, _, n)| *n > 0).count();
        let unchanged = staged.len() - changed;
        let ids = staged.iter().map(|(id, _, _)| *id).collect::<Vec<_>>();
        document.clear_selection();
        document.replace_object_geometries_with_history(
            staged
                .into_iter()
                .map(|(id, mesh, _)| (id, Geometry::Mesh(mesh))),
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        document.move_objects_to_end_in_order(ids)?;
        self.0.set(options);
        Ok(format!(
            "Triangulated {converted} nonplanar quad(s) in {changed} mesh(es); {unchanged} mesh(es) unchanged"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::MeshFace;

    #[test]
    fn native_option_names_toggles_aliases_and_invalid_updates_are_atomic() {
        let defaults = Options::default();
        assert_eq!(defaults.distance, 0.);
        assert_eq!(defaults.angle, 0.);
        let angle = parse(&["_SelectBy"], defaults).unwrap();
        assert_eq!(angle.select_by, SelectBy::Angle);
        assert_eq!(parse(&["SelectBy"], angle).unwrap(), defaults);
        for (native, alias) in [
            ("MinimumArea", "MinimizeArea"),
            ("MaximumArea", "MaximizeArea"),
        ] {
            assert_eq!(
                parse(&["_SplitMethod", native], defaults).unwrap(),
                parse(&["Split", alias], defaults).unwrap()
            );
        }
        for words in [
            vec!["SelectBy=Angle", "Mode=Both"],
            vec!["SplitMethod=MinimumArea", "Split=MinimizeArea"],
            vec!["Distance=1", "Angle=NaN"],
            vec!["Angle=181"],
            vec!["Distance=-1"],
            vec!["SplitMethod"],
            vec!["SelectBy=Angle", "SelectBy"],
        ] {
            assert!(parse(&words, defaults).is_err());
        }
        assert_eq!(parse(&[], defaults).unwrap(), defaults);
    }

    #[test]
    fn command_is_atomic_preserves_identity_and_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let vertices = [[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.4]]
            .map(|p| Point3::try_from(p).unwrap());
        let mesh = TriangleMesh::try_new_faces(
            vertices.to_vec(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        let point = document
            .add_geometry(Geometry::Point(Point3::try_new(4., 0., 0.).unwrap()))
            .unwrap();
        document
            .select_objects_direct([point], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "TriangulateNonPlanarQuads Distance=0.1"),
            Err(CommandError::UnsupportedTriangulateNonPlanarQuadsGeometry)
        ));
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh.clone())
        );
        document
            .select_objects_direct([id, point], SelectionMode::Replace)
            .unwrap();
        assert_eq!(registry.execute(&mut document, "TriangulateNonPlanarQuads Mode=Both Distance=0.2 Angle=1 SplitMethod=LongestDiagonal").unwrap(), "Triangulated 1 nonplanar quad(s) in 1 mesh(es); 0 mesh(es) unchanged");
        let Geometry::Mesh(result) = document.object(id).unwrap().geometry() else {
            panic!("expected mesh");
        };
        assert_eq!(result.face_count(), 2);
        assert_eq!(document.undo_label(), Some("TriangulateNonPlanarQuads"));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(id).unwrap().geometry(),
            &Geometry::Mesh(mesh)
        );
        assert!(
            registry
                .execute(&mut document, "TriangulateNonPlanarQuads Distance=-1")
                .is_err()
        );
    }
}
