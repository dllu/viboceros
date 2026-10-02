//! Exact component hole removal, staged before any document mutation.
use super::*;
use viboceros_document::ReplacementHistory;
use viboceros_geometry::{BrepLoopType, BrepTrimType, PolyCurve3};

#[cfg(test)]
mod tests;

const USAGE: &str = "UntrimHoles object-id component-index [All=Yes|No] [MaximumEdgeLength=number] [KeepTrimObjects=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UntrimHolesOptions {
    pub all: bool,
    pub maximum_edge_length: Real,
    pub keep_trim_objects: bool,
}

impl UntrimHolesOptions {
    pub fn validate(self) -> Result<(), CommandError> {
        if !self.maximum_edge_length.is_finite() || self.maximum_edge_length < 0. {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(())
    }

    /// Parses an entire option tail atomically. Zero disables length filtering.
    pub fn updated(self, input: &str) -> Result<Self, CommandError> {
        parse_options(&input.split_whitespace().collect::<Vec<_>>(), self)
    }

    pub fn command_line(self) -> String {
        format!(
            "UntrimHoles All={} MaximumEdgeLength={} KeepTrimObjects={}",
            if self.all { "Yes" } else { "No" },
            self.maximum_edge_length,
            if self.keep_trim_objects { "Yes" } else { "No" }
        )
    }
}

fn parse_options(
    arguments: &[&str],
    mut options: UntrimHolesOptions,
) -> Result<UntrimHolesOptions, CommandError> {
    let mut index = 0;
    let mut seen = BTreeSet::new();
    while index < arguments.len() {
        let (name, value, consumed) = orient_option(arguments, index, USAGE)?;
        if !seen.insert(name.trim_start_matches('_').to_ascii_lowercase()) {
            return Err(CommandError::Usage(USAGE));
        }
        if option_name_eq(name, "All") {
            options.all = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(name, "KeepTrimObjects") {
            options.keep_trim_objects = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(name, "MaximumEdgeLength") {
            options.maximum_edge_length = value
                .trim_start_matches('_')
                .parse()
                .map_err(|_| CommandError::Usage(USAGE))?;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    options.validate()?;
    Ok(options)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UntrimHolesComponent {
    Edge(usize),
    Face(usize),
}

/// Read-only source snapshot and completely validated replacement/retained objects.
#[derive(Clone, Debug)]
pub struct UntrimHolesSelection {
    object: ObjectId,
    source: Geometry,
    tolerance: Tolerance,
    replacement: Option<Brep>,
    retained: Vec<Geometry>,
    openings: usize,
    walls: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UntrimHolesResult {
    pub changed: bool,
    pub retained: Vec<ObjectId>,
    pub removed_openings: usize,
    pub removed_faces: usize,
}

impl UntrimHolesSelection {
    pub fn prepare(
        document: &Document,
        object: ObjectId,
        component: UntrimHolesComponent,
        options: UntrimHolesOptions,
    ) -> Result<Self, CommandError> {
        options.validate()?;
        if !document.is_object_selectable(object) {
            return Err(CommandError::UntrimHolesUnavailable);
        }
        let source = document
            .object(object)
            .ok_or(CommandError::UntrimHolesUnavailable)?
            .geometry()
            .clone();
        let tolerance = document.tolerance();
        let brep = match &source {
            Geometry::Brep(brep) => brep.clone(),
            Geometry::NurbsSurface(surface) => {
                Brep::try_surface_face_with_native_edge_parameters(surface.clone(), tolerance)?
            }
            _ => return Err(CommandError::UntrimHolesUnavailable),
        };
        match component {
            UntrimHolesComponent::Face(face) if options.all && face < brep.faces().len() => {}
            UntrimHolesComponent::Edge(edge) if !options.all && edge < brep.edges().len() => {}
            _ => return Err(CommandError::UntrimHolesUnavailable),
        }
        let mut holes = Vec::new();
        for (face_index, face) in brep.faces().iter().enumerate() {
            for (loop_index, boundary) in face.loops().iter().enumerate() {
                if boundary.loop_type() != BrepLoopType::Inner {
                    continue;
                }
                let picked = match component {
                    UntrimHolesComponent::Face(index) => face_index == index,
                    UntrimHolesComponent::Edge(index) => boundary
                        .trims()
                        .iter()
                        .any(|trim| trim.edge() == Some(index)),
                };
                if !picked {
                    continue;
                }
                if options.maximum_edge_length != 0. {
                    // Native UntrimHoles filters the entire hole perimeter,
                    // including when one short edge is the selected component.
                    let edges = boundary
                        .trims()
                        .iter()
                        .filter_map(|trim| trim.edge())
                        .collect::<BTreeSet<_>>();
                    let mut length = 0.;
                    for edge in edges {
                        length += brep.edges()[edge].curve().length(tolerance)?;
                    }
                    if !length.is_finite() || length > options.maximum_edge_length {
                        continue;
                    }
                }
                holes.push((face_index, loop_index));
            }
        }
        let removal = brep.try_remove_holes_with_topology(&holes, tolerance)?;
        let mut retained = Vec::new();
        let mut openings = 0;
        let mut walls = 0;
        let replacement = if let Some(removal) = removal {
            openings = removal.removed_openings().len();
            walls = removal.removed_faces().len();
            if options.keep_trim_objects {
                let mut wall_edges = BTreeSet::new();
                for &face in removal.removed_faces() {
                    wall_edges.extend(
                        brep.faces()[face]
                            .loops()
                            .iter()
                            .flat_map(|boundary| boundary.trims())
                            .filter_map(|trim| trim.edge()),
                    );
                }
                for &(face, boundary) in removal.removed_openings() {
                    // Joined openings are represented by their detached walls,
                    // without additional coincident trim curves.
                    if !brep.faces()[face].loops()[boundary]
                        .trims()
                        .iter()
                        .any(|trim| trim.edge().is_some_and(|edge| wall_edges.contains(&edge)))
                    {
                        retained.extend(retained_hole_geometry(&brep, face, boundary)?);
                    }
                }
                if walls != 0 {
                    let detached = brep.sub_brep(removal.removed_faces(), tolerance)?;
                    for component in detached.edge_connected_face_components() {
                        retained.push(Geometry::Brep(detached.sub_brep(&component, tolerance)?));
                    }
                }
                if retained.len() > MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs("UntrimHoles"));
                }
            }
            Some(removal.into_brep())
        } else {
            None
        };
        Ok(Self {
            object,
            source,
            tolerance,
            replacement,
            retained,
            openings,
            walls,
        })
    }

    pub fn object(&self) -> ObjectId {
        self.object
    }
    pub fn changes_geometry(&self) -> bool {
        self.replacement.is_some()
    }

    pub fn validate_source(&self, document: &Document) -> Result<(), CommandError> {
        if !document.is_object_selectable(self.object)
            || document.tolerance() != self.tolerance
            || document
                .object(self.object)
                .is_none_or(|object| object.geometry() != &self.source)
        {
            return Err(CommandError::UntrimHolesStale);
        }
        Ok(())
    }

    /// Commits one accepted component immediately, in one atomic Undo record.
    pub fn commit(&self, document: &mut Document) -> Result<UntrimHolesResult, CommandError> {
        run_command_transaction(document, "UntrimHoles", |document| self.apply(document))
    }

    fn apply(&self, document: &mut Document) -> Result<UntrimHolesResult, CommandError> {
        self.validate_source(document)?;
        let mut result = UntrimHolesResult {
            changed: false,
            retained: vec![],
            removed_openings: self.openings,
            removed_faces: self.walls,
        };
        let Some(replacement) = &self.replacement else {
            return Ok(result);
        };
        document.clear_selection();
        for geometry in &self.retained {
            result
                .retained
                .push(document.add_geometry(geometry.clone())?);
        }
        document.replace_object_geometries_with_history(
            [(self.object, Geometry::Brep(replacement.clone()))],
            ReplacementHistory::EveryReplacement,
        )?;
        document
            .move_objects_to_end_in_order(result.retained.iter().copied().chain([self.object]))?;
        result.changed = true;
        Ok(result)
    }
}

fn retained_hole_geometry(
    brep: &Brep,
    face_index: usize,
    loop_index: usize,
) -> Result<Vec<Geometry>, GeometryError> {
    let face = &brep.faces()[face_index];
    let boundary = &face.loops()[loop_index];
    let mut result = Vec::new();
    for mut component in brep.loop_boundary_curve_components(face_index, loop_index)? {
        // Hole retention follows the original UV trim winding even when the
        // face's normal is reversed. Boundary chaining follows face normals.
        if face.is_reversed() {
            component = component
                .into_iter()
                .rev()
                .map(|curve| curve.reversed())
                .collect::<Result<_, _>>()?;
        }
        if component.len() == 1 {
            result.push(Geometry::NurbsCurve(component.pop().unwrap()));
            continue;
        }
        // The hole command keeps the first trim's seam and the spatial edge
        // intervals. Border extraction has a different seam/domain policy.
        for trim in boundary.trims().iter().filter(|trim| {
            matches!(
                trim.trim_type(),
                BrepTrimType::Boundary | BrepTrimType::Mated
            )
        }) {
            let curve = brep.edges()[trim.edge().expect("validated non-seam trim")].curve();
            let seam = curve.evaluate(if trim.is_reversed_3d() {
                *curve.domain().end()
            } else {
                *curve.domain().start()
            })?;
            if let Some(start) = component.iter().position(|curve| {
                curve
                    .evaluate(*curve.domain().start())
                    .is_ok_and(|point| point == seam)
            }) {
                component.rotate_left(start);
                break;
            }
        }
        result.push(Geometry::NurbsCurve(
            PolyCurve3::try_new(component)?.to_nurbs()?,
        ));
    }
    Ok(result)
}

#[derive(Default)]
pub(super) struct UntrimHolesCommand {
    options: remembered::Remembered<UntrimHolesOptions>,
}

impl Command for UntrimHolesCommand {
    fn name(&self) -> &'static str {
        "UntrimHoles"
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.options
            .set(parse_options(arguments, self.options.get())?);
        Ok(())
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.len() < 2 {
            return Err(CommandError::Usage(USAGE));
        }
        let object = arguments[0]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let index = arguments[1]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let options = parse_options(&arguments[2..], self.options.get())?;
        let component = if options.all {
            UntrimHolesComponent::Face(index)
        } else {
            UntrimHolesComponent::Edge(index)
        };
        let result =
            UntrimHolesSelection::prepare(document, object, component, options)?.apply(document)?;
        self.options.set(options);
        Ok(format!(
            "Removed {} hole opening(s) and {} wall face(s); retained {} trim object(s)",
            result.removed_openings,
            result.removed_faces,
            result.retained.len()
        ))
    }
}
