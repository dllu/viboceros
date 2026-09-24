//! Exact normal offsets of planar surfaces and single-face B-reps.

use super::*;

const USAGE: &str = "OffsetSrf distance [Solid=Yes|No] [BothSides=Yes|No] [DeleteInput=Yes|No]";

pub(super) struct OffsetSurfaceCommand;

#[derive(Clone, Copy)]
struct Options {
    distance: Real,
    solid: bool,
    both_sides: bool,
    delete_input: bool,
}

impl Command for OffsetSurfaceCommand {
    fn name(&self) -> &'static str {
        "OffsetSrf"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse(arguments, document.tolerance())?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut staged = Vec::with_capacity(
            selected.len() * (1 + usize::from(options.both_sides && !options.solid)),
        );
        for (id, geometry) in &selected {
            let normal = planar_normal(geometry, document.tolerance())?;
            let profile = options
                .solid
                .then(|| solid_profile(geometry, document.tolerance()))
                .transpose()?;
            if options.solid && options.both_sides {
                let offset = normal.scaled(options.distance)?;
                let profile = profile.as_ref().expect("solid offset has a profile");
                staged.push((
                    *id,
                    Geometry::Brep(Brep::try_extruded_curve(
                        profile,
                        normal.scaled(-options.distance)?,
                        offset,
                        document.tolerance(),
                    )?),
                ));
                continue;
            }
            for signed_distance in [options.distance, -options.distance]
                .into_iter()
                .take(1 + usize::from(options.both_sides))
            {
                let offset = normal.scaled(signed_distance)?;
                let result = if let Some(profile) = &profile {
                    Geometry::Brep(Brep::try_extruded_curve(
                        profile,
                        Vector3::try_new(0.0, 0.0, 0.0)?,
                        offset,
                        document.tolerance(),
                    )?)
                } else {
                    geometry.transformed(
                        AffineTransform3::from_translation(offset),
                        document.tolerance(),
                    )?
                };
                staged.push((*id, result));
            }
        }
        let output_ids = document.copy_object_pieces_into_source_groups(staged)?;
        if options.delete_input {
            document.delete_objects(selected.into_iter().map(|(id, _)| id))?;
        }
        let count = output_ids.len();
        document.select_objects_direct(output_ids, SelectionMode::Replace)?;
        Ok(format!("Offset {count} surface(s)"))
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        // The numeric validity is checked after selection using model tolerance.
        parse(arguments, Tolerance::NUMERICAL_VALIDATION)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Surfaces,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

fn parse(arguments: &[&str], tolerance: Tolerance) -> Result<Options, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(USAGE));
    };
    let distance = if let Some((name, value)) = first.split_once('=') {
        if !option_name_eq(name, "Distance") {
            return Err(CommandError::Usage(USAGE));
        }
        parse_finite_real(value)?
    } else {
        parse_finite_real(first)?
    };
    if distance.abs() <= tolerance.absolute() {
        return Err(CommandError::Usage(USAGE));
    }
    let (mut solid, mut both_sides, mut delete_input) = (None, None, None);
    for argument in &arguments[1..] {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let choice = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        let slot = if option_name_eq(name, "Solid") {
            &mut solid
        } else if option_name_eq(name, "BothSides") {
            &mut both_sides
        } else if option_name_eq(name, "DeleteInput") {
            &mut delete_input
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if slot.replace(choice).is_some() {
            return Err(CommandError::Usage(USAGE));
        }
    }
    Ok(Options {
        distance,
        solid: solid.unwrap_or(false),
        both_sides: both_sides.unwrap_or(false),
        delete_input: delete_input.unwrap_or(false),
    })
}

fn planar_normal(geometry: &Geometry, tolerance: Tolerance) -> Result<Vector3, CommandError> {
    let (surface, reversed) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed())
        }
        _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
    };
    let plane = surface
        .plane(tolerance)?
        .ok_or(CommandError::OffsetSurfaceRequiresPlanarFaces)?;
    Ok(plane
        .normal()
        .as_vector()
        .scaled(if reversed { -1.0 } else { 1.0 })?)
}

