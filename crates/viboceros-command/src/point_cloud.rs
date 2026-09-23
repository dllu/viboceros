//! PointCloud creation and selected-cloud editing.
use super::*;

const USAGE: &str = "PointCloud [UsePointColors=No] | PointCloud Add [Target=<id>] | PointCloud Remove Indices=<zero-based-list> [Target=<id>] [Output=Points|PointCloud] | PointCloud Hide|Show Indices=<zero-based-list> [Target=<id>]";

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
    SetHidden {
        target: Option<ObjectId>,
        indices: BTreeSet<usize>,
        hidden: bool,
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
            Operation::SetHidden { .. } => return Ok(None),
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

const REDUCE_USAGE: &str = "ReducePointCloud <count>|Percent=<0..100> [Target=<id>]";

pub(super) struct ReducePointCloudCommand;

#[derive(Clone, Copy)]
enum ReductionAmount {
    Count(usize),
    Percent(f64),
}

impl Command for ReducePointCloudCommand {
    fn name(&self) -> &'static str {
        "ReducePointCloud"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if arguments.is_empty() {
            return Ok(Some(reduction_prompt(
                ObjectSelectionWorkflow::ConfirmAfterSelection,
            )));
        }
        let (_, target) = parse_reduction(arguments)?;
        Ok(target.is_none().then_some(reduction_prompt(
            ObjectSelectionWorkflow::OptionsDuringSelection,
        )))
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok((arguments.is_empty()
            && document
                .selected_objects()
                .any(|object| matches!(object.geometry(), Geometry::PointCloud(_))))
        .then_some(reduction_prompt(
            ObjectSelectionWorkflow::ConfirmAfterSelection,
        )))
    }

    fn object_selection_complete(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<bool, CommandError> {
        if !arguments.is_empty() {
            parse_reduction(arguments)?;
        }
        Ok(document
            .selected_objects()
            .filter(|object| matches!(object.geometry(), Geometry::PointCloud(_)))
            .take(2)
            .count()
            == 1)
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (amount, target) = parse_reduction(arguments)?;
        reduce_point_cloud(document, target, amount, &mut rand::rng())
    }
}

fn reduction_prompt(workflow: ObjectSelectionWorkflow) -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command: "ReducePointCloud",
        filter: ObjectSelectionFilter::PointCloud,
        workflow,
        options: vec![],
        menus: vec![],
        choices: vec![],
    }
}

fn parse_reduction(
    arguments: &[&str],
) -> Result<(ReductionAmount, Option<ObjectId>), CommandError> {
    let mut amount = None;
    let mut target = None;
    for argument in arguments {
        if let Some((name, value)) = argument.split_once('=') {
            let name = name.trim_start_matches('_');
            if name.eq_ignore_ascii_case("Target") && target.is_none() {
                target = Some(
                    value
                        .parse()
                        .map_err(|_| CommandError::Usage(REDUCE_USAGE))?,
                );
            } else if name.eq_ignore_ascii_case("Percent") && amount.is_none() {
                let percent = value
                    .parse::<f64>()
                    .map_err(|_| CommandError::Usage(REDUCE_USAGE))?;
                if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                    return Err(CommandError::Usage(REDUCE_USAGE));
                }
                amount = Some(ReductionAmount::Percent(percent));
            } else if name.eq_ignore_ascii_case("Count") && amount.is_none() {
                amount = Some(ReductionAmount::Count(
                    value
                        .parse()
                        .map_err(|_| CommandError::Usage(REDUCE_USAGE))?,
                ));
            } else {
                return Err(CommandError::Usage(REDUCE_USAGE));
            }
        } else if amount.is_none() {
            amount = Some(ReductionAmount::Count(
                argument
                    .parse()
                    .map_err(|_| CommandError::Usage(REDUCE_USAGE))?,
            ));
        } else {
            return Err(CommandError::Usage(REDUCE_USAGE));
        }
    }
    Ok((amount.ok_or(CommandError::Usage(REDUCE_USAGE))?, target))
}

