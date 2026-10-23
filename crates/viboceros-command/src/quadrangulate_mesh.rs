//! Scriptable triangle-to-quad mesh cleanup.

use super::*;

const USAGE: &str = "QuadrangulateMesh [Planarity=degrees] [Rectangularity=ratio]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub planarity: Real,
    pub rectangularity: Real,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            planarity: 5.,
            rectangularity: 2.,
        }
    }
}
impl Options {
    pub fn command(self) -> String {
        format!(
            "QuadrangulateMesh Planarity={} Rectangularity={}",
            self.planarity, self.rectangularity
        )
    }
}
/// Parse a complete option update before accepting either value.
pub fn parse(arguments: &[&str], default: Options) -> Result<Options, CommandError> {
    let mut planarity = None;
    let mut rectangularity = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (key, value, consumed) = if let Some((key, value)) = argument.split_once('=') {
            (key, value, 1)
        } else {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            (argument, *value, 2)
        };
        if option_name_eq(key, "Planarity") && planarity.is_none() {
            planarity = Some(
                value
                    .parse::<Real>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            );
        } else if option_name_eq(key, "Rectangularity") && rectangularity.is_none() {
            rectangularity = Some(
                value
                    .parse::<Real>()
                    .map_err(|_| CommandError::Usage(USAGE))?,
            );
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    let planarity = planarity.unwrap_or(default.planarity);
    let rectangularity = rectangularity.unwrap_or(default.rectangularity);
    if !planarity.is_finite()
        || !(0.0..=180.0).contains(&planarity)
        || !rectangularity.is_finite()
        || rectangularity < 1.0
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(Options {
        planarity,
        rectangularity,
    })
}

pub(super) struct QuadrangulateMeshCommand(pub std::sync::Arc<remembered::Remembered<Options>>);

impl CommandRegistry {
    pub fn quadrangulate_mesh_options(&self, args: &[&str]) -> Result<Options, CommandError> {
        parse(args, self.quadrangulate_mesh_preferences.get())
    }
}

impl Command for QuadrangulateMeshCommand {
    fn name(&self) -> &'static str {
        "QuadrangulateMesh"
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(args, self.0.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: "QuadrangulateMesh",
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
                    return Err(CommandError::UnsupportedQuadrangulateMeshGeometry);
                };
                let (converted, count) = mesh.quadrangulate_triangles(
                    options.planarity,
                    options.rectangularity,
                    document.tolerance(),
                )?;
                Ok((object.id(), converted, count))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if staged.is_empty() {
            return Err(CommandError::UnsupportedQuadrangulateMeshGeometry);
        }
        let converted_faces = staged.iter().map(|(_, _, count)| count).sum::<usize>();
        let changed_meshes = staged.iter().filter(|(_, _, count)| *count > 0).count();
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
            "Quadrangulated {converted_faces} triangle pair(s) in {changed_meshes} mesh(es)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn square() -> TriangleMesh {
        TriangleMesh::try_new(
            vec![
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(0.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn edits_selected_meshes_atomically_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let first = document.add_geometry(Geometry::Mesh(square())).unwrap();
        let second = document.add_geometry(Geometry::Mesh(square())).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "QuadrangulateMesh Planarity=0 Rectangularity=1"
                )
                .unwrap(),
            "Quadrangulated 2 triangle pair(s) in 2 mesh(es)"
        );
        for id in [first, second] {
            let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(mesh.face_count(), 1);
            assert!(mesh.faces()[0].is_quad());
        }
        assert_eq!(document.undo_label(), Some("QuadrangulateMesh"));
        document.undo().unwrap();
        assert_eq!(
            registry.quadrangulate_mesh_options(&[]).unwrap(),
            Options {
                planarity: 0.,
                rectangularity: 1.
            }
        );
        assert_eq!(
            CommandRegistry::with_builtins()
                .quadrangulate_mesh_options(&[])
                .unwrap(),
            Options::default()
        );
        for id in [first, second] {
            let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(mesh.face_count(), 2);
        }
    }

    #[test]
    fn invalid_options_preserve_document_and_preferences() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let mesh = document.add_geometry(Geometry::Mesh(square())).unwrap();
        let point = document
            .add_geometry(Geometry::Point(point(2.0, 0.0)))
            .unwrap();
        document
            .select_objects_direct([mesh, point], SelectionMode::Replace)
            .unwrap();
        let before = document.undo_label().map(str::to_owned);
        for input in [
            "QuadrangulateMesh Planarity=-1",
            "QuadrangulateMesh Rectangularity=0.5",
            "QuadrangulateMesh Planarity=1 Planarity=2",
            "QuadrangulateMesh _Planarity 1 Planarity=2",
            "QuadrangulateMesh Rectangularity=2 _Rectangularity 3",
            "QuadrangulateMesh Planarity",
            "QuadrangulateMesh Rectangularity",
            "QuadrangulateMesh Planarity=NaN",
            "QuadrangulateMesh Rectangularity inf",
            "QuadrangulateMesh Unexpected=1",
            "QuadrangulateMesh Planarity=inf",
        ] {
            assert!(registry.execute(&mut document, input).is_err());
            let Geometry::Mesh(current) = document.object(mesh).unwrap().geometry() else {
                panic!("mesh expected")
            };
            assert_eq!(current.face_count(), 2);
            assert_eq!(document.undo_label(), before.as_deref());
            assert_eq!(
                registry.quadrangulate_mesh_options(&[]).unwrap(),
                Options::default()
            );
        }
    }
}
