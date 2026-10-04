//! Shape-preserving placement units shared by commands and temporary display.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use viboceros_document::{CopyGroupPolicy, GroupId, ReplacementHistory};

#[derive(Clone, Debug)]
pub struct RigidLayout {
    units: Vec<Vec<ObjectId>>,
    sources: Vec<ObjectId>,
    centers: BTreeMap<ObjectId, Point3>,
}

impl RigidLayout {
    pub fn try_new(
        document: &Document,
        ids: &[ObjectId],
        grips: &[viboceros_document::ControlPointId],
    ) -> Result<Self, CommandError> {
        Self::build(document, ids, grips, true)
    }

    /// ScalePositions places each object independently of group membership.
    pub fn try_individual(document: &Document, ids: &[ObjectId]) -> Result<Self, CommandError> {
        Self::build(document, ids, &[], false)
    }

    fn build(
        document: &Document,
        ids: &[ObjectId],
        grips: &[viboceros_document::ControlPointId],
        grouped: bool,
    ) -> Result<Self, CommandError> {
        let ignored = grips.iter().map(|p| p.object).collect::<BTreeSet<_>>();
        let ids = ids
            .iter()
            .copied()
            .filter(|id| !ignored.contains(id))
            .collect::<Vec<_>>();
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum Unit {
            Object(ObjectId),
            Group(GroupId),
        }
        let mut slots = BTreeMap::new();
        let mut units = Vec::<Vec<ObjectId>>::new();
        let mut group_bounds = BTreeMap::new();
        let mut object_bounds = BTreeMap::new();
        for id in &ids {
            let object = document
                .object(*id)
                .ok_or(DocumentError::ObjectNotFound(*id))?;
            let key = if grouped {
                object.top_group().map_or(Unit::Object(*id), Unit::Group)
            } else {
                Unit::Object(*id)
            };
            let slot = *slots.entry(key).or_insert_with(|| {
                units.push(Vec::new());
                units.len() - 1
            });
            units[slot].push(*id);
            let bounds = object.geometry().tight_bounds(document.tolerance())?;
            object_bounds.insert(*id, bounds);
            // Every selected membership contributes to that group's center,
            // even when an overlapping member has a different top group.
            if grouped {
                for group in object.group_ids() {
                    let union = group_bounds.get(group).map_or(Ok(bounds), |b| {
                        viboceros_geometry::BoundingBox3::union(*b, bounds)
                    })?;
                    group_bounds.insert(*group, union);
                }
            }
        }
        let centers = ids
            .iter()
            .map(|id| {
                let object = document.object(*id).unwrap();
                let bounds = if grouped {
                    object
                        .top_group()
                        .map_or(object_bounds[id], |group| group_bounds[&group])
                } else {
                    object_bounds[id]
                };
                Ok((*id, bounds.center()?))
            })
            .collect::<Result<BTreeMap<_, _>, GeometryError>>()?;
        Ok(Self {
            sources: units.iter().flatten().copied().collect(),
            units,
            centers,
        })
    }

    pub fn sources(&self) -> &[ObjectId] {
        &self.sources
    }
    pub fn center(&self, id: ObjectId) -> Option<Point3> {
        self.centers.get(&id).copied()
    }
}

pub fn rigid_map(
    center: Point3,
    transform: AffineTransform3,
) -> Result<AffineTransform3, GeometryError> {
    Ok(AffineTransform3::from_translation(
        center.vector_to(transform.transform_point(center)?)?,
    ))
}

pub(super) fn apply(
    document: &mut Document,
    sources: &history_policy::TransformSources,
    transform: AffineTransform3,
    grip_transform: AffineTransform3,
    copy: bool,
) -> Result<(usize, usize), CommandError> {
    let layout = RigidLayout::try_new(document, &sources.ids, &sources.grips)?;
    if layout.sources.is_empty() || !copy && transform == AffineTransform3::identity() {
        if !copy {
            // Native numeric completion leaves the last partial X/Y grip
            // display without replacing its owner or creating an Undo entry.
            document
                .transform_control_point_display(sources.grips.iter().copied(), grip_transform)?;
        }
        return Ok((layout.sources.len(), 0));
    }
    let transforms = layout
        .sources
        .iter()
        .map(|id| Ok((*id, rigid_map(layout.centers[id], transform)?)))
        .collect::<Result<BTreeMap<_, _>, GeometryError>>()?;
    // Copies stage all geometry before allocating groups. In-place edits use
    // the document's atomic batch and transform each object only once.
    let staged = if copy {
        layout
            .units
            .iter()
            .map(|unit| {
                unit.iter()
                    .map(|id| {
                        let geometry = document
                            .object(*id)
                            .unwrap()
                            .geometry()
                            .transformed_for_edit(transforms[id], document.tolerance())?;
                        Ok((*id, geometry))
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    if sources.release_on_replay && (sources.grips.is_empty() || sources.postselected) {
        document.release_command_selection_on_history_replay(layout.sources.iter().copied())?;
    }
    let mut count = 0;
    if copy {
        for unit in staged {
            let policy = if unit.len() == 1 {
                CopyGroupPolicy::DefinitionsOnly
            } else {
                CopyGroupPolicy::Preserve
            };
            count += document
                .copy_object_geometries_with_groups(unit, policy)?
                .len();
        }
    } else {
        document
            .transform_objects_individually(transforms, ReplacementHistory::EveryReplacement)?;
    }
    if !copy {
        document.move_objects_to_end_in_order(layout.sources.iter().copied())?;
        document.transform_control_point_display(sources.grips.iter().copied(), grip_transform)?;
    }
    if sources.postselected && !copy {
        document.clear_selection();
    } else if copy {
        document.select_command_results(sources.ids.iter().copied())?;
    }
    Ok((layout.sources.len(), if copy { count } else { 0 }))
}