fn solid_profile(geometry: &Geometry, tolerance: Tolerance) -> Result<NurbsCurve, CommandError> {
    let loops = match geometry {
        Geometry::NurbsSurface(surface) => surface.natural_boundary_curve_loops()?,
        Geometry::Brep(brep) => brep.face_boundary_curve_components(0)?,
        _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
    };
    let [boundary] = loops.as_slice() else {
        return Err(CommandError::OffsetSurfaceSolidRequiresOneLoop);
    };
    border::assemble(boundary.clone(), tolerance)?
        .nurbs_curve_representation()?
        .ok_or(CommandError::OffsetSurfaceSolidRequiresOneLoop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_surface_offsets_on_both_sides_and_undoes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "OffsetSrf 2 BothSides=Yes DeleteInput=Yes")
            .unwrap();
        let mut heights = document
            .objects()
            .map(|object| {
                let Geometry::NurbsSurface(surface) = object.geometry() else {
                    panic!("offset preserves a NURBS surface")
                };
                surface
                    .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
                    .unwrap()
                    .z()
            })
            .collect::<Vec<_>>();
        heights.sort_by(Real::total_cmp);
        assert_eq!(heights, [-2.0, 2.0]);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn solid_offset_closes_a_planar_surface_at_exact_volume() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry
            .execute(&mut document, "OffsetSrf 2 Solid=Yes")
            .unwrap();
        assert_eq!(document.objects().count(), 2);
        let Geometry::Brep(solid) = document.selected_objects().next().unwrap().geometry() else {
            panic!("solid offset is a B-rep")
        };
        assert!((solid.signed_volume(document.tolerance()).unwrap() - 24.0).abs() < 1e-9);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 2 Solid=Yes BothSides=Yes")
            .unwrap();
        assert_eq!(document.objects().count(), 2);
        let Geometry::Brep(solid) = document.selected_objects().next().unwrap().geometry() else {
            panic!("two-sided solid offset is one B-rep")
        };
        assert!((solid.signed_volume(document.tolerance()).unwrap() - 48.0).abs() < 1e-9);
    }

    #[test]
    fn trimmed_planar_brep_offset_follows_face_orientation() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Rectangle 0,0,0 4,3,0")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "PlanarSrf DeleteInput=Yes")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let source = document.objects().next().unwrap().geometry().clone();
        let normal = planar_normal(&source, document.tolerance()).unwrap();
        let Geometry::Brep(original) = &source else {
            panic!("planar source is a B-rep")
        };
        let surface = original.faces()[0].surface();
        let source_point = surface
            .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
            .unwrap();
        registry
            .execute(&mut document, "OffsetSrf -2 DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(offset) = document.objects().next().unwrap().geometry() else {
            panic!("trimmed offset remains a B-rep")
        };
        let surface = offset.faces()[0].surface();
        let offset_point = surface
            .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
            .unwrap();
        let displacement = source_point.vector_to(offset_point).unwrap();
        assert!((displacement.dot(normal).unwrap() + 2.0).abs() < 1e-10);
        assert_eq!(
            offset.faces()[0].loops().len(),
            original.faces()[0].loops().len()
        );
        registry
            .execute(&mut document, "OffsetSrf 1 Solid=Yes")
            .unwrap();
        let Geometry::Brep(solid) = document.selected_objects().next().unwrap().geometry() else {
            panic!("trimmed planar solid offset is a B-rep")
        };
        assert!((solid.signed_volume(document.tolerance()).unwrap() - 12.0).abs() < 1e-9);
    }

    #[test]
    fn unsupported_selection_does_not_create_partial_offsets() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0")
            .unwrap();
        registry.execute(&mut document, "Line 0,0,0 1,0,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut document, "OffsetSrf 2"),
            Err(CommandError::OffsetSurfaceRequiresPlanarFaces)
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);

        let mut warped = Document::default();
        registry
            .execute(&mut warped, "SrfPt 0,0,0 4,0,0 4,3,1 0,3,0")
            .unwrap();
        registry.execute(&mut warped, "SelAll").unwrap();
        let before = warped.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut warped, "OffsetSrf 2 Solid=Yes"),
            Err(CommandError::OffsetSurfaceRequiresPlanarFaces)
        ));
        assert_eq!(warped.objects().cloned().collect::<Vec<_>>(), before);
    }
}
