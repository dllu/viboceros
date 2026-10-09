//! One-level/recursive expansion with source metadata, group isolation and history.
use super::*;

#[derive(Clone, Debug)]
pub struct PreparedBlockExplosion {
    source: Object,
    tolerance: Tolerance,
    pieces: Vec<Piece>,
    delete_source: bool,
    layers: Vec<Layer>,
}

#[derive(Clone, Debug)]
struct Piece {
    geometry: GeometrySnapshot,
    attributes: ObjectAttributes,
    geometry_user_text: BTreeMap<String, String>,
    prototype_groups: Vec<ScopedGroup>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ScopedGroup {
    path: Vec<(BlockDefinitionId, usize)>,
    id: GroupId,
}

impl PreparedBlockExplosion {
    pub fn source_id(&self) -> ObjectId {
        self.source.id
    }
    pub fn output_count(&self) -> usize {
        self.pieces.len()
    }
}

impl Document {
    /// Prepare all fallible geometry and metadata without editing the model.
    /// A one-level expansion keeps nested references as instance objects.
    pub fn prepare_block_explosion(
        &self,
        id: ObjectId,
        recursive: bool,
        maximum: usize,
    ) -> Result<PreparedBlockExplosion, DocumentError> {
        let source = self.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
        self.ensure_object_editable(source)?;
        let Geometry::BlockInstance(instance) = source.geometry() else {
            return Err(DocumentError::NotBlockInstance(id));
        };
        let definition = self
            .block_definition(instance.reference().definition())
            .ok_or(DocumentError::BlockDefinitionNotFound(
                instance.reference().definition(),
            ))?;
        let count = if recursive {
            instance.members().len()
        } else {
            definition.members().len()
        };
        if count > maximum {
            return Err(DocumentError::BlockExplosionLimit);
        }
        let mut pieces = Vec::with_capacity(count);
        if recursive {
            for member in instance.members() {
                let mut groups = Vec::new();
                // Inner prototype groups precede outer containers' groups.
                for (depth, location) in member.path.iter().enumerate().rev() {
                    let prototype = &self
                        .block_definition(location.definition)
                        .unwrap()
                        .members()[location.member_index];
                    let path = member.path[..depth]
                        .iter()
                        .map(|location| (location.definition, location.member_index))
                        .collect::<Vec<_>>();
                    for group in prototype.group_ids() {
                        let group = ScopedGroup {
                            path: path.clone(),
                            id: *group,
                        };
                        if !groups.contains(&group) {
                            groups.push(group);
                        }
                    }
                }
                let state = self.block_member_display(source.attributes(), &member.path)?;
                pieces.push(Piece {
                    geometry: member.geometry.clone(),
                    attributes: exploded_attributes(&member.attributes, state),
                    geometry_user_text: member.geometry_user_text.clone(),
                    prototype_groups: groups,
                });
            }
        } else {
            for (index, member) in definition.members().iter().enumerate() {
                let path = [BlockMemberLocation {
                    definition: definition.id(),
                    member_index: index,
                }];
                let state = self.block_member_display(source.attributes(), &path)?;
                let geometry = match member.content() {
                    BlockContent::Reference(reference) => self
                        .block_instance_geometry(BlockReference::try_new(
                            reference.definition(),
                            reference
                                .transform()
                                .then(instance.reference().transform())?,
                        )?)?
                        .into(),
                    BlockContent::Geometry(geometry) => {
                        if instance.reference().transform() == AffineTransform3::identity() {
                            geometry.clone()
                        } else {
                            geometry
                                .transformed(
                                    instance.reference().transform(),
                                    super::blocks::transformation_tolerance(
                                        geometry,
                                        self.tolerance,
                                        1.,
                                    )?,
                                )?
                                .into()
                        }
                    }
                };
                pieces.push(Piece {
                    geometry,
                    attributes: exploded_attributes(member.attributes(), state),
                    geometry_user_text: member.geometry_user_text().clone(),
                    prototype_groups: member
                        .group_ids()
                        .iter()
                        .map(|id| ScopedGroup {
                            path: Vec::new(),
                            id: *id,
                        })
                        .collect(),
                });
            }
        }
        for piece in &pieces {
            self.layer(piece.attributes.layer_id())
                .ok_or(DocumentError::LayerNotFound(piece.attributes.layer_id()))?;
            for group in &piece.prototype_groups {
                self.group(group.id)
                    .ok_or(DocumentError::GroupNotFound(group.id))?;
            }
        }
        let mut layer_ids = BTreeSet::from([source.attributes.layer_id]);
        for piece in &pieces {
            layer_ids.insert(piece.attributes.layer_id());
        }
        if recursive {
            for member in instance.members() {
                for location in &member.path {
                    layer_ids.insert(
                        self.block_definition(location.definition)
                            .unwrap()
                            .members()[location.member_index]
                            .attributes()
                            .layer_id(),
                    );
                }
            }
        }
        let layers = layer_ids
            .into_iter()
            .map(|id| self.layer(id).unwrap().clone())
            .collect();
        let layer = self.layer(source.attributes.layer_id).unwrap();
        let delete_source = source.attributes.visible
            && !source.attributes.locked
            && layer.visible
            && !layer.locked;
        Ok(PreparedBlockExplosion {
            source: source.clone(),
            tolerance: self.tolerance,
            pieces,
            delete_source,
            layers,
        })
    }

