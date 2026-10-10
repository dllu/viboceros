//! Optional closed curved partitions, preserving target metadata and branch lineage.
use super::*;
use viboceros_smlib::{BooleanOperation, Solid};
type Output = (ObjectId, Geometry, BTreeMap<String, String>);
type Staged = (Vec<Output>, Vec<ObjectId>);
struct Piece {
    solid: Solid,
    clear_text: bool,
}
fn error(message: &str) -> CommandError {
    viboceros_smlib::Error::Kernel(message.into()).into()
}
fn volume(solid: &Solid, tolerance: Tolerance) -> Result<f64, CommandError> {
    if solid.is_empty()? {
        return Ok(0.);
    }
    Ok(solid.to_brep(tolerance)?.signed_volume(tolerance)?)
}
fn dimensional_epsilon(brep: &Brep, tolerance: Tolerance) -> Result<f64, CommandError> {
    let scale = brep.bounds().min().distance_to(brep.bounds().max())?;
    Ok(tolerance.absolute() * scale * scale + tolerance.relative() * scale * scale * scale)
}
pub(super) fn split(
    breps: &[&Brep],
    objects: &[&viboceros_document::Object],
    ids: &[ObjectId],
    first: &[ObjectId],
    second: &[ObjectId],
    tolerance: Tolerance,
) -> Result<Staged, CommandError> {
    let native = breps
        .iter()
        .map(|b| Solid::from_brep(b, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let indices = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect::<BTreeMap<_, _>>();
    let mut copies = Vec::new();
    let mut changed = Vec::new();
    for &target in first {
        let index = indices[&target];
        let original = breps[index].signed_volume(tolerance)?;
        let epsilon = dimensional_epsilon(breps[index], tolerance)?;
        if original <= epsilon {
            return Err(error("Split target has no positive material volume"));
        }
        let mut active = BTreeSet::new();
        let mut active_target: Option<Solid> = None;
        let mut expected = 0.;
        for body in native[index].material_parts()? {
            let body_volume = volume(&body, tolerance)?;
            let mut crosses = false;
            for &id in second {
                let cutter = indices[&id];
                if cutter == index {
                    continue;
                }
                let inside = body.boolean(&native[cutter], BooleanOperation::Intersection)?;
                let amount = volume(&inside, tolerance)?;
                if amount > epsilon
                    && body_volume - amount > epsilon
                    && body.boundary_contact(&native[cutter], tolerance)?
                {
                    active.insert(cutter);
                    crosses = true;
                }
            }
            if crosses {
                expected += body_volume;
                active_target = Some(if let Some(current) = active_target {
                    current.boolean(&body, BooleanOperation::Union)?
                } else {
                    body
                });
            }
        }
        let Some(active_target) = active_target else {
            continue;
        };
        // Keep cutter order from the original getter, not the object-index map.
        let active = second
            .iter()
            .map(|id| indices[id])
            .filter(|i| active.contains(i))
            .collect::<Vec<_>>();
        let mut pieces = vec![Piece {
            solid: active_target,
            clear_text: false,
        }];
        let mut was_split = false;
        for cutter in active {
            let mut next = Vec::new();
            for piece in pieces {
                let before = volume(&piece.solid, tolerance)?;
                let inside = piece
                    .solid
                    .boolean(&native[cutter], BooleanOperation::Intersection)?;
                let amount = volume(&inside, tolerance)?;
                if amount <= epsilon || (before - amount).abs() <= epsilon {
                    next.push(piece);
                    continue;
                }
                let outside = piece
                    .solid
                    .boolean(&native[cutter], BooleanOperation::Difference)?;
                let remainder = volume(&outside, tolerance)?;
                if remainder <= epsilon {
                    return Err(error("Split branches disagree about material coverage"));
                }
                if (amount + remainder - before).abs() > epsilon {
                    return Err(error("Split failed volume conservation"));
                }
                was_split = true;
                for branch in [outside, inside] {
                    let bodies = branch.material_parts()?;
                    let clear = piece.clear_text || bodies.len() > 1;
                    for solid in bodies {
                        next.push(Piece {
                            solid,
                            clear_text: clear,
                        });
                    }
                }
                if next.len() > 4096 {
                    return Err(GeometryError::BrepBooleanWorkLimit.into());
                }
            }
            pieces = next;
        }
        if !was_split || pieces.len() < 2 {
            continue;
        }
        let mut target_outputs = Vec::new();
        let mut total = 0.;
        for piece in pieces {
            let brep = piece.solid.to_brep(tolerance)?;
            total += brep.signed_volume(tolerance)?;
            target_outputs.push((
                target,
                Geometry::Brep(brep),
                if piece.clear_text {
                    BTreeMap::new()
                } else {
                    objects[index].geometry_user_text().clone()
                },
            ));
        }
        if (total - expected).abs() > epsilon {
            return Err(error("Split output volume does not match target"));
        }
        changed.push(target);
        copies.extend(target_outputs);
        if copies.len() > 4096 {
            return Err(GeometryError::BrepBooleanWorkLimit.into());
        }
    }
    if changed.is_empty() {
        return Err(CommandError::NothingSplit);
    }
    Ok((copies, changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn frame() -> Frame3 {
        Frame3::try_from_normal(
            p(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    fn setup() -> (Document, Vec<ObjectId>) {
        let mut doc = Document::default();
        let target = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(), 2., 0., 10., Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let cutter = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(),
                    [[-3., 3.], [-3., 3.], [4., 6.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.set_object_names([(target, Some("Cylinder".into()))])
            .unwrap();
        doc.set_object_user_text([target], "part", Some("A"))
            .unwrap();
        doc.set_object_geometry_user_text([target], "source", Some("cylinder"))
            .unwrap();
        doc.clear_history().unwrap();
        (doc, vec![target, cutter])
    }
    #[test]
    fn curved_split_conserves_volume_retains_cutters_and_branch_metadata() {
        for delete in [true, false] {
            let (mut doc, ids) = setup();
            let registry = CommandRegistry::with_builtins();
            let group = doc.add_group(Some("target".into()), [ids[0]]).unwrap();
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "BooleanSplit FirstSet={} SecondSet={} DeleteInput={}",
                        ids[0],
                        ids[1],
                        if delete { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            assert_eq!(doc.object(ids[0]).is_none(), delete);
            assert!(doc.object(ids[1]).is_some());
            let outputs = doc
                .objects()
                .filter(|o| !ids.contains(&o.id()))
                .collect::<Vec<_>>();
            assert_eq!(outputs.len(), 3);
            let mut sum = 0.;
            let mut texts = 0;
            for object in outputs {
                let Geometry::Brep(b) = object.geometry() else {
                    panic!()
                };
                assert!(b.is_solid());
                sum += b.signed_volume(Tolerance::DEFAULT).unwrap();
                assert_eq!(object.attributes().name(), Some("Cylinder"));
                assert_eq!(
                    object
                        .attributes()
                        .user_text()
                        .get("part")
                        .map(String::as_str),
                    Some("A")
                );
                assert!(object.group_ids().contains(&group));
                assert!(doc.is_selected(object.id()));
                if !object.geometry_user_text().is_empty() {
                    texts += 1;
                }
            }
            assert!((sum - 40. * std::f64::consts::PI).abs() < 1e-6);
            assert_eq!(texts, 1);
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            doc.redo().unwrap();
            assert_eq!(doc.objects().filter(|o| !ids.contains(&o.id())).count(), 3);
        }
    }
    #[test]
    fn containment_disjoint_self_and_invalid_inputs_leave_model_unchanged() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let inner = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(),
                    [[-0.5, 0.5], [-0.5, 0.5], [2., 3.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let enclosing = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(),
                    [[-5., 5.], [-5., 5.], [-1., 11.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let remote = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame(), [[20., 22.]; 3], Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        for cutter in [inner, enclosing, remote, ids[0]] {
            assert!(matches!(
                registry.execute(
                    &mut doc,
                    &format!("BooleanSplit FirstSet={} SecondSet={cutter}", ids[0])
                ),
                Err(CommandError::NothingSplit)
            ));
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
        }
    }
    #[test]
    fn postselected_curved_split_uses_unselected_outputs() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        registry
            .execute_postselected(
                &mut doc,
                &format!("BooleanSplit FirstSet={} SecondSet={}", ids[0], ids[1]),
                Default::default(),
            )
            .unwrap();
        assert_eq!(doc.selected_object_count(), 0);
        assert!(doc.object(ids[1]).is_some());
    }

    #[test]
    fn uncut_compound_material_is_excluded_from_replacements() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let cylinder = Brep::try_cylinder(frame(), 2., 0., 10., Tolerance::DEFAULT).unwrap();
        let remote = Brep::try_box(frame(), [[20., 22.]; 3], Tolerance::DEFAULT).unwrap();
        let target = doc
            .add_geometry(Geometry::Brep(
                Brep::try_disjoint_union(vec![cylinder, remote], Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let cutter = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(),
                    [[-3., 3.], [-3., 3.], [4., 6.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!("BooleanSplit FirstSet={target} SecondSet={cutter}"),
            )
            .unwrap();
        assert!(doc.object(target).is_none());
        assert!(doc.object(cutter).is_some());
        let outputs = doc
            .objects()
            .filter(|o| o.id() != cutter)
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 3);
        let volume = outputs
            .iter()
            .map(|o| {
                let Geometry::Brep(b) = o.geometry() else {
                    panic!()
                };
                b.signed_volume(Tolerance::DEFAULT).unwrap()
            })
            .sum::<f64>();
        assert!((volume - 40. * std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn overlapping_cutters_retain_each_interface_and_disconnected_text_lineage() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let other = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(),
                    [[-3., 3.], [-3., 3.], [5., 8.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "BooleanSplit FirstSet={} SecondSet={},{other}",
                    ids[0], ids[1]
                ),
            )
            .unwrap();
        let outputs = doc
            .objects()
            .filter(|o| o.id() != ids[1] && o.id() != other)
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 5);
        let mut ranges = Vec::new();
        let mut sum = 0.;
        let mut texts = 0;
        for object in outputs {
            let Geometry::Brep(b) = object.geometry() else {
                panic!()
            };
            let low = b
                .vertices()
                .iter()
                .map(|v| v.point().z())
                .min_by(f64::total_cmp)
                .unwrap();
            let high = b
                .vertices()
                .iter()
                .map(|v| v.point().z())
                .max_by(f64::total_cmp)
                .unwrap();
            ranges.push((low, high));
            sum += b.signed_volume(Tolerance::DEFAULT).unwrap();
            if !object.geometry_user_text().is_empty() {
                texts += 1;
            }
        }
        ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(
            ranges,
            vec![(0., 4.), (4., 5.), (5., 6.), (6., 8.), (8., 10.)]
        );
        assert!((sum - 40. * std::f64::consts::PI).abs() < 1e-6);
        assert_eq!(texts, 2);
    }

    #[test]
    fn internal_cutters_do_not_create_extra_cavity_partitions() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let internal = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(),
                    [[-0.5, 0.5], [-0.5, 0.5], [1., 2.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "BooleanSplit FirstSet={} SecondSet={},{internal}",
                    ids[0], ids[1]
                ),
            )
            .unwrap();
        assert_eq!(
            doc.objects()
                .filter(|o| o.id() != ids[1] && o.id() != internal)
                .count(),
            3
        );
        assert!(doc.object(internal).is_some());
    }

    #[test]
    fn open_curved_inputs_fail_before_any_document_edit() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let Geometry::Brep(b) = doc.object(ids[1]).unwrap().geometry() else {
            panic!()
        };
        let open = b.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
        let open = doc.add_geometry(Geometry::Brep(open)).unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("BooleanSplit FirstSet={} SecondSet={open}", ids[0])
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
}
