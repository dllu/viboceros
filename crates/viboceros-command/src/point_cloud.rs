//! PointCloud creation and selected-cloud editing.
use super::*;

const USAGE: &str = "PointCloud [UsePointColors=No] | PointCloud Add [Target=<id>] | PointCloud Remove Indices=<zero-based-list> [Target=<id>] [Output=Points|PointCloud]";

enum Operation {
    Create {
        use_colors: bool,
    },
    Add {
        target: Option<ObjectId>,
    },
    Remove {
        target: Option<ObjectId>,
        indices: Option<BTreeSet<usize>>,
        output_cloud: bool,
    },
}

pub(super) struct PointCloudCommand;

impl Command for PointCloudCommand {
    fn name(&self) -> &'static str {
        "PointCloud"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        execute(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        execute(document, arguments, true)
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (filter, use_colors) = match parse(arguments)? {
            Operation::Create { use_colors } => {
                (ObjectSelectionFilter::PointCloudSources, use_colors)
            }
            Operation::Add { .. } => (ObjectSelectionFilter::PointCloudAddSources, false),
            Operation::Remove { indices: None, .. } => {
                (ObjectSelectionFilter::PointCloudRemoveTarget, false)
            }
            Operation::Remove {
                indices: Some(_), ..
            } => return Ok(None),
        };
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: if filter == ObjectSelectionFilter::PointCloudSources {
                vec![BooleanSelectionOption {
                    name: "UsePointColors",
                    value: use_colors,
                    aliases: &[],
                }]
            } else {
                vec![]
            },
            menus: vec![],
            choices: vec![],
        }))
    }
}

