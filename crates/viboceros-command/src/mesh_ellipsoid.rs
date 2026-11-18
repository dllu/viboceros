//! MeshEllipsoid construction and topology settings shared by scripts and the viewport.

use crate::{
    Command, CommandContext, CommandError, CommandRegistry, DEFAULT_MESH_ELLIPSOID_FACE_COUNT,
    ellipsoid::{self, Definition},
    option_name_eq, parse_mesh_cap_face_style,
    remembered::Remembered,
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    GeometryError, MAX_MESH_ELLIPSOID_FACES, MeshCapFaceStyle, MeshEllipsoidOptions,
};

const USAGE: &str =
    "MeshEllipsoid [VerticalFaces=n AroundFaces=n CapFaceStyle=Tri|Quad] ellipsoid-input";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Preferences {
    pub topology: MeshEllipsoidOptions,
    pub mark_foci: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            topology: MeshEllipsoidOptions {
                vertical_count: DEFAULT_MESH_ELLIPSOID_FACE_COUNT,
                around_count: DEFAULT_MESH_ELLIPSOID_FACE_COUNT,
                cap_style: MeshCapFaceStyle::Triangles,
            },
            mark_foci: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyOption {
    VerticalFaces,
    AroundFaces,
    CapFaceStyle,
}

impl TopologyOption {
    pub fn parse(word: &str) -> Option<(Self, Option<&str>)> {
        let word = word.trim();
        let (name, value) = word
            .split_once('=')
            .or_else(|| word.split_once(char::is_whitespace))
            .map_or((word, None), |(name, value)| {
                (name.trim(), Some(value.trim()))
            });
        [Self::VerticalFaces, Self::AroundFaces, Self::CapFaceStyle]
            .into_iter()
            .find(|option| option_name_eq(name, option.name()))
            .map(|option| (option, value))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::VerticalFaces => "VerticalFaces",
            Self::AroundFaces => "AroundFaces",
            Self::CapFaceStyle => "CapFaceStyle",
        }
    }

    pub fn assign(
        self,
        topology: &mut MeshEllipsoidOptions,
        value: Option<&str>,
    ) -> Result<(), CommandError> {
        let value = value.map(|value| value.trim_start_matches('_'));
        match self {
            Self::VerticalFaces => {
                let value = value.ok_or(CommandError::Usage(USAGE))?;
                topology.vertical_count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 2)
                    .ok_or_else(|| {
                        CommandError::InvalidMeshEllipsoidVerticalFaceCount(value.into())
                    })?;
            }
            Self::AroundFaces => {
                let value = value.ok_or(CommandError::Usage(USAGE))?;
                topology.around_count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 3)
                    .ok_or_else(|| {
                        CommandError::InvalidMeshEllipsoidAroundFaceCount(value.into())
                    })?;
            }
            Self::CapFaceStyle => {
                topology.cap_style = match value {
                    Some(value) => {
                        parse_mesh_cap_face_style(value).ok_or(CommandError::Usage(USAGE))?
                    }
                    None if topology.cap_style == MeshCapFaceStyle::Triangles => {
                        MeshCapFaceStyle::Quadrilaterals
                    }
                    None => MeshCapFaceStyle::Triangles,
                };
            }
        }
        Ok(())
    }
}

pub fn face_count(topology: MeshEllipsoidOptions) -> Option<usize> {
    let quad_caps = topology.cap_style == MeshCapFaceStyle::Quadrilaterals
        && topology.around_count.is_multiple_of(2);
    topology
        .vertical_count
        .checked_sub(usize::from(quad_caps))?
        .checked_mul(topology.around_count)
}

pub fn validate_topology(topology: MeshEllipsoidOptions) -> Result<(), GeometryError> {
    if topology.vertical_count < 2 || topology.around_count < 3 {
        return Err(GeometryError::InvalidMeshEllipsoidFaceCount {
            vertical_count: topology.vertical_count,
            around_count: topology.around_count,
        });
    }
    if face_count(topology).is_none_or(|count| count > MAX_MESH_ELLIPSOID_FACES) {
        return Err(GeometryError::TooManyMeshFaces);
    }
    Ok(())
}

pub(super) struct MeshEllipsoidCommand(pub(super) Arc<Remembered<Preferences>>);

impl CommandRegistry {
    pub fn mesh_ellipsoid_preferences(&self) -> Preferences {
        self.mesh_ellipsoid_preferences.get()
    }

    pub fn remember_mesh_ellipsoid_preferences(&self, value: Preferences) {
        self.mesh_ellipsoid_preferences.set(value);
    }
}

impl Command for MeshEllipsoidCommand {
    fn name(&self) -> &'static str {
        "MeshEllipsoid"
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let mut preferences = self.0.get();
        let construction = parse_options(args, &mut preferences.topology)?;
        let (definition, release, mark_foci) = ellipsoid::parse_definition(
            doc,
            &construction,
            context.construction_plane,
            preferences.mark_foci,
        )?;
        let mesh = definition.mesh(preferences.topology, doc.tolerance())?;
        let id = ellipsoid::add_geometry(doc, Geometry::Mesh(mesh), definition.foci, release)?;
        preferences.mark_foci = mark_foci;
        self.0.set(preferences);
        Ok(message(id, preferences.topology))
    }
}

fn parse_options<'a>(
    args: &[&'a str],
    topology: &mut MeshEllipsoidOptions,
) -> Result<Vec<&'a str>, CommandError> {
    let mut construction = Vec::with_capacity(args.len());
    let mut remaining = args.iter().copied();
    let mut started = false;
    let mut seen_suffix = [false; 3];
    while let Some(word) = remaining.next() {
        if let Some((option, value)) = TopologyOption::parse(word) {
            // Legacy complete commands accept one assignment of each setting
            // after the geometry. At the initial menu the same setting can be
            // changed more than once, for example before and after editing caps.
            if started && std::mem::replace(&mut seen_suffix[option as usize], true) {
                return Err(CommandError::Usage(USAGE));
            }
            if option == TopologyOption::CapFaceStyle {
                if !started && !topology.around_count.is_multiple_of(2) {
                    return Err(CommandError::Usage(USAGE));
                }
                option.assign(topology, value)?;
            } else {
                option.assign(topology, value.or_else(|| remaining.next()))?;
            }
        } else {
            started |= ![
                "Radius",
                "Default",
                "Corner",
                "Diameter",
                "Vertical",
                "FromFoci",
                "AroundCurve",
            ]
            .iter()
            .any(|mode| option_name_eq(word, mode));
            construction.push(word);
        }
    }
    validate_topology(*topology)?;
    Ok(construction)
}

pub fn commit(
    doc: &mut Document,
    definition: Definition,
    topology: MeshEllipsoidOptions,
    release: Option<ObjectId>,
) -> Result<String, CommandError> {
    let mesh = definition.mesh(topology, doc.tolerance())?;
    let id = ellipsoid::commit_geometry(
        doc,
        "MeshEllipsoid",
        Geometry::Mesh(mesh),
        definition.foci,
        release,
    )?;
    Ok(message(id, topology))
}

fn message(id: ObjectId, topology: MeshEllipsoidOptions) -> String {
    format!(
        "Added mesh ellipsoid {id} ({} around × {} vertical faces)",
        topology.around_count, topology.vertical_count
    )
}

#[cfg(test)]
mod tests;