    /// Commit prepared records as one undoable operation. Each root's prototype
    /// groups receive independent definitions; root group memberships are retained.
    /// Restricted selected group peers follow the existing Explode copy policy.
    pub fn commit_block_explosions(
        &mut self,
        prepared: Vec<PreparedBlockExplosion>,
        group_output: bool,
    ) -> Result<Vec<ObjectId>, DocumentError> {
        let mut seen = BTreeSet::new();
        for plan in &prepared {
            if !seen.insert(plan.source.id) {
                return Err(DocumentError::DuplicateCopySource(plan.source.id));
            }
            let current = self
                .object(plan.source.id)
                .ok_or(DocumentError::ObjectNotFound(plan.source.id))?;
            if current != &plan.source || self.tolerance != plan.tolerance {
                return Err(DocumentError::StaleBlockExplosion);
            }
            if plan
                .layers
                .iter()
                .any(|layer| self.layer(layer.id()) != Some(layer))
            {
                return Err(DocumentError::StaleBlockExplosion);
            }
            self.ensure_object_editable(current)?;
            for piece in &plan.pieces {
                for group in &piece.prototype_groups {
                    self.group(group.id)
                        .ok_or(DocumentError::GroupNotFound(group.id))?;
                }
                for group in &plan.source.group_ids {
                    self.group(*group)
                        .ok_or(DocumentError::GroupNotFound(*group))?;
                }
            }
        }
        let count = prepared.iter().try_fold(0usize, |count, plan| {
            count
                .checked_add(plan.pieces.len())
                .ok_or(DocumentError::BlockExplosionLimit)
        })?;
        if count == 0 {
            return Ok(Vec::new());
        }
        self.objects
            .try_reserve_exact(count)
            .map_err(|_| DocumentError::TooManyObjectCopies)?;
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction("Explode blocks")?;
        }
        let result = (|| {
            let mut outputs = Vec::with_capacity(count);
            let mut removed = Vec::new();
            let mut names = groups::GroupNames::default();
            for plan in prepared {
                let mut mapped = BTreeMap::new();
                for piece in &plan.pieces {
                    for group in &piece.prototype_groups {
                        if !mapped.contains_key(group) {
                            let name = names.next(self);
                            let copied = self.add_empty_group(Some(name))?;
                            mapped.insert(group.clone(), copied);
                        }
                    }
                }
                let output_group = if group_output {
                    let name = names.next(self);
                    Some(self.add_empty_group(Some(name))?)
                } else {
                    None
                };
                for piece in plan.pieces {
                    let id = ObjectId::new();
                    let index = self.objects.len();
                    self.objects.push(Object {
                        id,
                        geometry: piece.geometry,
                        attributes: piece.attributes,
                        geometry_user_text: piece.geometry_user_text,
                        isolation: ObjectIsolation::None,
                        group_ids: Vec::new(),
                    });
                    self.record_edit(
                        "Explode block member",
                        Edit::ObjectInserted {
                            index,
                            id,
                            stored: None,
                            selected: false,
                        },
                    );
                    let memberships = piece
                        .prototype_groups
                        .iter()
                        .map(|group| mapped[group])
                        .chain(plan.source.group_ids.iter().copied())
                        .chain(output_group)
                        .collect::<Vec<_>>();
                    self.set_object_group_memberships_at(index, memberships)?;
                    outputs.push(id);
                }
                if plan.delete_source {
                    removed.push(plan.source.id);
                }
            }
            // Consume source selection before recording removals, as with the
            // ordinary Explode adapter. Restricted roots remain unselected.
            self.select_command_results(outputs.iter().copied())?;
            self.delete_objects(removed)?;
            Ok(outputs)
        })();
        if owns {
            if result.is_ok() {
                self.commit_transaction()?;
            } else {
                self.rollback_transaction()?;
            }
        }
        result
    }
}

fn exploded_attributes(
    attributes: &ObjectAttributes,
    state: BlockMemberDisplay,
) -> ObjectAttributes {
    let mut result = attributes
        .clone()
        .with_file_state(state.visible, state.locked);
    if attributes.color_source() == ObjectColorSource::Parent {
        result = result.with_object_color(state.color);
    }
    result
}

#[cfg(test)]
mod tests;
