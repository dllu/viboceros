//! Selected-object PointCloud creation; mesh inputs remain in the document.
use super::*;

const USAGE: &str = "PointCloud [UsePointColors=No]";

pub(super) struct PointCloudCommand;

impl Command for PointCloudCommand {
    fn name(&self) -> &'static str {
        "PointCloud"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        convert(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        convert(document, arguments, true)
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::PointCloudSources,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
}

fn parse(arguments: &[&str]) -> Result<(), CommandError> {
    if arguments.is_empty() {
        return Ok(());
    }
    let (name, value, consumed) = orient_option(arguments, 0, USAGE)?;
    require_consumed(arguments, consumed, USAGE)?;
    if !option_name_eq(name, "UsePointColors") {
        return Err(CommandError::Usage(USAGE));
    }
    match parse_yes_no(value) {
        Some(false) => Ok(()),
        Some(true) => Err(CommandError::PointCloudColorsUnsupported),
        None => Err(CommandError::Usage(USAGE)),
    }
}

fn convert(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    parse(arguments)?;
    if document.selected_object_count() == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    if !postselected
        && document.objects().any(|o| {
            document.is_selected(o.id()) && matches!(o.geometry(), Geometry::PointCloud(_))
        })
    {
        return Err(CommandError::PointCloudEditingUnsupported);
    }
    let inputs = if postselected {
        let ranks = document
            .selected_object_ids()
            .enumerate()
            .map(|(rank, id)| (id, rank))
            .collect::<BTreeMap<_, _>>();
        let mut inputs = document
            .objects()
            .filter(|o| {
                ranks.contains_key(&o.id())
                    && ObjectSelectionFilter::PointCloudSources.accepts(o.geometry())
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|o| ranks[&o.id()]);
        inputs
    } else {
        // The document already supplies preselection order: no rank map or
        // sorting of the whole document is needed for large point sets.
        document
            .objects()
            .filter(|o| {
                document.is_selected(o.id())
                    && ObjectSelectionFilter::PointCloudSources.accepts(o.geometry())
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
    let mut consumed = Vec::new();
    for object in inputs {
        match object.geometry() {
            Geometry::Point(p) => {
                points.push(*p);
                consumed.push(object.id());
            }
            Geometry::Mesh(m) => points.extend_from_slice(m.vertices()),
            _ => unreachable!("source filter excludes non-point-bearing geometry"),
        }
    }
    let count = points.len();
    let cloud = PointCloud3::try_new(points)?;
    document.add_geometry(Geometry::PointCloud(cloud))?;
    document.delete_objects(consumed)?;
    if postselected {
        document.clear_selection();
    }
    Ok(format!("Created point cloud with {count} points"))
}

#[cfg(test)]
mod tests;