fn reduce_point_cloud<R: rand::Rng + ?Sized>(
    document: &mut Document,
    explicit: Option<ObjectId>,
    amount: ReductionAmount,
    rng: &mut R,
) -> Result<String, CommandError> {
    let target = selected_target(document, explicit)?;
    let Geometry::PointCloud(cloud) = document.object(target).unwrap().geometry() else {
        unreachable!()
    };
    let total = cloud.points().len();
    let remove_count = match amount {
        ReductionAmount::Count(count) => count,
        ReductionAmount::Percent(percent) => ((percent / 100.0) * total as f64).round() as usize,
    };
    if remove_count > total {
        return Err(CommandError::Usage(REDUCE_USAGE));
    }
    if remove_count == 0 {
        return Ok(format!("Removed 0 of {total} point cloud members"));
    }
    if remove_count == total {
        document.delete_object(target)?;
        return Ok(format!(
            "Removed {remove_count} of {total} point cloud members"
        ));
    }

    let retain_count = total - remove_count;
    // Sample the smaller side, then restore stored order before copying channels.
    let retained_indices = if retain_count <= remove_count {
        let mut indices = rand::seq::index::sample(rng, total, retain_count).into_vec();
        indices.sort_unstable();
        indices
    } else {
        let mut removed = rand::seq::index::sample(rng, total, remove_count).into_vec();
        removed.sort_unstable();
        let mut removed = removed.into_iter().peekable();
        (0..total)
            .filter(|index| {
                if removed.peek() == Some(index) {
                    removed.next();
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>()
    };
    let points = retained_indices
        .iter()
        .map(|&index| cloud.points()[index])
        .collect();
    let channels = channels_for_indices(cloud, &retained_indices);
    document.replace_object_geometries([(
        target,
        Geometry::PointCloud(PointCloud3::try_with_channels(points, channels)?),
    )])?;
    Ok(format!(
        "Removed {remove_count} of {total} point cloud members"
    ))
}

fn parse(arguments: &[&str]) -> Result<Operation, CommandError> {
    if arguments.is_empty() {
        return Ok(Operation::Create { use_colors: false });
    }
    let action = arguments[0].trim_start_matches('_');
    if action.eq_ignore_ascii_case("Add")
        || action.eq_ignore_ascii_case("Remove")
        || action.eq_ignore_ascii_case("Hide")
        || action.eq_ignore_ascii_case("Show")
    {
        let remove = action.eq_ignore_ascii_case("Remove");
        let visibility = action.eq_ignore_ascii_case("Hide") || action.eq_ignore_ascii_case("Show");
        let mut target = None;
        let mut indices = None;
        let mut output_cloud = false;
        let mut seen_output = false;
        for argument in &arguments[1..] {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            let name = name.trim_start_matches('_');
            if name.eq_ignore_ascii_case("Target") && target.is_none() {
                target = Some(value.parse().map_err(|_| CommandError::Usage(USAGE))?);
            } else if (remove || visibility)
                && name.eq_ignore_ascii_case("Indices")
                && indices.is_none()
            {
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
        } else if visibility {
            Ok(Operation::SetHidden {
                target,
                indices: indices.ok_or(CommandError::Usage(USAGE))?,
                hidden: action.eq_ignore_ascii_case("Hide"),
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
        Operation::SetHidden {
            target,
            indices,
            hidden,
        } => set_hidden(document, target, &indices, hidden),
    }
}

fn set_hidden(
    document: &mut Document,
    explicit: Option<ObjectId>,
    indices: &BTreeSet<usize>,
    hidden: bool,
) -> Result<String, CommandError> {
    let target = selected_target(document, explicit)?;
    let Geometry::PointCloud(cloud) = document.object(target).unwrap().geometry() else {
        unreachable!()
    };
    if indices
        .last()
        .is_some_and(|index| *index >= cloud.points().len())
    {
        return Err(CommandError::PointCloudIndexOutOfRange);
    }
    let mut flags = cloud
        .hidden()
        .map(<[_]>::to_vec)
        .unwrap_or_else(|| vec![false; cloud.points().len()]);
    for &index in indices {
        flags[index] = hidden;
    }
    document
        .replace_object_geometries([(target, Geometry::PointCloud(cloud.with_hidden(flags)?))])?;
    Ok(format!(
        "{} {} point cloud members",
        if hidden { "Hid" } else { "Revealed" },
        indices.len()
    ))
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
        hidden: select(cloud.hidden(), indices),
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
    let use_hidden = cloud.hidden().is_some()
        || document.selected_objects().any(|object| {
            object.id() != target
                && matches!(object.geometry(), Geometry::PointCloud(source) if source.hidden().is_some())
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
    let mut hidden = use_hidden.then(|| {
        cloud
            .hidden()
            .map(<[_]>::to_vec)
            .unwrap_or_else(|| vec![false; old_count])
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
                if let Some(hidden) = &mut hidden {
                    hidden.push(false);
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
                if let Some(hidden) = &mut hidden {
                    if let Some(source_hidden) = source.hidden() {
                        hidden.extend_from_slice(source_hidden);
                    } else {
                        hidden.extend(std::iter::repeat_n(false, source.points().len()));
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
                hidden,
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
