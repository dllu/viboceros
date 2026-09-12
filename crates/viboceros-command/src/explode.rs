use super::*;

mod summary;
use summary::{ExplodeSummary, PartKind};

pub(super) struct ExplodeCommand;

enum ExplodedParts {
    Lines(Vec<LineSegment>),
    Curves(Vec<viboceros_geometry::CurveSegment3>),
    Points(Vec<Point3>),
    Surfaces(Vec<Brep>),
    Meshes(Vec<TriangleMesh>),
}

impl ExplodedParts {
    fn report(&self) -> (PartKind, usize) {
        match self {
            Self::Lines(parts) => (PartKind::Polyline, parts.len()),
            Self::Curves(parts) => (PartKind::Polycurve, parts.len()),
            Self::Points(parts) => (PartKind::PointCloud, parts.len()),
            Self::Surfaces(parts) => (PartKind::Polysurface, parts.len()),
            Self::Meshes(parts) => (PartKind::Mesh, parts.len()),
        }
    }

    fn into_geometries(self) -> Vec<Geometry> {
        match self {
            Self::Lines(parts) => parts.into_iter().map(Geometry::Line).collect(),
            Self::Curves(parts) => parts
                .into_iter()
                .map(|part| Geometry::from(part.into_curve()))
                .collect(),
            Self::Points(parts) => parts.into_iter().map(Geometry::Point).collect(),
            Self::Surfaces(parts) => parts.into_iter().map(Geometry::Brep).collect(),
            Self::Meshes(parts) => parts.into_iter().map(Geometry::Mesh).collect(),
        }
    }
}

impl Command for ExplodeCommand {
    fn name(&self) -> &'static str {
        "Explode"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["X"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Explode")?;
        let locked_layers = document
            .layers()
            .filter(|layer| layer.is_locked())
            .map(|layer| layer.id())
            .collect::<BTreeSet<_>>();
        let selected = document
            .selected_objects()
            .map(|object| {
                (
                    object.id(),
                    object.geometry(),
                    object.attributes().is_visible()
                        && !object.attributes().is_locked()
                        && !locked_layers.contains(&object.attributes().layer_id()),
                )
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut exploded = Vec::new();
        let mut summary = ExplodeSummary::default();
        let mut unchanged_ids = Vec::new();
        let mut deleted_sources = Vec::new();
        for (id, geometry, delete_source) in &selected {
            let parts = match geometry {
                Geometry::PolyCurve(curve) => {
                    let mut parts = Vec::new();
                    for (index, segment) in curve.segments().iter().enumerate() {
                        let segment = segment.try_reparameterized(curve.segment_domain(index)?)?;
                        if let viboceros_geometry::CurveSegment3::Polyline(polyline) = segment {
                            parts.extend(
                                polyline
                                    .segments()
                                    .map(viboceros_geometry::CurveSegment3::Line),
                            );
                        } else {
                            parts.push(segment);
                        }
                    }
                    parts.reverse();
                    Some(ExplodedParts::Curves(parts))
                }
                Geometry::Polyline(polyline) => {
                    let mut parts = polyline.segments().collect::<Vec<_>>();
                    parts.reverse();
                    Some(ExplodedParts::Lines(parts))
                }
                Geometry::PointCloud(cloud) => {
                    let mut parts = cloud.points().to_vec();
                    parts.reverse();
                    Some(ExplodedParts::Points(parts))
                }
                Geometry::Brep(brep) if brep.faces().len() > 1 => {
                    let mut parts = brep.explode_faces(document.tolerance())?;
                    parts.reverse();
                    Some(ExplodedParts::Surfaces(parts))
                }
                Geometry::Mesh(mesh) => {
                    let mut parts = mesh.explode_pieces();
                    if parts.len() <= 1 {
                        None
                    } else {
                        parts.reverse();
                        Some(ExplodedParts::Meshes(parts))
                    }
                }
                _ => None,
            };
            let Some(parts) = parts else {
                unchanged_ids.push(*id);
                continue;
            };
            let (kind, count) = parts.report();
            summary.record(kind, count)?;
            if *delete_source {
                deleted_sources.push(*id);
            }
            exploded.push((*id, parts));
        }
        if exploded.is_empty() {
            return Err(CommandError::NoExplodableObjects);
        }
        let unchanged_count = unchanged_ids.len();
        let pieces = exploded.into_iter().flat_map(|(source, parts)| {
            parts
                .into_geometries()
                .into_iter()
                .map(move |geometry| (source, geometry))
        });
        // Copy while restricted sources remain selected/editable, then consume
        // source selection before recording their deletion (Rhino's Explode
        // history policy). Fresh pieces inherit attributes and ordered groups.
        let selected_result_ids = document.copy_object_pieces_into_source_groups(pieces)?;
        document.select_command_results(unchanged_ids.iter().copied())?;
        document.delete_objects(deleted_sources)?;
        // Retained restricted sources stay unselected, and overlapping groups
        // must not pull untouched peers into the output selection.
        document.select_command_results(unchanged_ids.into_iter().chain(selected_result_ids))?;
        Ok(summary.message(unchanged_count))
    }
}
