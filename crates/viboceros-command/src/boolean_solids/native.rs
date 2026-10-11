//! Native closed-solid set operations, staged independently of document edits.
use super::*;
use viboceros_smlib::{BooleanOperation, Solid};
mod compound_intersection;
pub(crate) type Output = (ObjectId, Geometry, BTreeMap<String, String>);
pub(crate) struct UnionOutput {
    pub copies: Vec<Output>,
    pub consumed: Vec<ObjectId>,
}

fn unsupported(context: &str) -> CommandError {
    viboceros_smlib::Error::Kernel(context.into()).into()
}
pub(crate) fn epsilon(breps: &[&Brep], tolerance: Tolerance) -> Result<f64, CommandError> {
    let mut scale = 0_f64;
    for b in breps {
        scale = scale.max(b.bounds().min().distance_to(b.bounds().max())?);
    }
    Ok(tolerance.absolute() * scale * scale + tolerance.relative() * scale * scale * scale)
}
pub(crate) fn imports(breps: &[&Brep], tolerance: Tolerance) -> Result<Vec<Solid>, CommandError> {
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit.into());
    }
    breps
        .iter()
        .map(|b| {
            let solid = Solid::from_brep(b, tolerance)?;
            solid.validate_boundary_intersections(tolerance)?;
            Ok(solid)
        })
        .collect()
}
pub(crate) fn parts(solid: &Solid, tolerance: Tolerance) -> Result<Vec<Brep>, CommandError> {
    Ok(solid
        .material_parts()?
        .into_iter()
        .map(|b| b.to_brep(tolerance))
        .collect::<Result<_, _>>()?)
}

/// Original connected boundaries for command participation. The kernel treats
/// each query boundary as outward, including an original cavity shell.
pub(crate) fn shells(
    brep: &Brep,
    solid: &Solid,
    tolerance: Tolerance,
) -> Result<Vec<(Vec<usize>, Solid)>, CommandError> {
    let components = brep.edge_connected_face_components();
    if components.len() == 1 {
        return Ok(vec![(components[0].clone(), solid.try_clone()?)]);
    }
    components
        .iter()
        .map(|faces| {
            let mut shell = brep.sub_brep(faces, tolerance)?;
            if shell.signed_volume(tolerance)? < 0. {
                shell.reverse_orientation();
            }
            Ok((faces.clone(), Solid::from_brep(&shell, tolerance)?))
        })
        .collect()
}

