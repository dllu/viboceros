//! Conversion choices and selection-dependent confirmation menus.
use super::*;
use viboceros_geometry::SubDNurbsContinuity;

pub(super) const USAGE: &str = "ToNURBS [DeleteInputObjects=Yes|No] [MeshOptions TrimTriangularFaces=Yes|No] [SubDOptions Faces=Packed|Unpacked ExtraordinaryVertex=G0|G1|G1x|G1xx|G2]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Options {
    pub delete_input: bool,
    pub trim_triangles: bool,
    pub packed: bool,
    pub continuity: SubDNurbsContinuity,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            delete_input: false,
            trim_triangles: true,
            packed: true,
            continuity: SubDNurbsContinuity::G1x,
        }
    }
}

#[derive(Default)]
pub(super) struct Kinds {
    pub mesh: bool,
    pub subd: bool,
}
impl Kinds {
    pub fn selected(document: &Document) -> Self {
        let mut result = Self::default();
        for object in document.selected_objects() {
            result.mesh |= matches!(object.geometry(), Geometry::Mesh(_));
            result.subd |= matches!(object.geometry(), Geometry::SubD(_));
        }
        result
    }
    pub fn require(&self, selected: &Self) -> Result<(), CommandError> {
        if (self.mesh && !selected.mesh) || (self.subd && !selected.subd) {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(())
    }
}

const MODES: &[(&str, SubDNurbsContinuity)] = &[
    ("G0", SubDNurbsContinuity::G0),
    ("G1", SubDNurbsContinuity::G1),
    ("G1x", SubDNurbsContinuity::G1x),
    ("G1xx", SubDNurbsContinuity::G1xx),
    ("G2", SubDNurbsContinuity::G2),
];

pub(super) fn prompt(options: Options, kinds: Kinds) -> ObjectSelectionPrompt {
    let mut menus = Vec::new();
    if kinds.mesh {
        menus.push(SelectionOptionMenu {
            name: "MeshOptions",
            options: vec![BooleanSelectionOption {
                name: "TrimTriangularFaces",
                value: options.trim_triangles,
                aliases: &[],
            }],
            choices: vec![],
        });
    }
    if kinds.subd {
        menus.push(SelectionOptionMenu {
            name: "SubDOptions",
            options: vec![],
            choices: vec![
                ChoiceSelectionOption {
                    name: "Faces",
                    value: if options.packed { "Packed" } else { "Unpacked" },
                    choices: &["Packed", "Unpacked"],
                    toggle: None,
                },
                ChoiceSelectionOption {
                    name: "ExtraordinaryVertex",
                    value: MODES.iter().find(|m| m.1 == options.continuity).unwrap().0,
                    choices: &["G0", "G1", "G1x", "G1xx", "G2"],
                    toggle: None,
                },
            ],
        });
    }
    ObjectSelectionPrompt {
        command: "ToNURBS",
        filter: ObjectSelectionFilter::ToNurbs,
        workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        choices: vec![],
        options: vec![BooleanSelectionOption {
            name: "DeleteInputObjects",
            value: options.delete_input,
            aliases: &["DeleteInput"],
        }],
        menus,
    }
}

pub(super) fn parse(
    arguments: &[&str],
    mut options: Options,
) -> Result<(Options, Kinds), CommandError> {
    let mut kinds = Kinds::default();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < arguments.len() {
        if let Some(menu) = ["MeshOptions", "SubDOptions"]
            .into_iter()
            .find(|name| option_name_eq(arguments[i], name))
        {
            if !seen.insert(menu) {
                return Err(CommandError::Usage(USAGE));
            }
            kinds.mesh |= menu == "MeshOptions";
            kinds.subd |= menu == "SubDOptions";
            i += 1;
            continue;
        }
        let (name, value, consumed) = orient_option(arguments, i, USAGE)?;
        let canonical =
            if option_name_eq(name, "DeleteInputObjects") || option_name_eq(name, "DeleteInput") {
                options.delete_input = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                "DeleteInputObjects"
            } else if option_name_eq(name, "TrimTriangularFaces") {
                kinds.mesh = true;
                options.trim_triangles = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                "TrimTriangularFaces"
            } else if option_name_eq(name, "Faces") {
                kinds.subd = true;
                options.packed = if option_name_eq(value, "Packed") {
                    true
                } else if option_name_eq(value, "Unpacked") {
                    false
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
                "Faces"
            } else if option_name_eq(name, "ExtraordinaryVertex") {
                kinds.subd = true;
                options.continuity = MODES
                    .iter()
                    .find(|m| option_name_eq(value, m.0))
                    .ok_or(CommandError::Usage(USAGE))?
                    .1;
                "ExtraordinaryVertex"
            } else {
                return Err(CommandError::Usage(USAGE));
            };
        if !seen.insert(canonical) {
            return Err(CommandError::Usage(USAGE));
        }
        i += consumed;
    }
    Ok((options, kinds))
}
