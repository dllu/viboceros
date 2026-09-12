//! Geometry-independent STEP assembly placement and import diagnostics.
use super::{StepError, StepImportReport, Table, record_relationship_report};
use monstertruck::core::cgmath64::{Matrix4, SquareMatrix};
use monstertruck::step::load::step_p21::{ast::Name, tables::PlaceHolder};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq)]
pub(super) struct ShapeInstance {
    pub shape_id: u64,
    pub transform: Matrix4,
    pub name: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(super) struct InstancePlan {
    pub instances: Vec<ShapeInstance>,
    pub report: StepImportReport,
}

/// Resolve placements without loading curves, surfaces, or display meshes.
pub(super) fn build(table: &Table) -> Result<InstancePlan, StepError> {
    let mut report = StepImportReport {
        swallowed_entity_count: table.entity_report.total(),
        ..Default::default()
    };
    let mut instances = Vec::new();
    let mut placed_shapes = BTreeSet::new();

    match table.step_assy() {
        Ok(assembly) => {
            for top in assembly.top_nodes() {
                for path in assembly.paths_iter(top.index()) {
                    let node = path.terminal_node();
                    let transform = path.edges().iter().try_fold(
                        Matrix4::identity(),
                        |accumulated, edge| {
                            Matrix4::try_from(edge.matrix())
                                .map(|matrix| accumulated * matrix)
                                .map_err(|error| {
                                    StepError::InvalidAssemblyTransform(error.to_string())
                                })
                        },
                    )?;
                    let mut shape_ids = node.shape().iter().copied().collect::<BTreeSet<_>>();
                    if let Some(representation) = table.shape_representation_of_node(node.entity())
                    {
                        let (_, relationship_report) =
                            table.solids_via_shape_relationship(representation);
                        record_relationship_report(&mut report, &relationship_report);
                        let (related_shapes, skipped) =
                            related_supported_shapes(table, representation);
                        shape_ids.extend(related_shapes);
                        report.skipped_representation_item_count += skipped;
                    }
                    let name = path
                        .edges()
                        .last()
                        .and_then(|edge| nonempty_name(&edge.attributes().name))
                        .or_else(|| nonempty_name(&node.attributes().name));
                    for shape_id in shape_ids {
                        if table.manifold_solid_brep.contains_key(&shape_id)
                            || table.shell_based_surface_model.contains_key(&shape_id)
                        {
                            placed_shapes.insert(shape_id);
                            instances.push(ShapeInstance {
                                shape_id,
                                transform,
                                name: name.clone(),
                            });
                        } else if !is_placement_item(table, shape_id) {
                            report.skipped_representation_item_count += 1;
                        }
                    }
                }
            }
        }
        Err(error) => {
            report.assembly_warning = Some(error.to_string());
        }
    }

    // Assembly construction traverses hash-backed source tables. Stabilize the
    // observable occurrence sequence across independent parses. Identical keys
    // represent indistinguishable occurrences and remain separate entries.
    instances.sort_by(|left, right| {
        left.shape_id
            .cmp(&right.shape_id)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| {
                (0..4)
                    .flat_map(|column| (0..4).map(move |row| (column, row)))
                    .map(|(column, row)| {
                        left.transform[column][row].total_cmp(&right.transform[column][row])
                    })
                    .find(|order| !order.is_eq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut supported_shape_ids = table
        .manifold_solid_brep
        .keys()
        .chain(table.shell_based_surface_model.keys())
        .copied()
        .collect::<Vec<_>>();
    supported_shape_ids.sort_unstable();
    supported_shape_ids.dedup();
    for shape_id in supported_shape_ids {
        if placed_shapes.contains(&shape_id) {
            continue;
        }
        report.unplaced_shape_count += 1;
        instances.push(ShapeInstance {
            shape_id,
            transform: Matrix4::identity(),
            name: None,
        });
    }

    Ok(InstancePlan { instances, report })
}

fn related_supported_shapes(table: &Table, source_representation: u64) -> (BTreeSet<u64>, usize) {
    let mut relationships = table
        .shape_representation_relationship
        .iter()
        .filter(|(_, relationship)| {
            matches!(
                &relationship.rep_1,
                PlaceHolder::Ref(Name::Entity(id)) if *id == source_representation
            )
        })
        .collect::<Vec<_>>();
    relationships.sort_unstable_by_key(|(id, _)| **id);

    let mut shapes = BTreeSet::new();
    let mut skipped = 0;
    for (_, relationship) in relationships {
        let PlaceHolder::Ref(Name::Entity(target_id)) = &relationship.rep_2 else {
            continue;
        };
        let Some(target) = table.shape_representation.get(target_id) else {
            continue;
        };
        for item in &target.items {
            let PlaceHolder::Ref(Name::Entity(item_id)) = item else {
                skipped += 1;
                continue;
            };
            if table.manifold_solid_brep.contains_key(item_id)
                || table.shell_based_surface_model.contains_key(item_id)
            {
                shapes.insert(*item_id);
            } else if !is_placement_item(table, *item_id) {
                skipped += 1;
            }
        }
    }
    (shapes, skipped)
}

fn is_placement_item(table: &Table, id: u64) -> bool {
    table.placement.contains_key(&id)
        || table.axis1_placement.contains_key(&id)
        || table.axis2_placement_2d.contains_key(&id)
        || table.axis2_placement_3d.contains_key(&id)
}

fn nonempty_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}
