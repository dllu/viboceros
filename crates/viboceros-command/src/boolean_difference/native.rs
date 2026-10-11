//! Native solid subtraction with command contact and metadata policies.
use super::*;
use crate::boolean_solids::native::{Output, imports, parts};
use std::borrow::Cow;
use viboceros_smlib::{BooleanOperation, Solid};
pub(super) fn difference(
    breps: &[&Brep],
    objects: &[&viboceros_document::Object],
    targets: &[ObjectId],
    tolerance: Tolerance,
) -> Result<Vec<Output>, CommandError> {
    let native = imports(breps, tolerance)?;
    let mut outputs = Vec::new();
    let mut changed = false;
    for (index, &id) in targets.iter().enumerate() {
        let components = breps[index].edge_connected_face_components();
        let cutters = &native[targets.len()..];
        let mut active = vec![false; cutters.len()];
        let mut faces = Vec::new();
        for component in &components {
            // Rhino decides participation per original shell. Inactive detached
            // shells and unopened cavities are omitted from replacements.
            let imported;
            let boundary = if components.len() == 1 {
                &native[index]
            } else {
                let shell = breps[index].sub_brep(component, tolerance)?;
                let shell = if shell.signed_volume(tolerance)? < 0. {
                    shell.reversed()
                } else {
                    shell
                };
                imported = Solid::from_brep(&shell, tolerance)?;
                &imported
            };
            let mut participates = false;
            for (i, cutter) in cutters.iter().enumerate() {
                if boundary.boundary_contact(cutter, tolerance)? {
                    active[i] = true;
                    participates = true;
                }
            }
            if participates {
                faces.extend(component);
            }
        }
        if faces.is_empty() {
            outputs.push((
                id,
                Geometry::Brep(breps[index].clone()),
                objects[index].geometry_user_text().clone(),
            ));
            continue;
        }
        let participating = if faces.len() == breps[index].faces().len() {
            Cow::Borrowed(breps[index])
        } else {
            Cow::Owned(breps[index].sub_brep(&faces, tolerance)?)
        };
        let subset = if matches!(participating, Cow::Owned(_)) {
            Some(Solid::from_brep(&participating, tolerance)?)
        } else {
            None
        };
        let input = subset.as_ref().unwrap_or(&native[index]);
        let original = participating.signed_volume(tolerance)?;
        if original <= 0. {
            return Err(viboceros_smlib::Error::Kernel(
                "Subtraction requires outward material boundaries".into(),
            )
            .into());
        }
        let mut result = None;
        for (i, cutter) in cutters.iter().enumerate() {
            // Command participation is decided against the original boundary,
            // so a previous cut cannot activate a strictly interior cutter.
            if !active[i] {
                continue;
            }
            let removed = input.boolean(cutter, BooleanOperation::Difference)?;
            if removed.is_empty()?
                && cutter
                    .boolean(input, BooleanOperation::Difference)?
                    .is_empty()?
            {
                return Err(viboceros_smlib::Error::Kernel(
                    "Coincident subtraction operands are unsupported".into(),
                )
                .into());
            }
            changed = true;
            let source = result.as_ref().unwrap_or(input);
            let next = if result.is_none() {
                removed
            } else {
                source.boolean(cutter, BooleanOperation::Difference)?
            };
            let empty = next.is_empty()?;
            result = Some(next);
            if empty {
                break;
            }
        }
        let result = result.expect("participating shell has an original interacting cutter");
        if result.is_empty()? {
            changed = true;
            continue;
        }
        let parts = parts(&result, tolerance)?;
        let volume = parts
            .iter()
            .map(|part| part.signed_volume(tolerance))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<f64>();
        let scale = participating
            .bounds()
            .min()
            .distance_to(participating.bounds().max())?;
        let epsilon = tolerance.absolute() * scale * scale + tolerance.relative() * original.abs();
        if volume > original + epsilon {
            return Err(viboceros_smlib::Error::Kernel(
                "Subtraction increased target volume".into(),
            )
            .into());
        }
        // A small volume difference does not establish geometric equality:
        // a narrow through-hole can have arbitrarily small removed volume.
        let text = if parts.len() == 1 {
            objects[index].geometry_user_text().clone()
        } else {
            BTreeMap::new()
        };
        for part in parts {
            outputs.push((id, Geometry::Brep(part), text.clone()));
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
                    [[-4., 4.], [-4., 4.], [-1., 12.]],
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

    #[test]
    fn small_corner_cut_is_retained_below_the_volume_tolerance() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let mut doc = Document::default();
        let source = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[0., 100_000.]; 3], tolerance).unwrap(),
            ))
            .unwrap();
        let cutter = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[99_999., 100_001.]; 3], tolerance).unwrap(),
            ))
            .unwrap();
        assert!(1. < tolerance.absolute() * 3. * 100_000_f64.powi(2));
        CommandRegistry::with_builtins()
            .execute(
                &mut doc,
                &format!("BooleanDifference FirstSet={source} SecondSet={cutter}",),
            )
            .unwrap();
        assert_eq!(doc.objects().count(), 1);
        let Geometry::Brep(brep) = doc.objects().next().unwrap().geometry() else {
            panic!("editable B-rep")
        };
        assert!(brep.is_solid());
        assert_eq!(
            brep.faces().len(),
            9,
            "the three new corner faces must survive"
        );
        assert!(
            brep.vertices()
                .iter()
                .any(|v| v.point() == point(99_999., 99_999., 99_999.))
        );
    }

    #[test]
    fn unopened_curved_cavity_and_detached_shell_are_excluded_from_replacement() {
        let tolerance = Tolerance::DEFAULT;
        let frame = |origin| {
            Frame3::try_from_normal(origin, Vector3::try_new(0., 0., 1.).unwrap(), tolerance)
                .unwrap()
        };
        let target = Brep::try_disjoint_union(
            vec![
                Brep::try_box(frame(point(0., 0., 0.)), [[0., 3.]; 3], tolerance).unwrap(),
                Brep::try_surface_face(
                    NurbsSurface::try_sphere(frame(point(1.5, 1.5, 1.5)), 0.25).unwrap(),
                    tolerance,
                )
                .unwrap()
                .reversed(),
                Brep::try_box(frame(point(0., 0., 0.)), [[10., 11.]; 3], tolerance).unwrap(),
            ],
            tolerance,
        )
        .unwrap();
        let mut doc = Document::default();
        let source = doc.add_geometry(Geometry::Brep(target.clone())).unwrap();
        let cutter = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(point(0., 0., 0.)),
                    [[2.5, 3.5], [2.5, 3.5], [1., 2.]],
                    tolerance,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.set_object_geometry_user_text([source], "part", Some("compound"))
            .unwrap();
        doc.clear_history().unwrap();
        CommandRegistry::with_builtins()
            .execute(
                &mut doc,
                &format!("BooleanDifference FirstSet={source} SecondSet={cutter} DeleteInput=No",),
            )
            .unwrap();
        let outputs = doc
            .objects()
            .filter(|o| o.id() != source && o.id() != cutter)
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1);
        assert_eq!(
            outputs[0]
                .geometry_user_text()
                .get("part")
                .map(String::as_str),
            Some("compound")
        );
        let Geometry::Brep(result) = outputs[0].geometry() else {
            panic!("editable B-rep")
        };
        assert!((result.signed_volume(tolerance).unwrap() - 26.75).abs() < 1e-8);
        assert_eq!(
            Solid::from_brep(result, tolerance)
                .unwrap()
                .material_census()
                .unwrap(),
            [1, 0]
        );
        assert_eq!(
            doc.object(source).unwrap().geometry(),
            &Geometry::Brep(target.clone())
        );
        doc.undo().unwrap();
        assert_eq!(doc.objects().count(), 2);
        assert_eq!(
            doc.object(source).unwrap().geometry(),
            &Geometry::Brep(target)
        );
    }
}
