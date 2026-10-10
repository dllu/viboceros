//! Native closed-solid set operations, staged independently of document edits.
use super::*;
use viboceros_smlib::{BooleanOperation, Solid};
pub(crate) type Output = (ObjectId, Geometry, BTreeMap<String, String>);
pub(crate) struct UnionOutput {
    pub copies: Vec<Output>,
    pub consumed: Vec<ObjectId>,
}

fn unsupported(context: &str) -> CommandError {
    viboceros_smlib::Error::Kernel(context.into()).into()
}
fn volume(solid: &Solid, tolerance: Tolerance) -> Result<f64, CommandError> {
    if solid.is_empty()? {
        return Ok(0.);
    }
    Ok(solid.to_brep(tolerance)?.signed_volume(tolerance)?)
}
fn epsilon(breps: &[&Brep], tolerance: Tolerance) -> Result<f64, CommandError> {
    let mut scale = 0_f64;
    for b in breps {
        scale = scale.max(b.bounds().min().distance_to(b.bounds().max())?);
    }
    Ok(tolerance.absolute() * scale * scale + tolerance.relative() * scale * scale * scale)
}
fn imports(breps: &[&Brep], tolerance: Tolerance) -> Result<Vec<Solid>, CommandError> {
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit.into());
    }
    Ok(breps
        .iter()
        .map(|b| Solid::from_brep(b, tolerance))
        .collect::<Result<_, _>>()?)
}
fn parts(solid: &Solid, tolerance: Tolerance) -> Result<Vec<Brep>, CommandError> {
    Ok(solid
        .material_parts()?
        .into_iter()
        .map(|b| b.to_brep(tolerance))
        .collect::<Result<_, _>>()?)
}
fn root(parents: &[usize], mut i: usize) -> usize {
    while parents[i] != i {
        i = parents[i];
    }
    i
}

