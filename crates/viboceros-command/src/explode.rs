use super::*;

pub(super) struct ExplodeCommand;

enum ExplodedParts {
    Lines(Vec<LineSegment>),
    Curves(Vec<viboceros_geometry::CurveSegment3>),
    Points(Vec<Point3>),
    Surfaces(Vec<Brep>),
    Meshes(Vec<TriangleMesh>),
}

impl ExplodedParts {
    fn output_count(&self) -> usize {
        match self {
            Self::Lines(parts) => parts.len(),
            Self::Curves(parts) => parts.len(),
            Self::Points(parts) => parts.len(),
            Self::Surfaces(parts) => parts.len(),
            Self::Meshes(parts) => parts.len(),
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
        let mut output_count = 0_usize;
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
                continue;
            };
            output_count = output_count
                .checked_add(parts.output_count())
                .filter(|count| *count <= MAX_SPAN_OUTPUT_OBJECTS)
                .ok_or_else(|| too_many_span_outputs("Explode"))?;
            exploded.push((*id, parts, *delete_source));
        }
        if exploded.is_empty() {
            return Err(CommandError::NoExplodableObjects);
        }
        let exploded_ids = exploded
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<BTreeSet<_>>();
        let unchanged_ids = selected
            .iter()
            .filter(|(id, _, _)| !exploded_ids.contains(id))
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        let polyline_count = exploded
            .iter()
            .filter(|(_, parts, _)| matches!(parts, ExplodedParts::Lines(_)))
            .count();
        let point_cloud_count = exploded
            .iter()
            .filter(|(_, parts, _)| matches!(parts, ExplodedParts::Points(_)))
            .count();
        let polycurve_count = exploded
            .iter()
            .filter(|(_, parts, _)| matches!(parts, ExplodedParts::Curves(_)))
            .count();
        let curve_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Curves(curves) => curves.len(),
                _ => 0,
            })
            .sum::<usize>();
        let polysurface_count = exploded
            .iter()
            .filter(|(_, parts, _)| matches!(parts, ExplodedParts::Surfaces(_)))
            .count();
        let mesh_count = exploded
            .iter()
            .filter(|(_, parts, _)| matches!(parts, ExplodedParts::Meshes(_)))
            .count();
        let line_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Lines(lines) => lines.len(),
                ExplodedParts::Points(_)
                | ExplodedParts::Curves(_)
                | ExplodedParts::Surfaces(_)
                | ExplodedParts::Meshes(_) => 0,
            })
            .sum::<usize>();
        let point_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Lines(_)
                | ExplodedParts::Curves(_)
                | ExplodedParts::Surfaces(_)
                | ExplodedParts::Meshes(_) => 0,
                ExplodedParts::Points(points) => points.len(),
            })
            .sum::<usize>();
        let surface_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Surfaces(surfaces) => surfaces.len(),
                ExplodedParts::Lines(_)
                | ExplodedParts::Curves(_)
                | ExplodedParts::Points(_)
                | ExplodedParts::Meshes(_) => 0,
            })
            .sum::<usize>();
        let mesh_part_count = exploded
            .iter()
            .map(|(_, parts, _)| match parts {
                ExplodedParts::Meshes(meshes) => meshes.len(),
                ExplodedParts::Lines(_)
                | ExplodedParts::Curves(_)
                | ExplodedParts::Points(_)
                | ExplodedParts::Surfaces(_) => 0,
            })
            .sum::<usize>();
        let unchanged_count = selected.len() - exploded_ids.len();
        let deleted_sources = exploded
            .iter()
            .filter(|(_, _, delete)| *delete)
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        let pieces = exploded.into_iter().flat_map(|(source, parts, _)| {
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
        let mut summaries = Vec::new();
        if polycurve_count > 0 {
            summaries.push(format!(
                "{polycurve_count} polycurve(s) into {curve_count} curve(s)"
            ));
        }
        if polyline_count > 0 {
            summaries.push(format!(
                "{polyline_count} polyline(s) into {line_count} line(s)"
            ));
        }
        if point_cloud_count > 0 {
            summaries.push(format!(
                "{point_cloud_count} point cloud(s) into {point_count} point(s)"
            ));
        }
        if polysurface_count > 0 {
            summaries.push(format!(
                "{polysurface_count} polysurface(s) into {surface_count} surface(s)"
            ));
        }
        if mesh_count > 0 {
            summaries.push(format!(
                "{mesh_count} mesh(es) into {mesh_part_count} part(s)"
            ));
        }
        let last = summaries
            .pop()
            .expect("at least one selected object was exploded");
        let summary = if summaries.is_empty() {
            last
        } else {
            format!("{} and {last}", summaries.join(", "))
        };
        Ok(format!(
            "Exploded {summary}; {unchanged_count} object(s) unchanged"
        ))
    }
}
