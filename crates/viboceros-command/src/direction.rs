//! Direction edits shared by scripted and command-first selection.
use super::*;

pub(super) const USAGE: &str =
    "Dir Flip|UReverse|VReverse|SwapUV (or Mode=FlipNormal|FlipU|FlipV|SwapUV) [Faces=All|i,j]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectionEdit {
    Flip,
    ReverseU,
    ReverseV,
    SwapUv,
}

impl DirectionEdit {
    const fn label(self) -> &'static str {
        match self {
            Self::Flip => "Flip",
            Self::ReverseU => "UReverse",
            Self::ReverseV => "VReverse",
            Self::SwapUv => "SwapUV",
        }
    }
}

pub(super) struct DirectionCommand;

impl Command for DirectionCommand {
    fn name(&self) -> &'static str {
        "Dir"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Direction"]
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = Options::parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Dir",
            filter: if options.edit == DirectionEdit::Flip {
                ObjectSelectionFilter::Any
            } else if options.faces.is_some() {
                ObjectSelectionFilter::SurfaceComponents
            } else {
                ObjectSelectionFilter::Surfaces
            },
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        run(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run(document, arguments, true)
    }
}

fn run(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    let options = Options::parse(arguments)?;
    let edit = options.edit;
    let selected = document
        .selected_objects()
        .map(|object| (object.id(), object.geometry()))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }

    let selection_before = document.selected_object_ids().collect::<Vec<_>>();
    let mut replacements = Vec::new();
    for (id, geometry) in selected {
        if let (Some(Some(indices)), Geometry::NurbsSurface(_)) = (&options.faces, geometry)
            && indices.iter().any(|&i| i != 0)
        {
            return Err(CommandError::UnsupportedDirectionEditGeometry);
        }
        if edit == DirectionEdit::Flip {
            if let Some(flipped) = flip::flipped_geometry(geometry, document.tolerance())? {
                replacements.push((id, flipped));
            }
            continue;
        }
        let geometry = match (edit, geometry) {
            (_, Geometry::NurbsSurface(surface)) => edit_brep(
                Brep::try_surface_face(surface.clone(), document.tolerance())?,
                &options,
            )?,
            (_, Geometry::Brep(brep)) => edit_brep(brep.clone(), &options)?,
            _ => return Err(CommandError::UnsupportedDirectionEditGeometry),
        };
        replacements.push((id, geometry));
    }
    replacements.retain(|(id, geometry)| document.object(*id).unwrap().geometry() != geometry);
    if replacements.is_empty() {
        return Ok("Edited 0 object direction(s)".into());
    }
    // Native Dir clears changed inputs on Undo. Preselection is restored
    // live and on Redo; command-first picks stay released in all phases.
    // Preserve skipped peers without expanding any groups.
    let changed = replacements
        .iter()
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    document.select_command_results(
        selection_before
            .iter()
            .copied()
            .filter(|id| !changed.contains(id)),
    )?;
    let count = document.replace_object_geometries(replacements)?;
    if !postselected {
        document.select_command_results(selection_before)?;
    }
    Ok(format!(
        "Edited {count} object direction(s) with {}",
        edit.label()
    ))
}

fn edit_brep(mut brep: Brep, options: &Options) -> Result<Geometry, CommandError> {
    let faces = match &options.faces {
        Some(Some(indices)) => indices.clone(),
        Some(None) => (0..brep.faces().len()).collect(),
        None if brep.faces().len() == 1 => vec![0],
        None => return Err(CommandError::UnsupportedDirectionEditGeometry),
    };
    match options.edit {
        DirectionEdit::ReverseU => brep.try_reverse_face_u(&faces)?,
        DirectionEdit::ReverseV => brep.try_reverse_face_v(&faces)?,
        DirectionEdit::SwapUv => brep.try_swap_face_uv(&faces)?,
        DirectionEdit::Flip => unreachable!(),
    }
    Ok(Geometry::Brep(brep))
}

struct Options {
    edit: DirectionEdit,
    // None: natural single-face source; Some(None): explicit All.
    faces: Option<Option<Vec<usize>>>,
}

impl Options {
    fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        let mut faces = None;
        let mut edit = Vec::new();
        for &argument in arguments {
            if let Some((name, value)) = argument.split_once('=')
                && (option_name_eq(name, "Face") || option_name_eq(name, "Faces"))
            {
                if faces.is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
                faces = Some(if option_name_eq(value, "All") {
                    None
                } else {
                    let indices = value
                        .split(',')
                        .map(|i| i.parse::<usize>().map_err(|_| CommandError::Usage(USAGE)))
                        .collect::<Result<Vec<_>, _>>()?;
                    if indices.iter().copied().collect::<BTreeSet<_>>().len() != indices.len() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    Some(indices)
                });
            } else {
                edit.push(argument);
            }
        }
        let edit = parse_direction_edit(&edit)?;
        if faces.is_some() && edit == DirectionEdit::Flip {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Self { edit, faces })
    }
}

fn parse_direction_edit(arguments: &[&str]) -> Result<DirectionEdit, CommandError> {
    let value = match arguments {
        [] => return Err(CommandError::Usage(USAGE)),
        [value] if !value.contains('=') => *value,
        _ => {
            let (name, value, consumed) = orient_option(arguments, 0, USAGE)?;
            if !option_name_eq(name, "Mode") || consumed != arguments.len() {
                return Err(CommandError::Usage(USAGE));
            }
            value
        }
    }
    .trim_start_matches(['_', '-']);
    if value.eq_ignore_ascii_case("Flip")
        || value.eq_ignore_ascii_case("FlipAll")
        || value.eq_ignore_ascii_case("FlipNormal")
    {
        Ok(DirectionEdit::Flip)
    } else if value.eq_ignore_ascii_case("UReverse")
        || value.eq_ignore_ascii_case("ReverseU")
        || value.eq_ignore_ascii_case("FlipU")
    {
        Ok(DirectionEdit::ReverseU)
    } else if value.eq_ignore_ascii_case("VReverse")
        || value.eq_ignore_ascii_case("ReverseV")
        || value.eq_ignore_ascii_case("FlipV")
    {
        Ok(DirectionEdit::ReverseV)
    } else if value.eq_ignore_ascii_case("SwapUV") {
        Ok(DirectionEdit::SwapUv)
    } else {
        Err(CommandError::Usage(USAGE))
    }
}
