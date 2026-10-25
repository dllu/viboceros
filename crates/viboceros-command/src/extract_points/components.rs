//! Validate typed components and resolve borrowed topology once per owner.
use super::*;

mod brep;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Vertex,
    Edge,
    Face,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub object: ObjectId,
    pub kind: Kind,
    pub index: usize,
}
pub(super) fn parse(value: &str) -> Result<Target, CommandError> {
    let parts = value.split(',').collect::<Vec<_>>();
    let [object, kind, index] = parts.as_slice() else {
        return Err(CommandError::Usage(USAGE));
    };
    let kind = if kind.eq_ignore_ascii_case("Vertex") {
        Kind::Vertex
    } else if kind.eq_ignore_ascii_case("Edge") {
        Kind::Edge
    } else if kind.eq_ignore_ascii_case("Face") {
        Kind::Face
    } else {
        return Err(CommandError::Usage(USAGE));
    };
    Ok(Target {
        object: object.parse().map_err(|_| CommandError::Usage(USAGE))?,
        kind,
        index: index.parse().map_err(|_| CommandError::Usage(USAGE))?,
    })
}

pub(super) fn resolve(
    document: &Document,
    options: &ExtractPointOptions,
    grip_owners: &BTreeSet<ObjectId>,
) -> Result<BTreeMap<ObjectId, Vec<Point3>>, CommandError> {
    let mut grouped = BTreeMap::<ObjectId, Vec<(Kind, usize)>>::new();
    let mut seen = BTreeSet::new();
    for target in &options.components {
        if seen.insert((target.object, target.kind, target.index)) {
            grouped
                .entry(target.object)
                .or_default()
                .push((target.kind, target.index));
        }
    }
    let mut resolved = BTreeMap::new();
    for (id, mut picks) in grouped {
        picks.sort();
        if !document.is_object_selectable(id) {
            return Err(CommandError::ExtractPointComponentUnavailable);
        }
        let object = document
            .object(id)
            .ok_or(CommandError::ExtractPointComponentUnavailable)?;
        let mut points = Vec::new();
        match object.geometry() {
            Geometry::Brep(brep) => {
                if grip_owners.contains(&id) {
                    brep::check_picks(brep, &picks)?;
                    continue;
                }
                points = brep::resolve(brep, &picks)?;
            }
            Geometry::NurbsSurface(surface) => {
                // Existing surface component picking uses the natural B-rep
                // boundary ordinals. Construct that topology only for edges.
                if picks.iter().all(|p| *p == (Kind::Face, 0)) {
                    if grip_owners.contains(&id) {
                        continue;
                    }
                    if surface.grip_dimensions().into_iter().product::<usize>()
                        > MAX_EXTRACTED_POINTS
                    {
                        return Err(CommandError::TooManyExtractedPoints {
                            maximum: MAX_EXTRACTED_POINTS,
                        });
                    }
                    brep::append_surface(&mut points, surface);
                } else {
                    let brep = Brep::try_surface_face(surface.clone(), document.tolerance())?;
                    if grip_owners.contains(&id) {
                        brep::check_picks(&brep, &picks)?;
                        continue;
                    }
                    points = brep::resolve(&brep, &picks)?;
                }
            }
            Geometry::Mesh(mesh) => {
                let vertices = picks
                    .iter()
                    .any(|p| p.0 == Kind::Vertex)
                    .then(|| mesh.topology_vertex_points());
                let edges = picks
                    .iter()
                    .any(|p| p.0 == Kind::Edge)
                    .then(|| mesh.topology_edge_points());
                let mut unique = BTreeMap::new();
                for &(kind, index) in &picks {
                    let coordinates = match kind {
                        Kind::Vertex => vec![
                            *vertices
                                .as_ref()
                                .unwrap()
                                .get(index)
                                .ok_or(CommandError::ExtractPointComponentUnavailable)?,
                        ],
                        Kind::Edge => edges
                            .as_ref()
                            .unwrap()
                            .get(index)
                            .ok_or(CommandError::ExtractPointComponentUnavailable)?
                            .to_vec(),
                        Kind::Face => mesh
                            .faces()
                            .get(index)
                            .ok_or(CommandError::ExtractPointComponentUnavailable)?
                            .indices()
                            .iter()
                            .map(|&v| mesh.vertices()[v as usize])
                            .collect(),
                    };
                    for point in coordinates {
                        unique.insert(
                            point
                                .to_array()
                                .map(|x| if x == 0. { 0 } else { x.to_bits() }),
                            point,
                        );
                        if unique.len() > MAX_EXTRACTED_POINTS {
                            return Err(CommandError::TooManyExtractedPoints {
                                maximum: MAX_EXTRACTED_POINTS,
                            });
                        }
                    }
                }
                points.extend(unique.into_values());
                points.sort_by(|a, b| a.to_array().partial_cmp(&b.to_array()).unwrap());
            }
            Geometry::SubD(subd) => {
                // Check every ordinal before the potentially expensive edge fit.
                for &(kind, index) in &picks {
                    let count = match kind {
                        Kind::Vertex => subd.control_points().len(),
                        Kind::Edge => subd.edges().len(),
                        Kind::Face => subd.faces().len(),
                    };
                    if index >= count {
                        return Err(CommandError::ExtractPointComponentUnavailable);
                    }
                }
                if grip_owners.contains(&id) {
                    continue;
                }
                let edges = picks
                    .iter()
                    .filter(|p| p.0 == Kind::Edge)
                    .map(|p| p.1)
                    .collect::<Vec<_>>();
                let curves = if options.control_net(document) || edges.is_empty() {
                    Vec::new()
                } else {
                    subd.edge_nurbs_curves(&edges)?
                };
                let mut curves = curves.into_iter();
                let vertices = if options.control_net(document) {
                    subd.control_points()
                } else {
                    subd.surface_points()
                };
                for &(kind, index) in &picks {
                    let coordinates = match kind {
                        Kind::Vertex => vec![vertices[index]],
                        Kind::Face => subd.faces()[index]
                            .iter()
                            .map(|&v| vertices[v as usize])
                            .collect(),
                        Kind::Edge if options.control_net(document) => {
                            subd.edges()[index].map(|v| vertices[v as usize]).to_vec()
                        }
                        Kind::Edge => curves
                            .next()
                            .unwrap()
                            .control_points()
                            .iter()
                            .map(|p| p.point())
                            .collect(),
                    };
                    if coordinates.len() > MAX_EXTRACTED_POINTS.saturating_sub(points.len()) {
                        return Err(CommandError::TooManyExtractedPoints {
                            maximum: MAX_EXTRACTED_POINTS,
                        });
                    }
                    points.extend(coordinates);
                }
            }
            _ => return Err(CommandError::ExtractPointComponentUnavailable),
        }
        if !grip_owners.contains(&id) {
            resolved.insert(id, points);
        }
    }
    Ok(resolved)
}

pub(super) fn whole_brep_points(brep: &Brep) -> Result<Vec<Point3>, CommandError> {
    let picks = (0..brep.faces().len())
        .map(|i| (Kind::Face, i))
        .collect::<Vec<_>>();
    brep::resolve(brep, &picks)
}
