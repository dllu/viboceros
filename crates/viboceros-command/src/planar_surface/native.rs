//! Plane grouping, region construction and nesting use usd-brep.
use super::*;
use viboceros_smlib::{KernelCurve, Shape};

pub(super) fn stage(
    curves: Vec<(ObjectId, NurbsCurve)>,
    tolerance: Tolerance,
    independent_contacts: bool,
) -> Result<(Vec<Brep>, Vec<ObjectId>), CommandError> {
    let mut profiles = Vec::new();
    let mut originals = Vec::new();
    let mut consumed = Vec::new();
    for (id, curve) in curves {
        if curve.is_closed()? && curve.is_planar(tolerance)? {
            profiles.push(KernelCurve::from_nurbs_for_modeling(&curve)?);
            originals.push((id, curve));
            consumed.push(id);
        }
    }
    if profiles.is_empty() {
        return Err(CommandError::NoPlanarSurfaceBoundaries);
    }
    if profiles.len() > 128 {
        return Err(
            viboceros_smlib::Error::Kernel("Too many planar surface profiles".into()).into(),
        );
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in 0..profiles.len() {
        let mut target = None;
        for (index, group) in groups.iter().enumerate() {
            let refs = group
                .iter()
                .copied()
                .chain(std::iter::once(i))
                .map(|j| &profiles[j])
                .collect::<Vec<_>>();
            if KernelCurve::are_coplanar(&refs, tolerance)? {
                target = Some(index);
                break;
            }
        }
        if let Some(index) = target {
            let group = &mut groups[index];
            group.push(i);
        } else {
            groups.push(vec![i]);
        }
    }
    let mut faces = Vec::new();
    for group in groups {
        let mut contact = false;
        for i in 0..if independent_contacts { group.len() } else { 0 } {
            for j in i + 1..group.len() {
                if super::portable::curves_contact(
                    &originals[group[i]].1,
                    &originals[group[j]].1,
                    tolerance,
                )? {
                    contact = true;
                    break;
                }
            }
            if contact {
                break;
            }
        }
        if contact {
            // Boundary contacts form independent input regions; the native
            // producer can merge them into a different union instead.
            faces.extend(
                super::portable::stage(
                    group.iter().map(|&i| originals[i].clone()).collect(),
                    tolerance,
                    true,
                )?
                .0,
            );
            continue;
        }
        let refs = group.iter().map(|&i| &profiles[i]).collect::<Vec<_>>();
        match Shape::planar_faces(&refs, tolerance) {
            Ok(output) => faces.extend(output.to_brep(tolerance)?.explode_faces(tolerance)?),
            Err(_) => {
                // Some valid concave profiles are rejected by the upstream
                // planar-face producer. Reuse the existing exact constructor
                // after native curve admission; invalid weights still fail
                // before this point and caller-owned profiles remain intact.
                let curves = group.iter().map(|&i| originals[i].clone()).collect();
                faces.extend(super::portable::stage(curves, tolerance, independent_contacts)?.0);
            }
        }
    }
    Ok((faces, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concave_profile_uses_the_existing_exact_constructor_when_upstream_rejects_it() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, "Polyline 0,0,0 4,0,0 4,4,0 2,1,0 0,4,0 0,0,0")
            .unwrap();
        let source = doc.objects().next().unwrap().id();
        doc.select_object(source, SelectionMode::Replace).unwrap();
        registry.execute(&mut doc, "PlanarSrf").unwrap();
        let area = doc
            .objects()
            .filter_map(|o| match o.geometry() {
                Geometry::Brep(b) => Some(b.area(doc.tolerance()).unwrap()),
                _ => None,
            })
            .sum::<Real>();
        assert!((area - 10.).abs() < 1e-10);
        assert!(doc.object(source).is_some());
        doc.undo().unwrap();
        assert_eq!(doc.objects().count(), 1);
    }
    #[test]
    fn rejected_later_profile_preserves_model_groups_selection_and_redo() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        let first = doc.objects().next().unwrap().id();
        let points = [[5., 0.], [6., 0.], [6., 1.], [5., 1.], [5., 0.]];
        let bad = NurbsCurve::try_new_rational(
            2,
            points
                .into_iter()
                .zip([1., 1., -0.2, 1., 1.])
                .map(|(p, w)| {
                    WeightedPoint3::try_new(Point3::try_new(p[0], p[1], 0.).unwrap(), w).unwrap()
                })
                .collect(),
            vec![0., 0., 0., 1., 2., 3., 3., 3.],
        )
        .unwrap();
        let second = doc.add_geometry(Geometry::NurbsCurve(bad)).unwrap();
        let group = doc.add_group(None, [first, second]).unwrap();
        registry.execute(&mut doc, "Point 30,0,0").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        doc.select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let members = doc.group(group).unwrap().members().collect::<Vec<_>>();
        let error = registry
            .execute(&mut doc, "PlanarSrf DeleteInput=Yes")
            .unwrap_err();
        assert!(matches!(error, CommandError::NativeSmlib(_)), "{error:?}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            doc.group(group).unwrap().members().collect::<Vec<_>>(),
            members
        );
        assert!(doc.is_selected(first) && doc.is_selected(second));
        assert_eq!(doc.redo_label(), Some("Point"));
    }
}
