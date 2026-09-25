use super::*;
use viboceros_geometry::{MeshAlignSelection, align_mesh_vertices};

const USAGE: &str = "AlignVertices [DistanceToAdjust=value] [AverageVertexesToAdjust=Yes|No] [Vertices=0,2,...|NakedEdges=0,2,...]";

pub(super) struct AlignVerticesCommand;

impl Command for AlignVerticesCommand {
    fn name(&self) -> &'static str {
        "AlignVertices"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_options(arguments, document.tolerance().absolute())?;
        let selected = document
            .selected_objects()
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedAlignVerticesGeometry);
                };
                Ok((object.id(), mesh))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let meshes = selected.iter().map(|(_, mesh)| *mesh).collect::<Vec<_>>();
        let results = align_mesh_vertices(
            &meshes,
            options.distance,
            options.average,
            &options.selection,
            document.tolerance(),
        )?;
        let moved = results.iter().map(|(_, moved)| moved).sum::<usize>();
        let changed = results.iter().filter(|(_, moved)| *moved > 0).count();
        let replacements = selected
            .iter()
            .map(|(id, _)| *id)
            .zip(results)
            .filter(|(_, (_, moved))| *moved > 0)
            .map(|(id, (mesh, _))| (id, Geometry::Mesh(mesh)))
            .collect::<Vec<_>>();
        document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Aligned {moved} mesh vertex/vertices in {changed} mesh(es)"
        ))
    }
}

struct AlignOptions {
    distance: Real,
    average: bool,
    selection: MeshAlignSelection,
}

fn parse_options(arguments: &[&str], default_distance: Real) -> Result<AlignOptions, CommandError> {
    let mut distance = None;
    let mut average = None;
    let mut vertices = None;
    let mut edges = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?;
            (argument, *value, 2)
        };
        if (option_name_eq(name, "DistanceToAdjust") || option_name_eq(name, "Distance"))
            && distance.is_none()
        {
            distance = Some(parse_finite_real(value)?);
        } else if (option_name_eq(name, "AverageVertexesToAdjust")
            || option_name_eq(name, "Average"))
            && average.is_none()
        {
            average = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
        } else if (option_name_eq(name, "Vertices") || option_name_eq(name, "SelectVertices"))
            && vertices.is_none()
        {
            vertices = Some(parse_indices(value)?);
        } else if (option_name_eq(name, "NakedEdges") || option_name_eq(name, "SelectNakedEdges"))
            && edges.is_none()
        {
            edges = Some(parse_indices(value)?);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    let distance = distance.unwrap_or(default_distance);
    if distance <= 0. || vertices.is_some() && edges.is_some() {
        return Err(CommandError::Usage(USAGE));
    }
    let selection = if let Some(vertices) = vertices {
        MeshAlignSelection::Vertices(vertices)
    } else if let Some(edges) = edges {
        MeshAlignSelection::NakedEdges(edges)
    } else {
        MeshAlignSelection::AllNaked
    };
    Ok(AlignOptions {
        distance,
        average: average.unwrap_or(false),
        selection,
    })
}

fn parse_indices(value: &str) -> Result<Vec<usize>, CommandError> {
    let indices = value
        .trim_start_matches('_')
        .split(',')
        .map(|item| {
            item.parse::<usize>()
                .map_err(|_| CommandError::Usage(USAGE))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if indices.is_empty() || indices.iter().copied().collect::<BTreeSet<_>>().len() != indices.len()
    {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_selected_meshes_with_undo_and_rejects_mixed_selection() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let make = |x| {
            TriangleMesh::try_new(
                vec![
                    Point3::try_new(x, 0., 0.).unwrap(),
                    Point3::try_new(x + 1., 0., 0.).unwrap(),
                    Point3::try_new(x, 1., 0.).unwrap(),
                ],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap()
        };
        let left = make(0.);
        let right = make(1.04);
        let a = document.add_geometry(Geometry::Mesh(left.clone())).unwrap();
        let b = document
            .add_geometry(Geometry::Mesh(right.clone()))
            .unwrap();
        let point = document
            .add_geometry(Geometry::Point(Point3::try_new(5., 0., 0.).unwrap()))
            .unwrap();
        document
            .select_objects_direct([a, b, point], SelectionMode::Replace)
            .unwrap();
        assert!(matches!(
            registry.execute(&mut document, "AlignVertices DistanceToAdjust=0.05"),
            Err(CommandError::UnsupportedAlignVerticesGeometry)
        ));
        assert_eq!(
            document.object(b).unwrap().geometry(),
            &Geometry::Mesh(right.clone())
        );
        document
            .select_objects_direct([a, b], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            registry
                .execute(&mut document, "AlignVertices DistanceToAdjust=0.05")
                .unwrap(),
            "Aligned 1 mesh vertex/vertices in 1 mesh(es)"
        );
        let Geometry::Mesh(aligned) = document.object(b).unwrap().geometry() else {
            panic!("expected mesh");
        };
        assert_eq!(aligned.vertices()[0], left.vertices()[1]);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(b).unwrap().geometry(),
            &Geometry::Mesh(right)
        );
        assert_eq!(
            registry
                .execute(
                    &mut document,
                    "AlignVertices DistanceToAdjust=0.05 AverageVertexesToAdjust=Yes"
                )
                .unwrap(),
            "Aligned 2 mesh vertex/vertices in 2 mesh(es)"
        );
        assert!(
            registry
                .execute(&mut document, "AlignVertices DistanceToAdjust=0")
                .is_err()
        );
    }
}