fn parse(arguments: &[&str]) -> Result<Operation, CommandError> {
    if arguments.is_empty() {
        return Ok(Operation::Create { use_colors: false });
    }
    let action = arguments[0].trim_start_matches('_');
    if action.eq_ignore_ascii_case("Add") || action.eq_ignore_ascii_case("Remove") {
        let remove = action.eq_ignore_ascii_case("Remove");
        let mut target = None;
        let mut indices = None;
        let mut output_cloud = false;
        let mut seen_output = false;
        for argument in &arguments[1..] {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let name = name.trim_start_matches('_');
            if name.eq_ignore_ascii_case("Target") && target.is_none() {
                target = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
            } else if remove && name.eq_ignore_ascii_case("Indices") && indices.is_none() {
                let parsed = value
                    .split(',')
                    .map(|part| {
                        part.parse::<usize>()
                            .map_err(|_| CommandError::Usage(USAGE))
                    })
                    .collect::<Result<BTreeSet<_>, _>>()?;
                if parsed.is_empty() {
                    return Err(CommandError::Usage(USAGE));
                }
                indices = Some(parsed);
            } else if remove && name.eq_ignore_ascii_case("Output") && !seen_output {
                output_cloud = if value.eq_ignore_ascii_case("PointCloud") {
                    true
                } else if value.eq_ignore_ascii_case("Points") {
                    false
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
                seen_output = true;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        return if remove {
            Ok(Operation::Remove {
                target,
                indices,
                output_cloud,
            })
        } else {
            Ok(Operation::Add { target })
        };
    }
    let (name, value, consumed) = orient_option(arguments, 0, USAGE)?;
    require_consumed(arguments, consumed, USAGE)?;
    if !option_name_eq(name, "UsePointColors") {
        return Err(CommandError::Usage(USAGE));
    }
    match parse_yes_no(value) {
        Some(use_colors) => Ok(Operation::Create { use_colors }),
        None => Err(CommandError::Usage(USAGE)),
    }
}

fn execute(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    match parse(arguments)? {
        Operation::Create { use_colors } => convert(document, postselected, use_colors),
        Operation::Add { target } => add(document, target),
        Operation::Remove {
            target,
            indices,
            output_cloud,
        } => remove(
            document,
            target,
            &indices.ok_or(CommandError::Usage(USAGE))?,
            output_cloud,
        ),
    }
}

fn selected_target(
    document: &Document,
    explicit: Option<ObjectId>,
) -> Result<ObjectId, CommandError> {
    if let Some(id) = explicit {
        return match document.object(id).map(|object| object.geometry()) {
            Some(Geometry::PointCloud(_)) => Ok(id),
            _ => Err(CommandError::PointCloudTargetRequired),
        };
    }
    let mut clouds = document
        .selected_objects()
        .filter(|object| matches!(object.geometry(), Geometry::PointCloud(_)));
    let target = clouds
        .next()
        .ok_or(CommandError::PointCloudTargetRequired)?
        .id();
    if clouds.next().is_some() {
        return Err(CommandError::PointCloudTargetAmbiguous);
    }
    Ok(target)
}

fn source_display_color(
    document: &Document,
    object: &viboceros_document::Object,
) -> Result<[u8; 4], CommandError> {
    let layer_id = object.attributes().layer_id();
    let layer = document
        .layer(layer_id)
        .ok_or(DocumentError::LayerNotFound(layer_id))?;
    let color = object.attributes().display_color(layer.color());
    Ok([color.red, color.green, color.blue, 0])
}

fn channels_for_indices(
    cloud: &PointCloud3,
    indices: &[usize],
) -> viboceros_geometry::PointCloudChannels {
    fn select<T: Copy>(values: Option<&[T]>, indices: &[usize]) -> Option<Vec<T>> {
        values.map(|values| indices.iter().map(|&index| values[index]).collect())
    }
    viboceros_geometry::PointCloudChannels {
        colors: select(cloud.colors(), indices),
        normals: select(cloud.normals(), indices),
        values: select(cloud.values(), indices),
        ordered: cloud.is_ordered(),
        plane: cloud.plane(),
    }
}

fn add(document: &mut Document, explicit: Option<ObjectId>) -> Result<String, CommandError> {
    let target = selected_target(document, explicit)?;
    let Geometry::PointCloud(cloud) = document.object(target).unwrap().geometry() else {
        unreachable!()
    };
    let mut points = cloud.points().to_vec();
    let old_count = points.len();
    let use_colors = cloud.colors().is_some()
        || document.selected_objects().any(|object| {
            object.id() != target
                && matches!(object.geometry(), Geometry::PointCloud(source) if source.colors().is_some())
        });
    let use_normals = cloud.normals().is_some()
        || document.selected_objects().any(|object| {
            object.id() != target
                && matches!(object.geometry(), Geometry::PointCloud(source) if source.normals().is_some())
        });
    let use_values = cloud.values().is_some()
        || document.selected_objects().any(|object| {
            object.id() != target
                && matches!(object.geometry(), Geometry::PointCloud(source) if source.values().is_some())
        });
    let mut colors = if use_colors {
        Some(if let Some(colors) = cloud.colors() {
            colors.to_vec()
        } else {
            vec![source_display_color(document, document.object(target).unwrap())?; old_count]
        })
    } else {
        None
    };
    let zero_normal = viboceros_geometry::Vector3::try_new(0.0, 0.0, 0.0).unwrap();
    let mut normals = use_normals.then(|| {
        cloud
            .normals()
            .map(<[_]>::to_vec)
            .unwrap_or_else(|| vec![zero_normal; old_count])
    });
    let mut values = use_values.then(|| {
        cloud
            .values()
            .map(<[_]>::to_vec)
            .unwrap_or_else(|| vec![0.0; old_count])
    });
    let mut consumed = Vec::new();
    for object in document.selected_objects() {
        if object.id() == target {
            continue;
        }
        match object.geometry() {
            Geometry::Point(point) => {
                points.push(*point);
                if let Some(colors) = &mut colors {
                    colors.push(source_display_color(document, object)?);
                }
                if let Some(normals) = &mut normals {
                    normals.push(zero_normal);
                }
                if let Some(values) = &mut values {
                    values.push(0.0);
                }
                consumed.push(object.id());
            }
            Geometry::PointCloud(source) => {
                points.extend_from_slice(source.points());
                if let Some(colors) = &mut colors {
                    if let Some(source_colors) = source.colors() {
                        colors.extend_from_slice(source_colors);
                    } else {
                        colors.extend(std::iter::repeat_n(
                            source_display_color(document, object)?,
                            source.points().len(),
                        ));
                    }
                }
                if let Some(normals) = &mut normals {
                    if let Some(source_normals) = source.normals() {
                        normals.extend_from_slice(source_normals);
                    } else {
                        normals.extend(std::iter::repeat_n(zero_normal, source.points().len()));
                    }
                }
                if let Some(values) = &mut values {
                    if let Some(source_values) = source.values() {
                        values.extend_from_slice(source_values);
                    } else {
                        values.extend(std::iter::repeat_n(0.0, source.points().len()));
                    }
                }
                consumed.push(object.id());
            }
            _ => {}
        }
    }
    if points.len() == old_count {
        return Err(CommandError::PointCloudRequiresAddSources);
    }
    let added = points.len() - old_count;
    let ordered = cloud.is_ordered();
    document.replace_object_geometries([(
        target,
        Geometry::PointCloud(PointCloud3::try_with_channels(
            points,
            viboceros_geometry::PointCloudChannels {
                colors,
                normals,
                values,
                ordered,
                plane: cloud.plane(),
            },
        )?),
    )])?;
    document.delete_objects(consumed)?;
    Ok(format!("Added {added} points to point cloud"))
}

fn remove(
    document: &mut Document,
    explicit: Option<ObjectId>,
    indices: &BTreeSet<usize>,
    output_cloud: bool,
) -> Result<String, CommandError> {
    let target = selected_target(document, explicit)?;
    let object = document.object(target).unwrap();
    let Geometry::PointCloud(cloud) = object.geometry() else {
        unreachable!()
    };
    if indices
        .last()
        .is_some_and(|index| *index >= cloud.points().len())
    {
        return Err(CommandError::PointCloudIndexOutOfRange);
    }
    let attributes = object.attributes().clone();
    let removed_indices = indices.iter().copied().collect::<Vec<_>>();
    let retained_indices = (0..cloud.points().len())
        .filter(|index| !indices.contains(index))
        .collect::<Vec<_>>();
    let removed = removed_indices
        .iter()
        .map(|&index| cloud.points()[index])
        .collect::<Vec<_>>();
    let retained = retained_indices
        .iter()
        .map(|&index| cloud.points()[index])
        .collect::<Vec<_>>();
    let removed_channels = channels_for_indices(cloud, &removed_indices);
    let retained_channels = channels_for_indices(cloud, &retained_indices);
    if retained.is_empty() {
        document.delete_object(target)?;
    } else {
        document.replace_object_geometries([(
            target,
            Geometry::PointCloud(PointCloud3::try_with_channels(retained, retained_channels)?),
        )])?;
    }
    if output_cloud {
        document.add_geometry_with_attributes(
            Geometry::PointCloud(PointCloud3::try_with_channels(removed, removed_channels)?),
            attributes,
        )?;
    } else {
        for (index, point) in removed.into_iter().enumerate() {
            let mut output_attributes = attributes.clone();
            if let Some(colors) = &removed_channels.colors {
                let [red, green, blue, _] = colors[index];
                output_attributes = output_attributes
                    .with_object_color(viboceros_document::ColorRgb::new(red, green, blue));
            }
            document.add_geometry_with_attributes(Geometry::Point(point), output_attributes)?;
        }
    }
    Ok(format!("Removed {} points from point cloud", indices.len()))
}

fn convert(
    document: &mut Document,
    postselected: bool,
    use_colors: bool,
) -> Result<String, CommandError> {
    if document.selected_object_count() == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    if !postselected
        && document.objects().any(|o| {
            document.is_selected(o.id()) && matches!(o.geometry(), Geometry::PointCloud(_))
        })
    {
        return Err(CommandError::PointCloudEditActionRequired);
    }
    let inputs = if postselected {
        document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::PointCloudSources.accepts_object(o))
            .collect::<Vec<_>>()
    } else {
        // The document already supplies preselection order: no rank map or
        // sorting of the whole document is needed for large point sets.
        document
            .objects()
            .filter(|o| {
                document.is_selected(o.id())
                    && ObjectSelectionFilter::PointCloudSources.accepts_object(o)
            })
            .collect::<Vec<_>>()
    };
    if inputs.is_empty() {
        return Err(CommandError::PointCloudRequiresSources);
    }
    if inputs.len() == 1 && matches!(inputs[0].geometry(), Geometry::Point(_)) {
        if postselected {
            document.clear_selection();
        }
        return Ok("No point cloud created from a single point".into());
    }
    let mut points = Vec::new();
    let mut colors = Vec::new();
    let mut consumed = Vec::new();
    for object in inputs {
        match object.geometry() {
            Geometry::Point(p) => {
                points.push(*p);
                if use_colors {
                    colors.push(source_display_color(document, object)?);
                }
                consumed.push(object.id());
            }
            Geometry::Mesh(m) => {
                points.extend_from_slice(m.vertices());
                if use_colors {
                    if let Some(vertex_colors) = m.vertex_colors() {
                        colors.extend_from_slice(vertex_colors);
                    } else {
                        colors.extend(std::iter::repeat_n(
                            source_display_color(document, object)?,
                            m.vertices().len(),
                        ));
                    }
                }
            }
            _ => unreachable!("source filter excludes non-point-bearing geometry"),
        }
    }
    let count = points.len();
    let cloud = PointCloud3::try_with_colors(points, use_colors.then_some(colors))?;
    document.add_geometry(Geometry::PointCloud(cloud))?;
    document.delete_objects(consumed)?;
    if postselected {
        document.clear_selection();
    }
    Ok(format!("Created point cloud with {count} points"))
}

#[cfg(test)]
mod tests;
