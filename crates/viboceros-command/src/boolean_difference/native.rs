//! Optional curved-solid subtraction, staged before document mutation.
use super::*;
use viboceros_smlib::{BooleanOperation, Solid};

type Output = (ObjectId, Geometry, BTreeMap<String, String>);
pub(super) fn difference(
    breps: &[&Brep],
    objects: &[&viboceros_document::Object],
    targets: &[ObjectId],
    tolerance: Tolerance,
) -> Result<Vec<Output>, CommandError> {
    let native = breps
        .iter()
        .map(|b| Solid::from_brep(b, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut outputs = Vec::new();
    let mut changed = false;
    for (index, &id) in targets.iter().enumerate() {
        let original = breps[index].signed_volume(tolerance)?;
        if original <= 0. {
            return Err(viboceros_smlib::Error::Kernel(
                "Curved subtraction requires outward material boundaries".into(),
            )
            .into());
        }
        let mut result = None;
        for cutter in &native[targets.len()..] {
            let source = result.as_ref().unwrap_or(&native[index]);
            let next = source.boolean(cutter, BooleanOperation::Difference)?;
            let empty = next.is_empty()?;
            result = Some(next);
            if empty {
                break;
            }
        }
        let result = result.expect("validated nonempty cutter set");
        if result.is_empty()? {
            changed = true;
            continue;
        }
        let parts = result
            .material_parts()?
            .into_iter()
            .map(|part| part.to_brep(tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let volume = parts
            .iter()
            .map(|part| part.signed_volume(tolerance))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<f64>();
        let scale = breps[index]
            .bounds()
            .min()
            .distance_to(breps[index].bounds().max())?;
        let epsilon = tolerance.absolute() * scale * scale + tolerance.relative() * original.abs();
        if volume > original + epsilon {
            return Err(viboceros_smlib::Error::Kernel(
                "Subtraction increased target volume".into(),
            )
            .into());
        }
        if original - volume > epsilon {
            changed = true;
            let text = if parts.len() == 1 {
                objects[index].geometry_user_text().clone()
            } else {
                BTreeMap::new()
            };
            for part in parts {
                outputs.push((id, Geometry::Brep(part), text.clone()));
            }
        } else {
            outputs.push((
                id,
                Geometry::Brep(breps[index].clone()),
                objects[index].geometry_user_text().clone(),
            ));
        }
    }
    if !changed {
        return Err(CommandError::NothingSubtracted);
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn setup() -> (Document, Vec<ObjectId>) {
        let mut doc = Document::default();
        let frame = Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let a =
            Brep::try_box(frame, [[-5., 5.], [-5., 5.], [0., 10.]], Tolerance::DEFAULT).unwrap();
        let b = Brep::try_cylinder(frame, 2., -1., 11., Tolerance::DEFAULT).unwrap();
        let ids = vec![
            doc.add_geometry(Geometry::Brep(a)).unwrap(),
            doc.add_geometry(Geometry::Brep(b)).unwrap(),
        ];
        doc.clear_history().unwrap();
        (doc, ids)
    }
    #[test]
    fn curved_difference_keeps_metadata_groups_and_one_undo() {
        for (delete, cutters) in [(true, true), (true, false), (false, true)] {
            let (mut doc, ids) = setup();
            let registry = CommandRegistry::with_builtins();
            doc.set_object_names([(ids[0], Some("Workpiece".into()))])
                .unwrap();
            doc.set_object_user_text([ids[0]], "part", Some("A"))
                .unwrap();
            doc.set_object_geometry_user_text([ids[0]], "fixture", Some("curved"))
                .unwrap();
            let group = doc.add_group(Some("target".into()), [ids[0]]).unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            doc.clear_history().unwrap();
            let yn = |v| if v { "Yes" } else { "No" };
            registry.execute(&mut doc,&format!("BooleanDifference FirstSet={} SecondSet={} DeleteInput={} DeleteCutters={}",ids[0],ids[1],yn(delete),yn(cutters))).unwrap();
            let result = doc.objects().find(|o| !ids.contains(&o.id())).unwrap();
            let Geometry::Brep(b) = result.geometry() else {
                panic!()
            };
            assert!(
                (b.signed_volume(Tolerance::DEFAULT).unwrap()
                    - (1000. - 40. * std::f64::consts::PI))
                    .abs()
                    < 1e-6
            );
            assert!(result.group_ids().contains(&group));
            assert_eq!(result.attributes().name(), Some("Workpiece"));
            assert_eq!(
                result
                    .attributes()
                    .user_text()
                    .get("part")
                    .map(String::as_str),
                Some("A")
            );
            assert_eq!(
                result
                    .geometry_user_text()
                    .get("fixture")
                    .map(String::as_str),
                Some("curved")
            );
            assert_eq!(doc.object(ids[0]).is_none(), delete);
            assert_eq!(doc.object(ids[1]).is_none(), delete && cutters);
            assert_eq!(doc.undo_label(), Some("BooleanDifference"));
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
    #[test]
    fn unsupported_open_target_is_atomic() {
        let (mut doc, ids) = setup();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut doc,
                &format!("BooleanDifference FirstSet={} SecondSet={}", ids[0], ids[1]),
            )
            .unwrap();
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        let source = doc.object(ids[0]).unwrap().geometry().clone();
        let Geometry::Brep(source) = source else {
            panic!()
        };
        let open = source.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
        let open = doc.add_geometry(Geometry::Brep(open)).unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("BooleanDifference FirstSet={open} SecondSet={}", ids[1])
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }

    #[test]
    fn complete_removal_and_disjoint_cutters_follow_existing_history_policy() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let enclosing = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    Frame3::try_from_normal(
                        point(0., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    [[-4., 4.], [-4., 4.], [-2., 12.]],
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
                    "BooleanDifference FirstSet={} SecondSet={enclosing}",
                    ids[1]
                ),
            )
            .unwrap();
        assert!(doc.object(ids[1]).is_none() && doc.object(enclosing).is_none());
        doc.undo().unwrap();
        let source = doc.object(ids[1]).unwrap().clone();
        let remote = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    Frame3::try_from_normal(
                        point(0., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    [[20., 22.]; 3],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(
                &mut doc,
                &format!("BooleanDifference FirstSet={} SecondSet={remote}", ids[1])
            ),
            Err(CommandError::NothingSubtracted)
        ));
        assert_eq!(doc.object(ids[1]).unwrap(), &source);
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }

    #[test]
    fn curved_split_results_are_separate_objects_with_source_attributes() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let frame = Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let slab = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[-3., 3.], [-3., 3.], [4., 6.]], Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.set_object_names([(ids[1], Some("Tube".into()))])
            .unwrap();
        doc.set_object_geometry_user_text([ids[1]], "source", Some("cylinder"))
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!("BooleanDifference FirstSet={} SecondSet={slab}", ids[1]),
            )
            .unwrap();
        let outputs = doc
            .objects()
            .filter(|o| o.id() != ids[0])
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 2);
        let mut volume = 0.;
        for o in outputs {
            assert_eq!(o.attributes().name(), Some("Tube"));
            assert!(o.geometry_user_text().is_empty());
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            assert_eq!(b.edge_connected_face_components().len(), 1);
            volume += b.signed_volume(Tolerance::DEFAULT).unwrap();
        }
        assert!((volume - 40. * std::f64::consts::PI).abs() < 1e-6);
    }
}