pub(crate) fn union(
    breps: &[&Brep],
    objects: &[&viboceros_document::Object],
    merge: bool,
    tolerance: Tolerance,
) -> Result<UnionOutput, CommandError> {
    let native = imports(breps, tolerance)?;
    let eps = epsilon(breps, tolerance)?;
    let volumes = breps
        .iter()
        .map(|b| b.signed_volume(tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut parents = (0..breps.len()).collect::<Vec<_>>();
    let mut contacts = Vec::new();
    let mut equal = vec![vec![false; breps.len()]; breps.len()];
    for a in 0..breps.len() {
        for b in a + 1..breps.len() {
            let overlap = native[a].boolean(&native[b], BooleanOperation::Intersection)?;
            let size = volume(&overlap, tolerance)?;
            let covered_a = (volumes[a] - size).abs() <= eps;
            let covered_b = (volumes[b] - size).abs() <= eps;
            if covered_a && covered_b {
                equal[a][b] = true;
                equal[b][a] = true;
                let x = root(&parents, a);
                let y = root(&parents, b);
                parents[y] = x;
                continue;
            }
            let contact = native[a].boundary_contact(&native[b], tolerance)?;
            if contact || covered_a || covered_b {
                let x = root(&parents, a);
                let y = root(&parents, b);
                parents[y] = x;
                if contact {
                    contacts.push([a, b]);
                }
            }
        }
    }
    let mut groups = BTreeMap::<usize, Vec<usize>>::new();
    for i in 0..breps.len() {
        groups.entry(root(&parents, i)).or_default().push(i);
    }
    let mut copies = Vec::new();
    let mut consumed = Vec::new();
    for group in groups.values().filter(|g| {
        contacts
            .iter()
            .any(|pair| g.contains(&pair[0]) && g.contains(&pair[1]))
    }) {
        let mut result = native[group[0]].boolean(&native[group[1]], BooleanOperation::Union)?;
        for &i in &group[2..] {
            result = result.boolean(&native[i], BooleanOperation::Union)?;
        }
        let mut contributors = Vec::new();
        for &i in group {
            let mut others: Option<Solid> = None;
            for &j in group {
                if i == j {
                    continue;
                }
                if j > i && equal[i][j] {
                    continue;
                }
                others = Some(if let Some(current) = others {
                    current.boolean(&native[j], BooleanOperation::Union)?
                } else {
                    native[j].try_clone()?
                });
            }
            let exposed = if let Some(others) = others {
                volume(
                    &native[i].boolean(&others, BooleanOperation::Difference)?,
                    tolerance,
                )? > eps
            } else {
                true
            };
            if exposed {
                contributors.push(i);
            }
        }
        if contributors.is_empty() {
            return Err(unsupported("Union boundary contributors are ambiguous"));
        }
        let owner = contributors[0];
        let geometry_owner = *contributors.last().unwrap();
        let mut outputs = parts(&result, tolerance)?;
        let text = if outputs.len() == 1 {
            objects[geometry_owner].geometry_user_text().clone()
        } else {
            BTreeMap::new()
        };
        for mut b in outputs.drain(..) {
            if merge {
                b = b
                    .try_merge_coplanar_polygon_faces_in_groups(
                        &vec![0; b.faces().len()],
                        tolerance,
                    )?
                    .unwrap_or(b);
            }
            copies.push((objects[owner].id(), Geometry::Brep(b), text.clone()));
        }
        consumed.extend(group.iter().map(|&i| objects[i].id()));
    }
    if copies.is_empty() {
        return Err(CommandError::NothingUnioned);
    }
    consumed.sort_unstable();
    consumed.dedup();
    Ok(UnionOutput { copies, consumed })
}

pub(crate) fn intersection(
    breps: &[&Brep],
    objects: &[&viboceros_document::Object],
    first: usize,
    common: bool,
    tolerance: Tolerance,
) -> Result<Vec<Output>, CommandError> {
    let native = imports(breps, tolerance)?;
    let eps = epsilon(breps, tolerance)?;
    if common {
        let mut current = native[0].try_clone()?;
        let mut owner = 0;
        let mut last = 0;
        for i in 1..native.len() {
            let before = volume(&current, tolerance)?;
            let mut next = current.boolean(&native[i], BooleanOperation::Intersection)?;
            let after = volume(&next, tolerance)?;
            if after <= eps {
                return Err(CommandError::NothingIntersected);
            }
            if (after - before).abs() > eps {
                if (after - breps[i].signed_volume(tolerance)?).abs() <= eps {
                    owner = i;
                    next = native[i].try_clone()?;
                }
                last = i;
            }
            current = next;
        }
        let result = parts(&current, tolerance)?;
        let text = if result.len() == 1 {
            objects[last].geometry_user_text().clone()
        } else {
            BTreeMap::new()
        };
        return Ok(result
            .into_iter()
            .map(|b| (objects[owner].id(), Geometry::Brep(b), text.clone()))
            .collect());
    }
    // Intersect each original pair, then union intersecting pair regions per material body.
    let mut regions = Vec::<(Solid, [usize; 2])>::new();
    let mut region_volumes = Vec::new();
    for a in 0..first {
        for b in first..native.len() {
            let region = native[a].boolean(&native[b], BooleanOperation::Intersection)?;
            let region_volume = volume(&region, tolerance)?;
            if region_volume > eps {
                if regions.len() >= 128 {
                    return Err(GeometryError::BrepBooleanWorkLimit.into());
                }
                region_volumes.push(region_volume);
                regions.push((region, [a, b]));
            }
        }
    }
    if regions.is_empty() {
        return Err(CommandError::NothingIntersected);
    }
    let mut merged = regions[0].0.try_clone()?;
    for (r, _) in &regions[1..] {
        merged = merged.boolean(r, BooleanOperation::Union)?;
    }
    let mut outputs = Vec::new();
    for body in merged.material_parts()? {
        let mut contributors = Vec::new();
        for (index, (region, pair)) in regions.iter().enumerate() {
            if volume(
                &region.boolean(&body, BooleanOperation::Intersection)?,
                tolerance,
            )? <= eps
            {
                continue;
            }
            let mut covered = false;
            for (other, (container, _)) in regions.iter().enumerate() {
                if index == other {
                    continue;
                }
                let outside = region.boolean(container, BooleanOperation::Difference)?;
                if volume(&outside, tolerance)? <= eps
                    && (region_volumes[other] > region_volumes[index] + eps || other < index)
                {
                    covered = true;
                    break;
                }
            }
            if !covered {
                contributors.push(pair[0]);
            }
        }
        if contributors.is_empty() {
            return Err(unsupported("Intersection contributors are ambiguous"));
        }
        contributors.sort_unstable();
        let owner = contributors[0];
        let last = *contributors.last().unwrap();
        outputs.push((
            objects[owner].id(),
            Geometry::Brep(body.to_brep(tolerance)?),
            objects[last].geometry_user_text().clone(),
        ));
    }
    let mut counts = BTreeMap::new();
    for (id, _, _) in &outputs {
        *counts.entry(*id).or_insert(0) += 1;
    }
    for (id, _, text) in &mut outputs {
        if counts[id] > 1 {
            text.clear();
        }
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn frame(z: f64) -> Frame3 {
        Frame3::try_from_normal(
            p(0., 0., z),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    fn setup() -> (Document, Vec<ObjectId>) {
        let mut doc = Document::default();
        let a = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(0.),
                    [[-5., 5.], [-5., 5.], [0., 10.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let b = doc
            .add_geometry(Geometry::Brep(
                Brep::try_surface_grid(
                    &NurbsSurface::try_sphere(frame(10.), 2.).unwrap(),
                    &[],
                    &[],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.set_object_names([(a, Some("box".into())), (b, Some("sphere".into()))])
            .unwrap();
        doc.set_object_geometry_user_text([a], "source", Some("box"))
            .unwrap();
        doc.set_object_geometry_user_text([b], "source", Some("sphere"))
            .unwrap();
        doc.clear_history().unwrap();
        (doc, vec![a, b])
    }
    fn result_volume(doc: &Document, ids: &[ObjectId]) -> f64 {
        doc.objects()
            .filter(|o| !ids.contains(&o.id()))
            .map(|o| {
                let Geometry::Brep(b) = o.geometry() else {
                    panic!()
                };
                b.signed_volume(Tolerance::DEFAULT).unwrap()
            })
            .sum()
    }
    #[test]
    fn curved_union_preserves_owners_groups_selection_and_undo() {
        for delete in [true, false] {
            let (mut doc, ids) = setup();
            let registry = CommandRegistry::with_builtins();
            let group = doc.add_group(Some("box group".into()), [ids[0]]).unwrap();
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            registry
                .execute(
                    &mut doc,
                    if delete {
                        "BooleanUnion"
                    } else {
                        "BooleanUnion DeleteInput=No"
                    },
                )
                .unwrap();
            let output = doc.objects().find(|o| !ids.contains(&o.id())).unwrap();
            assert_eq!(output.attributes().name(), Some("box"));
            assert!(output.group_ids().contains(&group));
            assert_eq!(
                output
                    .geometry_user_text()
                    .get("source")
                    .map(String::as_str),
                Some("sphere")
            );
            assert!(
                (result_volume(&doc, &ids) - (1000. + 16. * std::f64::consts::PI / 3.)).abs()
                    < 1e-6
            );
            assert_eq!(doc.object(ids[0]).is_none(), delete);
            assert!(doc.is_selected(output.id()));
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
    #[test]
    fn curved_common_and_two_set_intersections_preserve_first_set_ownership() {
        for common in [true, false] {
            let (mut doc, ids) = setup();
            let registry = CommandRegistry::with_builtins();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let command = if common {
                format!("BooleanIntersection FirstSet={},{}", ids[0], ids[1])
            } else {
                format!(
                    "BooleanIntersection FirstSet={} SecondSet={}",
                    ids[0], ids[1]
                )
            };
            registry.execute(&mut doc, &command).unwrap();
            let output = doc.objects().find(|o| !ids.contains(&o.id())).unwrap();
            assert_eq!(output.attributes().name(), Some("box"));
            assert_eq!(
                output
                    .geometry_user_text()
                    .get("source")
                    .map(String::as_str),
                Some(if common { "sphere" } else { "box" })
            );
            assert!((result_volume(&doc, &ids) - 16. * std::f64::consts::PI / 3.).abs() < 1e-6);
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
    #[test]
    fn disjoint_and_contained_union_inputs_remain_unchanged() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let small = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(2.), 1., 0., 2., Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([ids[0], small], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut doc, "BooleanUnion"),
            Err(CommandError::NothingUnioned)
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        let remote = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(
                    Frame3::try_from_normal(
                        p(30., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    1.,
                    0.,
                    2.,
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([ids[0], remote], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut doc, "BooleanUnion"),
            Err(CommandError::NothingUnioned)
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn successful_union_consumes_interior_inputs_and_keeps_disjoint_singletons() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let interior = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(2.), 1., 0., 2., Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let remote = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(
                    Frame3::try_from_normal(
                        p(30., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    1.,
                    0.,
                    2.,
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([ids[0], ids[1], interior, remote], SelectionMode::Replace)
            .unwrap();
        let original = doc.object(remote).unwrap().clone();
        doc.clear_history().unwrap();
        registry.execute(&mut doc, "BooleanUnion").unwrap();
        assert!(doc.object(interior).is_none());
        assert_eq!(doc.object(remote).unwrap(), &original);
        assert!(doc.is_selected(remote));
        let result = doc.objects().find(|o| o.id() != remote).unwrap();
        assert_eq!(result.attributes().name(), Some("box"));
        assert_eq!(
            result
                .geometry_user_text()
                .get("source")
                .map(String::as_str),
            Some("sphere")
        );
    }

    #[test]
    fn common_intersection_skips_enclosing_inputs_and_admits_smaller_owner() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let enclosing = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(-5.), 20., 0., 30., Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.set_object_names([(enclosing, Some("enclosing".into()))])
            .unwrap();
        doc.set_object_geometry_user_text([enclosing], "source", Some("enclosing"))
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "BooleanIntersection FirstSet={},{},{enclosing}",
                    ids[0], ids[1]
                ),
            )
            .unwrap();
        let result = doc.objects().next().unwrap();
        assert_eq!(result.attributes().name(), Some("box"));
        assert_eq!(
            result
                .geometry_user_text()
                .get("source")
                .map(String::as_str),
            Some("sphere")
        );
        doc.undo().unwrap();
        let smaller = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(9.), 0.25, 0., 0.5, Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.set_object_names([(smaller, Some("smaller".into()))])
            .unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "BooleanIntersection FirstSet={},{},{smaller}",
                    ids[0], ids[1]
                ),
            )
            .unwrap();
        let result = doc.objects().find(|o| o.id() != enclosing).unwrap();
        assert_eq!(result.attributes().name(), Some("smaller"));
    }

    #[test]
    fn two_set_intersection_deletes_disjoint_inputs_and_keeps_maximal_first_owner() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let enclosing = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(-5.), 20., 0., 30., Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.set_object_names([(enclosing, Some("large".into()))])
            .unwrap();
        doc.set_object_geometry_user_text([enclosing], "source", Some("large"))
            .unwrap();
        let remote = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(
                    Frame3::try_from_normal(
                        p(50., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    1.,
                    0.,
                    2.,
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
                    "BooleanIntersection FirstSet={},{enclosing} SecondSet={},{remote}",
                    ids[0], ids[1]
                ),
            )
            .unwrap();
        assert!(doc.object(remote).is_none());
        assert_eq!(doc.objects().len(), 1);
        let result = doc.objects().next().unwrap();
        assert_eq!(result.attributes().name(), Some("large"));
        assert_eq!(
            result
                .geometry_user_text()
                .get("source")
                .map(String::as_str),
            Some("large")
        );
    }

    #[test]
    fn empty_intersection_and_open_union_leave_geometry_and_history_unchanged() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let remote = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(
                    Frame3::try_from_normal(
                        p(30., 0., 0.),
                        Vector3::try_new(0., 0., 1.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                    1.,
                    0.,
                    2.,
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
                &format!("BooleanIntersection FirstSet={} SecondSet={remote}", ids[1])
            ),
            Err(CommandError::NothingIntersected)
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        let Geometry::Brep(b) = doc.object(ids[0]).unwrap().geometry() else {
            panic!()
        };
        let open = b.sub_brep(&[0], Tolerance::DEFAULT).unwrap();
        let open = doc.add_geometry(Geometry::Brep(open)).unwrap();
        doc.select_objects_direct([open, ids[1]], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(registry.execute(&mut doc, "BooleanUnion").is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }

    #[test]
    fn two_set_intersection_keeps_separate_material_outputs_and_clears_split_text() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let cylinder = doc
            .add_geometry(Geometry::Brep(
                Brep::try_cylinder(frame(0.), 1., 0., 10., Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        doc.set_object_geometry_user_text([cylinder], "source", Some("tube"))
            .unwrap();
        let a = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(0.),
                    [[-2., 2.], [-2., 2.], [0., 2.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let b = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame(0.),
                    [[-2., 2.], [-2., 2.], [8., 10.]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!("BooleanIntersection FirstSet={cylinder} SecondSet={a},{b}"),
            )
            .unwrap();
        assert_eq!(doc.objects().len(), 2);
        let mut sum = 0.;
        for object in doc.objects() {
            assert!(object.geometry_user_text().is_empty());
            let Geometry::Brep(b) = object.geometry() else {
                panic!()
            };
            sum += b.signed_volume(Tolerance::DEFAULT).unwrap();
        }
        assert!((sum - 4. * std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn equal_inputs_do_not_steal_boundary_ownership_in_a_changing_union() {
        let (mut doc, ids) = setup();
        let registry = CommandRegistry::with_builtins();
        let geometry = doc.object(ids[0]).unwrap().geometry().clone();
        let duplicate = doc.add_geometry(geometry).unwrap();
        doc.set_object_names([(duplicate, Some("duplicate".into()))])
            .unwrap();
        doc.select_objects_direct([ids[0], duplicate, ids[1]], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        registry.execute(&mut doc, "BooleanUnion").unwrap();
        assert_eq!(doc.objects().len(), 1);
        let result = doc.objects().next().unwrap();
        assert_eq!(result.attributes().name(), Some("box"));
        assert_eq!(
            result
                .geometry_user_text()
                .get("source")
                .map(String::as_str),
            Some("sphere")
        );
    }
}
