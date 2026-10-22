//! The two vertex commands use one validated selector and nearest-point picker.
use super::*;

pub(crate) fn run(
    document: &mut Document,
    mut mode: VertexSeamMode,
    arguments: &[&str],
) -> Result<String, CommandError> {
    let usage = match mode {
        VertexSeamMode::Weld => "WeldVertices (point|Vertices=All|Vertices=0,2,...)",
        VertexSeamMode::Unweld { .. } => {
            "UnweldVertex (point|Vertices=All|Vertices=0,2,...) [ModifyNormals=Yes|No]"
        }
    };
    let mut indices = None;
    let mut normals = None;
    let mut positional = Vec::new();
    let mut i = 0;
    while i < arguments.len() {
        let arg = arguments[i];
        let option = if let Some((name, value)) = arg.split_once('=') {
            Some((name, value, 1))
        } else if ["Vertices", "VertexIndices", "ModifyNormals"]
            .iter()
            .any(|name| option_name_eq(arg, name))
        {
            Some((
                arg,
                *arguments.get(i + 1).ok_or(CommandError::Usage(usage))?,
                2,
            ))
        } else {
            None
        };
        if let Some((name, value, used)) = option {
            if (option_name_eq(name, "Vertices") || option_name_eq(name, "VertexIndices"))
                && indices.is_none()
            {
                indices = Some(parse_surface_face_indices(value, usage)?);
            } else if option_name_eq(name, "ModifyNormals")
                && matches!(mode, VertexSeamMode::Unweld { .. })
                && normals.is_none()
            {
                normals = Some(parse_yes_no(value).ok_or(CommandError::Usage(usage))?);
            } else {
                return Err(CommandError::Usage(usage));
            }
            i += used;
        } else {
            positional.push(arg);
            i += 1;
        }
    }
    if let Some(modify_normals) = normals {
        mode = VertexSeamMode::Unweld { modify_normals };
    }
    let bare = indices.is_none() && positional.is_empty();
    if bare {
        let available = available(document);
        let targets = document
            .selected_control_points()
            .filter(|(g, _)| {
                available.contains(&g.object)
                    && document
                        .object(g.object)
                        .is_some_and(|o| matches!(o.geometry(), Geometry::SubD(_)))
            })
            .map(|(g, _)| VertexSeamTarget {
                object: g.object,
                index: g.index,
            })
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return Err(CommandError::Usage(usage));
        }
        return VertexSeamEdit::prepare(document, mode, targets)?
            .apply(document)
            .map(|out| out.message);
    }
    let point = if indices.is_none() {
        let (point, consumed) = parse_point(&positional)?;
        require_consumed(&positional, consumed, usage)?;
        Some(point)
    } else {
        require_consumed(&positional, 0, usage)?;
        None
    };
    // Validate every selected object before computing any topology or distances.
    let sources = document.selected_objects().collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    if sources
        .iter()
        .any(|o| !matches!(o.geometry(), Geometry::Mesh(_) | Geometry::SubD(_)))
    {
        return Err(mode.unsupported());
    }
    let mut targets = Vec::new();
    if let Some(selection) = indices {
        for object in &sources {
            let count = vertex_count(object.geometry(), mode)?;
            let indices = match &selection {
                SurfaceFaceIndices::All => (0..count).collect(),
                SurfaceFaceIndices::Indices(values) => values.clone(),
            };
            if let Some(&index) = indices.iter().find(|&&i| i >= count) {
                return Err(mode.index_error(index, count));
            }
            targets.extend(indices.into_iter().map(|index| VertexSeamTarget {
                object: object.id(),
                index,
            }));
        }
    } else {
        let target = point.unwrap();
        let mut best = None;
        for object in &sources {
            let mut consider = |index, point: Point3| -> Result<(), CommandError> {
                let distance = point.distance_to(target)?;
                if best.is_none_or(|(d, _)| distance < d) {
                    best = Some((
                        distance,
                        VertexSeamTarget {
                            object: object.id(),
                            index,
                        },
                    ));
                }
                Ok(())
            };
            match object.geometry() {
                Geometry::Mesh(mesh) => {
                    for (i, p) in mesh.topology_vertex_points().into_iter().enumerate() {
                        consider(i, p)?;
                    }
                }
                Geometry::SubD(subd) => {
                    for (i, (&control, &surface)) in subd
                        .control_points()
                        .iter()
                        .zip(subd.surface_points())
                        .enumerate()
                    {
                        consider(i, control)?;
                        consider(i, surface)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        targets.push(best.ok_or_else(|| mode.empty())?.1);
    }
    let count = sources
        .iter()
        .filter(|o| matches!(o.geometry(), Geometry::Mesh(_)))
        .count();
    let mut plan = VertexSeamEdit::prepare(document, mode, targets)?;
    plan.source_count = count;
    plan.apply(document).map(|out| out.message)
}