pub(super) fn participating_output(
    solid: &Solid,
    active: &[Solid],
    tolerance: Tolerance,
) -> Result<Vec<Brep>, CommandError> {
    let result = solid.to_brep(tolerance)?;
    let mut outputs = Vec::new();
    for component in result.edge_connected_face_components() {
        let mut shell = result.sub_brep(&component, tolerance)?;
        if !shell.is_solid() {
            // The material boundary exporter retains native edge contacts.
            // They have no solid volume or declared outward orientation.
            outputs.push(shell);
            continue;
        }
        if shell.signed_volume(tolerance)? < 0. {
            shell.reverse_orientation();
        }
        let boundary = Solid::from_brep(&shell, tolerance)?;
        for source in active {
            if boundary.boundary_contact(source, tolerance)? {
                outputs.push(shell);
                break;
            }
        }
    }
    Ok(outputs)
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
    let mut parents = (0..breps.len()).collect::<Vec<_>>();
    let mut contacts = Vec::new();
    let mut equal = vec![vec![false; breps.len()]; breps.len()];
    for a in 0..breps.len() {
        for b in a + 1..breps.len() {
            // Delegate set coverage to the kernel. Small positive material
            // must not disappear merely because it has small integrated volume.
            let covered_a = native[a]
                .boolean(&native[b], BooleanOperation::Difference)?
                .is_empty()?;
            let covered_b = native[b]
                .boolean(&native[a], BooleanOperation::Difference)?
                .is_empty()?;
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
        let mut result = native[group[0]].boolean_with_postprocess(
            &native[group[1]],
            BooleanOperation::Union,
            merge,
        )?;
        for &i in &group[2..] {
            result = result.boolean_with_postprocess(&native[i], BooleanOperation::Union, merge)?;
        }
        let mut contributors = Vec::new();
        for (position, &i) in group.iter().enumerate() {
            // A contained input can still own a coincident exterior face.
            // Native boundary queries decide participation; material volume
            // cannot identify those contributors. Earlier equal inputs win.
            if group[..position].iter().any(|&j| equal[i][j]) {
                continue;
            }
            if native[i].boundary_contact(&result, tolerance)? {
                contributors.push(i);
            }
        }
        if contributors.is_empty() {
            return Err(unsupported("Union boundary contributors are ambiguous"));
        }
        let owner = contributors[0];
        let geometry_owner = *contributors.last().unwrap();
        let mut active = Vec::new();
        for &i in group {
            for (_, shell) in shells(breps[i], &native[i], tolerance)? {
                for &j in group {
                    if i != j && shell.boundary_contact(&native[j], tolerance)? {
                        active.push(shell);
                        break;
                    }
                }
            }
        }
        // Rhino returns participating connected boundaries as separate outward
        // objects, including opened cavity boundaries. Material-body extraction
        // alone would keep those cavities attached to the surrounding object.
        let mut outputs = participating_output(&result, &active, tolerance)?;
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
    if common {
        if breps
            .iter()
            .any(|b| b.edge_connected_face_components().len() > 1)
        {
            return compound_common(breps, &native, objects, tolerance);
        }
        return common_intersection(&native, objects, tolerance);
    }
    if breps
        .iter()
        .any(|b| b.edge_connected_face_components().len() > 1)
    {
        return compound_intersection::sets(breps, &native, objects, first, tolerance);
    }
    // Intersect each original pair, then union intersecting pair regions per material body.
    let mut regions = Vec::<(Solid, [usize; 2])>::new();
    let mut crossed = false;
    for a in 0..first {
        for b in first..native.len() {
            let region = native[a].boolean(&native[b], BooleanOperation::Intersection)?;
            if !(covered_by(&native[a], &native[b])? && covered_by(&native[b], &native[a])?) {
                crossed |= native[a].boundary_contact(&native[b], tolerance)?;
            }
            if !region.is_empty()? {
                if regions.len() >= 128 {
                    return Err(GeometryError::BrepBooleanWorkLimit.into());
                }
                regions.push((region, [a, b]));
            }
        }
    }
    if !crossed || regions.is_empty() {
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
            if region
                .boolean(&body, BooleanOperation::Intersection)?
                .is_empty()?
            {
                continue;
            }
            let mut covered = false;
            for (other, (container, _)) in regions.iter().enumerate() {
                if index == other {
                    continue;
                }
                let outside = region.boolean(container, BooleanOperation::Difference)?;
                if outside.is_empty()? && (!covered_by(container, region)? || other < index) {
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

fn covered_by(a: &Solid, b: &Solid) -> Result<bool, CommandError> {
    Ok(a.boolean(b, BooleanOperation::Difference)?.is_empty()?)
}

fn compound_common(
    breps: &[&Brep],
    inputs: &[Solid],
    objects: &[&viboceros_document::Object],
    tolerance: Tolerance,
) -> Result<Vec<Output>, CommandError> {
    let originals = breps
        .iter()
        .zip(inputs)
        .map(|(b, s)| shells(b, s, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut owners = BTreeSet::new();
    let mut active = Vec::new();
    for a in 0..inputs.len() {
        for b in a + 1..inputs.len() {
            for (_, left) in &originals[a] {
                for (_, right) in &originals[b] {
                    if covered_by(left, right)? && covered_by(right, left)? {
                        continue;
                    }
                    if left.boundary_contact(right, tolerance)? {
                        owners.extend([a, b]);
                        active.push(left.try_clone()?);
                        active.push(right.try_clone()?);
                    }
                }
            }
        }
    }
    let Some(&first) = owners.first() else {
        return Err(CommandError::NothingIntersected);
    };
    let mut current = inputs[0].try_clone()?;
    for input in &inputs[1..] {
        current = current.boolean(input, BooleanOperation::Intersection)?;
        if current.is_empty()? {
            return Err(CommandError::NothingIntersected);
        }
    }
    let outputs = participating_output(&current, &active, tolerance)?;
    if outputs.is_empty() {
        return Err(CommandError::NothingIntersected);
    }
    let text = if outputs.len() == 1 {
        objects[*owners.last().unwrap()]
            .geometry_user_text()
            .clone()
    } else {
        BTreeMap::new()
    };
    let mut result = Vec::new();
    for output in outputs {
        let mut owner = first;
        if output.is_solid() {
            let boundary = Solid::from_brep(&output, tolerance)?;
            for &i in &owners {
                if covered_by(&boundary, &inputs[i])? && covered_by(&inputs[i], &boundary)? {
                    owner = i;
                    break;
                }
            }
        }
        result.push((objects[owner].id(), Geometry::Brep(output), text.clone()));
    }
    Ok(result)
}

fn common_intersection(
    inputs: &[Solid],
    objects: &[&viboceros_document::Object],
    tolerance: Tolerance,
) -> Result<Vec<Output>, CommandError> {
    let mut current = inputs[0].try_clone()?;
    let mut owners = vec![0];
    let mut interacted = false;
    for (i, input) in inputs.iter().enumerate().skip(1) {
        let current_inside = covered_by(&current, input)?;
        let input_inside = covered_by(input, &current)?;
        if current_inside && input_inside {
            return Err(unsupported("Equal common-intersection operands"));
        }
        let crossing = current.boundary_contact(input, tolerance)?;
        if !crossing {
            if input_inside {
                current = input.try_clone()?;
                owners = vec![i];
            } else if !current_inside {
                current = current.boolean(input, BooleanOperation::Intersection)?;
                if current.is_empty()? {
                    return Err(CommandError::NothingIntersected);
                }
            }
            continue;
        }
        let mut retained = Vec::new();
        for owner in owners {
            let keep = if input_inside {
                inputs[owner].boundary_contact(input, tolerance)?
            } else {
                !covered_by(input, &inputs[owner])?
            };
            if keep {
                retained.push(owner);
            }
        }
        retained.push(i);
        owners = retained;
        current = current.boolean(input, BooleanOperation::Intersection)?;
        if current.is_empty()? {
            return Err(CommandError::NothingIntersected);
        }
        interacted = true;
    }
    if !interacted {
        return Err(CommandError::NothingIntersected);
    }
    let result = parts(&current, tolerance)?;
    let owner = owners[0];
    let text = if result.len() == 1 {
        objects[*owners.last().unwrap()]
            .geometry_user_text()
            .clone()
    } else {
        BTreeMap::new()
    };
    Ok(result
        .into_iter()
        .map(|b| (objects[owner].id(), Geometry::Brep(b), text.clone()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_common_and_two_set_regions_survive_without_volume_filtering() {
        for common in [true, false] {
            let tolerance = Tolerance::DEFAULT;
            let mut doc = Document::default();
            let a = doc
                .add_geometry(Geometry::Brep(
                    Brep::try_box(frame(0.), [[0., 100_000.]; 3], tolerance).unwrap(),
                ))
                .unwrap();
            let b = doc
                .add_geometry(Geometry::Brep(
                    Brep::try_box(frame(0.), [[99_999., 100_001.]; 3], tolerance).unwrap(),
                ))
                .unwrap();
            doc.clear_history().unwrap();
            let command = if common {
                format!("BooleanIntersection FirstSet={a},{b}")
            } else {
                format!("BooleanIntersection FirstSet={a} SecondSet={b}")
            };
            CommandRegistry::with_builtins()
                .execute(&mut doc, &command)
                .unwrap();
            assert_eq!(doc.objects().count(), 1);
            let Geometry::Brep(result) = doc.objects().next().unwrap().geometry() else {
                panic!("editable B-rep")
            };
            assert!(result.is_solid());
            assert!((result.signed_volume(tolerance).unwrap() - 1.).abs() < 1e-9);
            assert!(result.vertices().iter().all(|v| {
                v.point()
                    .to_array()
                    .iter()
                    .all(|c| (99_999. - 1e-8..=100_000. + 1e-8).contains(c))
            }));
            doc.undo().unwrap();
            assert!(doc.object(a).is_some() && doc.object(b).is_some());
        }
    }

    #[test]
    fn curved_common_contacts_match_public_rhino_command_outcomes() {
        let capture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/common_curved.json"
        ))
        .unwrap();
        for row in capture["results"].as_array().unwrap() {
            let case = row["id"]
                .as_str()
                .unwrap()
                .strip_prefix("curved_common_")
                .unwrap();
            let radius = match case {
                "inside" => 0.5,
                "crossing" => 1.25,
                _ => 1.,
            };
            let tolerance = Tolerance::DEFAULT;
            let mut doc = Document::default();
            let a = doc
                .add_geometry(Geometry::Brep(
                    Brep::try_box(frame(0.), [[0., 2.]; 3], tolerance).unwrap(),
                ))
                .unwrap();
            let sphere_frame = Frame3::try_from_normal(
                p(1., 1., 1.),
                Vector3::try_new(0., 0., 1.).unwrap(),
                tolerance,
            )
            .unwrap();
            let b = doc
                .add_geometry(Geometry::Brep(
                    Brep::try_surface_face(
                        NurbsSurface::try_sphere(sphere_frame, radius).unwrap(),
                        tolerance,
                    )
                    .unwrap(),
                ))
                .unwrap();
            doc.set_object_geometry_user_text([a], "Code", Some("geometry-0"))
                .unwrap();
            doc.set_object_geometry_user_text([b], "Code", Some("geometry-1"))
                .unwrap();
            doc.select_objects_direct([a, b], SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let command = if case == "two_sets_inscribed" {
                format!("BooleanIntersection FirstSet={a} SecondSet={b}")
            } else {
                format!("BooleanIntersection FirstSet={a},{b}")
            };
            let result = CommandRegistry::with_builtins().execute_postselected(
                &mut doc,
                &command,
                Default::default(),
            );
            let native = row["value"]["command"]["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == "BooleanIntersection")
                .unwrap()["result"]
                .as_str()
                .unwrap();
            assert_eq!(result.is_ok(), native == "Success", "{case}: {result:?}");
            if result.is_ok() {
                assert_eq!(doc.objects().count(), 1);
                let output = doc.objects().next().unwrap();
                assert_eq!(
                    output.geometry_user_text().get("Code").map(String::as_str),
                    Some("geometry-1")
                );
                let Geometry::Brep(brep) = output.geometry() else {
                    panic!("editable B-rep")
                };
                let on_boundary = |point: Point3| {
                    let coordinates = point.to_array();
                    assert!(
                        coordinates
                            .iter()
                            .all(|&c| (-1e-8..=2. + 1e-8).contains(&c))
                    );
                    let radial = coordinates
                        .iter()
                        .map(|c| (c - 1.).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    assert!(radial <= radius + 1e-8);
                    let plane_distance = coordinates
                        .iter()
                        .map(|c| c.abs().min((c - 2.).abs()))
                        .fold(f64::INFINITY, f64::min);
                    assert!((radial - radius).abs().min(plane_distance) <= 1e-8);
                };
                for edge in brep.edges() {
                    let domain = edge.curve().domain();
                    for i in 0..=32 {
                        on_boundary(
                            edge.curve()
                                .evaluate(
                                    domain.start()
                                        + (domain.end() - domain.start()) * i as f64 / 32.,
                                )
                                .unwrap(),
                        );
                    }
                }
                for face in brep.faces() {
                    for boundary in face.loops() {
                        for trim in boundary.trims() {
                            let domain = trim.curve().domain();
                            for i in 0..=32 {
                                let uv = trim
                                    .curve()
                                    .evaluate(
                                        domain.start()
                                            + (domain.end() - domain.start()) * i as f64 / 32.,
                                    )
                                    .unwrap();
                                on_boundary(face.surface().evaluate(uv.x(), uv.y()).unwrap());
                            }
                        }
                    }
                }
                doc.undo().unwrap();
            }
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before, "{case}");
        }
    }

    #[test]
    fn small_boundary_contributor_survives_dimensional_volume_thresholds() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let mut doc = Document::default();
        let large = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[0., 100_000.]; 3], tolerance).unwrap(),
            ))
            .unwrap();
        let small = doc
            .add_geometry(Geometry::Brep(
                Brep::try_box(frame, [[99_999., 100_001.]; 3], tolerance).unwrap(),
            ))
            .unwrap();
        doc.set_object_geometry_user_text([small], "part", Some("small-boundary"))
            .unwrap();
        doc.select_objects_direct([large, small], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        CommandRegistry::with_builtins()
            .execute(&mut doc, "BooleanUnion")
            .unwrap();
        assert_eq!(doc.objects().count(), 1);
        let output = doc.objects().next().unwrap();
        assert_eq!(
            output.geometry_user_text().get("part").map(String::as_str),
            Some("small-boundary")
        );
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("editable B-rep")
        };
        assert!(
            brep.vertices()
                .iter()
                .any(|v| v.point() == p(100_001., 100_001., 100_001.))
        );
        assert!(7. < epsilon(&[brep], tolerance).unwrap());
        doc.undo().unwrap();
        assert!(doc.object(large).is_some() && doc.object(small).is_some());
    }

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
