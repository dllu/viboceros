//! Exact normal offsets of planar and canonical analytic surfaces.

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
        let mut staged = Vec::with_capacity(selected.len());
        for (id, geometry) in &selected {
            if let Some(offset) = spherical_offset(geometry, options, document.tolerance())? {
                staged.push((*id, offset));
                continue;
            }
            if let Some(offset) = cylindrical_offset(geometry, options, document.tolerance())? {
                staged.push((*id, offset));
                continue;
            }
            if let Some(offset) = toroidal_offset(geometry, options, document.tolerance())? {
                staged.push((*id, offset));
                continue;
            }
            if let Some(offset) = conical_offset(geometry, options, document.tolerance())? {
                staged.push((*id, offset));
                continue;
            }
            let normal = planar_normal(geometry, document.tolerance())?;
            if options.solid {
                let solid = if let Some(box_solid) = rectangular_solid(
                    geometry,
                    normal,
                    options.distance,
                    options.both_sides,
                    document.tolerance(),
                )? {
                    box_solid
                } else {
                    let profile = solid_profile(geometry, document.tolerance())?;
                    Brep::try_extruded_curve(
                        &profile,
                        normal.scaled(if options.both_sides {
                            -options.distance
                        } else {
                            0.0
                        })?,
                        normal.scaled(options.distance)?,
                        document.tolerance(),
                    )?
                };
                staged.push((*id, Geometry::Brep(solid)));
                continue;
            }
            if options.both_sides {
                let mut parts = Vec::with_capacity(2);
                for signed_distance in [options.distance, -options.distance] {
                    let offset = normal.scaled(signed_distance)?;
                    let moved = geometry.transformed(
                        AffineTransform3::from_translation(offset),
                        document.tolerance(),
                    )?;
                    parts.push(match moved {
                        Geometry::NurbsSurface(surface) => {
                            Brep::try_surface_face(surface, document.tolerance())?
                        }
                        Geometry::Brep(brep) => brep,
                        _ => unreachable!("only surface geometry reaches an offset"),
                    });
                }
                staged.push((
                    *id,
                    Geometry::Brep(Brep::try_disjoint_union(parts, document.tolerance())?),
                ));
                continue;
            }
            let offset = normal.scaled(options.distance)?;
            let result = geometry.transformed(
                AffineTransform3::from_translation(offset),
                document.tolerance(),
            )?;
            staged.push((*id, result));
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

fn spherical_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((center, radius)) = surface.canonical_sphere(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let u = surface.parameter_at_u(0.125)?;
    let v = surface.parameter_at_v(0.5)?;
    let radial = center.vector_to(surface.evaluate(u, v)?)?;
    let natural_outward = radial.dot(surface.normal_at(u, v)?.as_vector())? > 0.0;
    let direction = if natural_outward ^ reversed {
        1.0
    } else {
        -1.0
    };
    let new_radius = radius + direction * options.distance;
    let opposite_radius = radius - direction * options.distance;
    if new_radius <= tolerance.absolute()
        || (options.both_sides && opposite_radius <= tolerance.absolute())
    {
        return Err(CommandError::OffsetSurfaceCollapsedSphere);
    }
    let scaled = |target_radius: Real| -> Result<NurbsSurface, CommandError> {
        Ok(surface.transformed(AffineTransform3::try_uniform_scale(
            center,
            target_radius / radius,
        )?)?)
    };
    let face = |target_radius: Real, outward: bool| -> Result<Brep, CommandError> {
        let mut result = Brep::try_surface_face(scaled(target_radius)?, tolerance)?;
        if natural_outward != outward {
            result.reverse_orientation();
        }
        Ok(result)
    };
    if options.solid {
        let (inner, outer) = if options.both_sides {
            (
                new_radius.min(opposite_radius),
                new_radius.max(opposite_radius),
            )
        } else {
            (new_radius.min(radius), new_radius.max(radius))
        };
        let result =
            Brep::try_disjoint_union(vec![face(outer, true)?, face(inner, false)?], tolerance)?;
        return Ok(Some(Geometry::Brep(result)));
    }
    if options.both_sides {
        let mut positive = face(new_radius, natural_outward)?;
        let mut negative = face(opposite_radius, natural_outward)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = scaled(new_radius)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

fn cylindrical_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((frame, radius, height)) = surface.canonical_cylinder(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let u = surface.parameter_at_u(0.125)?;
    let v = surface.parameter_at_v(0.5)?;
    let axis_point = frame.point_at([0.0, 0.0, height * 0.5])?;
    let radial = axis_point.vector_to(surface.evaluate(u, v)?)?;
    let natural_outward = radial.dot(surface.normal_at(u, v)?.as_vector())? > 0.0;
    let direction = if natural_outward ^ reversed {
        1.0
    } else {
        -1.0
    };
    let new_radius = radius + direction * options.distance;
    let opposite_radius = radius - direction * options.distance;
    if new_radius <= tolerance.absolute()
        || (options.both_sides && opposite_radius <= tolerance.absolute())
    {
        return Err(CommandError::OffsetSurfaceCollapsedCylinder);
    }
    if options.solid {
        let radii = if options.both_sides {
            [new_radius, opposite_radius]
        } else {
            [radius, new_radius]
        };
        return Ok(Some(Geometry::Brep(Brep::try_offset_cylinder_tube(
            frame, radii, height, tolerance,
        )?)));
    }
    let scaled = |target_radius: Real| -> Result<NurbsSurface, CommandError> {
        Ok(surface.transformed(AffineTransform3::try_frame_mapping(
            frame,
            frame,
            [target_radius / radius, target_radius / radius, 1.0],
        )?)?)
    };
    if options.both_sides {
        let mut positive = Brep::try_surface_face(scaled(new_radius)?, tolerance)?;
        let mut negative = Brep::try_surface_face(scaled(opposite_radius)?, tolerance)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = scaled(new_radius)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

fn toroidal_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((frame, major_radius, minor_radius)) = surface.canonical_torus(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let u = surface.parameter_at_u(0.125)?;
    let v = surface.parameter_at_v(0.125)?;
    let point = surface.evaluate(u, v)?;
    let local = frame.coordinates_of(point)?;
    let radial_length = local[0].hypot(local[1]);
    if radial_length <= tolerance.absolute() {
        return Err(CommandError::OffsetSurfaceCollapsedTorus);
    }
    let major_point = frame.point_at([
        major_radius * local[0] / radial_length,
        major_radius * local[1] / radial_length,
        0.0,
    ])?;
    let radial = major_point.vector_to(point)?;
    let natural_outward = radial.dot(surface.normal_at(u, v)?.as_vector())? > 0.0;
    let direction = if natural_outward ^ reversed {
        1.0
    } else {
        -1.0
    };
    let new_minor = minor_radius + direction * options.distance;
    let opposite_minor = minor_radius - direction * options.distance;
    if new_minor <= tolerance.absolute()
        || new_minor >= major_radius
        || (options.both_sides
            && (opposite_minor <= tolerance.absolute() || opposite_minor >= major_radius))
    {
        return Err(CommandError::OffsetSurfaceCollapsedTorus);
    }
    let torus = |target_minor: Real| -> Result<NurbsSurface, CommandError> {
        Ok(NurbsSurface::try_torus(frame, major_radius, target_minor)?
            .try_reparameterized(surface.domain_u(), surface.domain_v())?)
    };
    let face = |target_minor: Real, outward: bool| -> Result<Brep, CommandError> {
        let mut result = Brep::try_surface_face(torus(target_minor)?, tolerance)?;
        if natural_outward != outward {
            result.reverse_orientation();
        }
        Ok(result)
    };
    if options.solid {
        let (inner, outer) = if options.both_sides {
            (new_minor.min(opposite_minor), new_minor.max(opposite_minor))
        } else {
            (new_minor.min(minor_radius), new_minor.max(minor_radius))
        };
        let result =
            Brep::try_disjoint_union(vec![face(outer, true)?, face(inner, false)?], tolerance)?;
        return Ok(Some(Geometry::Brep(result)));
    }
    if options.both_sides {
        let mut positive = face(new_minor, natural_outward)?;
        let mut negative = face(opposite_minor, natural_outward)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = torus(new_minor)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
}

fn conical_offset(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    let (surface, reversed, brep_source) = match geometry {
        Geometry::NurbsSurface(surface) => (surface, false, false),
        Geometry::Brep(brep) if brep.faces().len() == 1 => {
            let face = &brep.faces()[0];
            (face.surface(), face.is_reversed(), true)
        }
        _ => return Ok(None),
    };
    let Some((_, _, height)) = surface.canonical_cone(tolerance)? else {
        return Ok(None);
    };
    if let Geometry::Brep(brep) = geometry
        && !brep.faces()[0].is_untrimmed(tolerance)?
    {
        return Ok(None);
    }
    let shifted = |distance: Real| -> Result<NurbsSurface, CommandError> {
        Ok(surface
            .try_offset_canonical_cone(if reversed { -distance } else { distance }, tolerance)?
            .expect("canonical cone was checked above"))
    };
    if options.solid {
        let positive = shifted(options.distance)?;
        let negative = if options.both_sides {
            shifted(-options.distance)?
        } else {
            surface.clone()
        };
        let apex_row = usize::from(height < 0.0);
        let base_row = 1 - apex_row;
        let apex_cap = negative
            .try_cone_offset_cap(&positive, apex_row)?
            .expect("matching offset cone nets");
        let base_cap = negative
            .try_cone_offset_cap(&positive, base_row)?
            .expect("matching offset cone nets");
        let parts = [positive, negative, apex_cap, base_cap]
            .into_iter()
            .map(|patch| Brep::try_surface_face(patch, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let joined = viboceros_geometry::join_breps(
            &parts.iter().collect::<Vec<_>>(),
            tolerance.absolute(),
            tolerance,
        )?;
        if joined.len() != 1 {
            return Err(CommandError::OffsetSurfaceRequiresPlanarFaces);
        }
        return Ok(Some(Geometry::Brep(
            joined
                .into_iter()
                .next()
                .unwrap()
                .brep
                .try_weld_coincident_vertices(tolerance)?,
        )));
    }
    if options.both_sides {
        let mut positive = Brep::try_surface_face(shifted(options.distance)?, tolerance)?;
        let mut negative = Brep::try_surface_face(shifted(-options.distance)?, tolerance)?;
        if reversed {
            positive.reverse_orientation();
            negative.reverse_orientation();
        }
        return Ok(Some(Geometry::Brep(Brep::try_disjoint_union(
            vec![positive, negative],
            tolerance,
        )?)));
    }
    let result = shifted(options.distance)?;
    if brep_source {
        let mut result = Brep::try_surface_face(result, tolerance)?;
        if reversed {
            result.reverse_orientation();
        }
        Ok(Some(Geometry::Brep(result)))
    } else {
        Ok(Some(Geometry::NurbsSurface(result)))
    }
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

fn rectangular_solid(
    geometry: &Geometry,
    normal: Vector3,
    distance: Real,
    both_sides: bool,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    let Geometry::NurbsSurface(surface) = geometry else {
        return Ok(None);
    };
    if surface.degree_u() != 1
        || surface.degree_v() != 1
        || surface.control_point_count_u() != 2
        || surface.control_point_count_v() != 2
    {
        return Ok(None);
    }
    let u = surface.domain_u();
    let v = surface.domain_v();
    let origin = surface.evaluate(*u.start(), *v.start())?;
    let east = surface.evaluate(*u.end(), *v.start())?;
    let north = surface.evaluate(*u.start(), *v.end())?;
    let opposite = surface.evaluate(*u.end(), *v.end())?;
    let frame = Frame3::try_from_points(origin, east, north, tolerance)?;
    let east_local = frame.coordinates_of(east)?;
    let north_local = frame.coordinates_of(north)?;
    let opposite_local = frame.coordinates_of(opposite)?;
    let width = east_local[0];
    let height = north_local[1];
    let residuals = [
        east_local[1],
        east_local[2],
        north_local[0],
        north_local[2],
        opposite_local[0] - width,
        opposite_local[1] - height,
        opposite_local[2],
    ];
    if width <= tolerance.absolute()
        || height <= tolerance.absolute()
        || residuals
            .iter()
            .any(|value| value.abs() > tolerance.absolute())
    {
        return Ok(None);
    }
    let signed_height = distance * normal.dot(frame.z_axis().as_vector())?;
    let z = if both_sides {
        [-signed_height.abs(), signed_height.abs()]
    } else {
        [signed_height.min(0.0), signed_height.max(0.0)]
    };
    Ok(Some(Brep::try_box(
        frame,
        [[0.0, width], [0.0, height], z],
        tolerance,
    )?))
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
        assert_eq!(document.objects().count(), 1);
        let Geometry::Brep(offset) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided offset is one disjoint B-rep")
        };
        assert_eq!(offset.faces().len(), 2);
        assert_eq!(offset.edges().len(), 8);
        assert_eq!(offset.vertices().len(), 8);
        assert!(offset.faces().iter().all(|face| !face.is_reversed()));
        let mut heights = offset
            .faces()
            .iter()
            .map(|face| {
                let surface = face.surface();
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
        assert_eq!(
            (
                solid.faces().len(),
                solid.edges().len(),
                solid.vertices().len()
            ),
            (6, 12, 8)
        );
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
        assert_eq!(
            (
                solid.faces().len(),
                solid.edges().len(),
                solid.vertices().len()
            ),
            (6, 12, 8)
        );
        assert!((solid.signed_volume(document.tolerance()).unwrap() - 48.0).abs() < 1e-9);

        registry.execute(&mut document, "Undo").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf -2 Solid=Yes")
            .unwrap();
        let Geometry::Brep(solid) = document.selected_objects().next().unwrap().geometry() else {
            panic!("negative solid offset is a B-rep")
        };
        assert_eq!(
            (
                solid.faces().len(),
                solid.edges().len(),
                solid.vertices().len()
            ),
            (6, 12, 8)
        );
        assert!((solid.signed_volume(document.tolerance()).unwrap() - 24.0).abs() < 1e-9);
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
    fn two_sided_trimmed_face_keeps_both_trim_loops_in_one_brep() {
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
        let Geometry::Brep(source) = document.objects().next().unwrap().geometry() else {
            panic!("planar source is a B-rep")
        };
        let source_loop_count = source.faces()[0].loops().len();
        let source_edge_count = source.edges().len();
        registry
            .execute(&mut document, "OffsetSrf 2 BothSides=Yes DeleteInput=Yes")
            .unwrap();
        assert_eq!(document.objects().count(), 1);
        let Geometry::Brep(offset) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided trimmed offset is a B-rep")
        };
        assert_eq!(offset.faces().len(), 2);
        assert!(
            offset
                .faces()
                .iter()
                .all(|face| face.loops().len() == source_loop_count)
        );
        assert_eq!(offset.edges().len(), source_edge_count * 2);
    }

    #[test]
    fn vertical_rectangle_offsets_to_six_face_solid() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "SrfPt 0,0,0 4,0,0 4,0,3 0,0,3")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "OffsetSrf -2 Solid=Yes BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        let Geometry::Brep(solid) = document.objects().next().unwrap().geometry() else {
            panic!("vertical offset is a B-rep")
        };
        assert_eq!(
            (
                solid.faces().len(),
                solid.edges().len(),
                solid.vertices().len()
            ),
            (6, 12, 8)
        );
        assert!((solid.signed_volume(document.tolerance()).unwrap() - 48.0).abs() < 1e-9);
    }

    #[test]
    fn exact_sphere_offsets_match_rhino_shell_volumes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Sphere 1,2,3 2").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let source = document.objects().cloned().collect::<Vec<_>>();

        registry
            .execute(&mut document, "OffsetSrf 0.5 DeleteInput=Yes")
            .unwrap();
        let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry() else {
            panic!("open spherical offset is a NURBS surface")
        };
        let (_, radius) = offset
            .canonical_sphere(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((radius - 2.5).abs() < 1e-10);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), source);

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf -0.5 DeleteInput=Yes")
            .unwrap();
        let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry() else {
            panic!("inward spherical offset is a NURBS surface")
        };
        let (_, radius) = offset
            .canonical_sphere(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((radius - 1.5).abs() < 1e-10);
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.5 Solid=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
            panic!("one-sided sphere solid offset is a B-rep")
        };
        assert_eq!((shell.faces().len(), shell.edges().len()), (2, 2));
        let one_side_volume =
            4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) - 2.0_f64.powi(3));
        assert!(
            (shell.signed_volume(document.tolerance()).unwrap() - one_side_volume).abs() < 1e-7
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.5 BothSides=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(open_pair) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided sphere offset is a B-rep")
        };
        assert_eq!((open_pair.faces().len(), open_pair.edges().len()), (2, 2));
        let summed_volume = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) + 1.5_f64.powi(3));
        assert!(
            (open_pair.signed_volume(document.tolerance()).unwrap() - summed_volume).abs() < 1e-7
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "OffsetSrf 0.5 Solid=Yes BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
            panic!("spherical solid offset is a B-rep")
        };
        assert_eq!((shell.faces().len(), shell.edges().len()), (2, 2));
        let shell_volume = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) - 1.5_f64.powi(3));
        assert!((shell.signed_volume(document.tolerance()).unwrap() - shell_volume).abs() < 1e-7);
    }

    #[test]
    fn collapsing_sphere_offset_is_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Sphere 1,2,3 2").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut document, "OffsetSrf -2.5 DeleteInput=Yes"),
            Err(CommandError::OffsetSurfaceCollapsedSphere)
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn untrimmed_spherical_brep_offsets_as_exact_shell() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        let surface = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let source = Brep::try_surface_face(surface, document.tolerance()).unwrap();
        document.add_geometry(Geometry::Brep(source)).unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.5 Solid=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
            panic!("spherical B-rep offset is an exact shell")
        };
        assert_eq!(shell.faces().len(), 2);
        let expected = 4.0 * std::f64::consts::PI / 3.0 * (2.5_f64.powi(3) - 2.0_f64.powi(3));
        assert!((shell.signed_volume(document.tolerance()).unwrap() - expected).abs() < 1e-7);
    }

    #[test]
    fn reversed_spherical_face_offsets_inward_along_its_normal() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let surface = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let mut source = Brep::try_surface_face(surface, tolerance).unwrap();
        source.reverse_orientation();
        let options = parse(&["0.5"], tolerance).unwrap();
        let Some(Geometry::Brep(offset)) =
            spherical_offset(&Geometry::Brep(source), options, tolerance).unwrap()
        else {
            panic!("reversed spherical face offsets as a B-rep")
        };
        let (_, radius) = offset.faces()[0]
            .surface()
            .canonical_sphere(tolerance)
            .unwrap()
            .unwrap();
        assert!((radius - 1.5).abs() < 1e-10);
        assert!(offset.faces()[0].is_reversed());
    }

    #[test]
    fn exact_cylinder_offsets_match_rhino_wall_and_tube_topology() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Cylinder 1,2,3 2 3 Solid=No")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let source = document.objects().cloned().collect::<Vec<_>>();

        registry
            .execute(&mut document, "OffsetSrf 0.5 DeleteInput=Yes")
            .unwrap();
        let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry() else {
            panic!("open cylindrical offset is a NURBS surface")
        };
        let (_, radius, height) = offset
            .canonical_cylinder(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((radius - 2.5).abs() < 1e-10);
        assert!((height - 3.0).abs() < 1e-10);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), source);

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf -0.5 DeleteInput=Yes")
            .unwrap();
        let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry() else {
            panic!("inward cylindrical offset is a NURBS surface")
        };
        let (_, radius, _) = offset
            .canonical_cylinder(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((radius - 1.5).abs() < 1e-10);
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.5 BothSides=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(open_pair) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided cylindrical offset is a B-rep")
        };
        assert_eq!(
            (
                open_pair.faces().len(),
                open_pair.edges().len(),
                open_pair.vertices().len()
            ),
            (2, 6, 4)
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.5 Solid=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(tube) = document.objects().next().unwrap().geometry() else {
            panic!("cylindrical solid offset is a tube")
        };
        assert!(
            (tube.signed_volume(document.tolerance()).unwrap() - 6.75 * std::f64::consts::PI).abs()
                < 1e-7
        );
        assert_eq!(
            (
                tube.faces().len(),
                tube.edges().len(),
                tube.vertices().len()
            ),
            (4, 8, 4)
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "OffsetSrf 0.5 Solid=Yes BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        let Geometry::Brep(tube) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided cylindrical solid offset is a tube")
        };
        assert!(
            (tube.signed_volume(document.tolerance()).unwrap() - 12.0 * std::f64::consts::PI).abs()
                < 1e-7
        );
        assert_eq!(
            (
                tube.faces().len(),
                tube.edges().len(),
                tube.vertices().len()
            ),
            (4, 8, 4)
        );
    }

    #[test]
    fn collapsing_cylinder_offset_is_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Cylinder 1,2,3 2 3 Solid=No")
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            registry.execute(&mut document, "OffsetSrf -2.5 DeleteInput=Yes"),
            Err(CommandError::OffsetSurfaceCollapsedCylinder)
        ));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }

    #[test]
    fn reversed_cylindrical_brep_offsets_inward() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        let surface = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 3.0).unwrap();
        let mut source = Brep::try_surface_face(surface, document.tolerance()).unwrap();
        source.reverse_orientation();
        document.add_geometry(Geometry::Brep(source)).unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.5 DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(offset) = document.objects().next().unwrap().geometry() else {
            panic!("cylindrical B-rep offset remains a B-rep")
        };
        let (_, radius, height) = offset.faces()[0]
            .surface()
            .canonical_cylinder(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((radius - 1.5).abs() < 1e-10);
        assert!((height - 3.0).abs() < 1e-10);
        assert!(offset.faces()[0].is_reversed());
    }

    #[test]
    fn exact_torus_offsets_keep_source_domains_and_shell_topology() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Torus 1,2,3 4 1").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let source = document.objects().cloned().collect::<Vec<_>>();
        let Geometry::NurbsSurface(original) = source[0].geometry() else {
            panic!("torus source is a NURBS surface")
        };
        let domains = (original.domain_u(), original.domain_v());

        registry
            .execute(&mut document, "OffsetSrf 0.25 DeleteInput=Yes")
            .unwrap();
        let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry() else {
            panic!("open toroidal offset is a NURBS surface")
        };
        let (_, major, minor) = offset
            .canonical_torus(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((major - 4.0).abs() < 1e-10);
        assert!((minor - 1.25).abs() < 1e-10);
        assert_eq!((offset.domain_u(), offset.domain_v()), domains);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), source);

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf -0.25 DeleteInput=Yes")
            .unwrap();
        let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry() else {
            panic!("inward toroidal offset is a NURBS surface")
        };
        let (_, _, minor) = offset
            .canonical_torus(document.tolerance())
            .unwrap()
            .unwrap();
        assert!((minor - 0.75).abs() < 1e-10);
        assert_eq!((offset.domain_u(), offset.domain_v()), domains);
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "OffsetSrf 0.25 BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        let Geometry::Brep(open_pair) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided toroidal offset is a B-rep")
        };
        assert_eq!(
            (
                open_pair.faces().len(),
                open_pair.edges().len(),
                open_pair.vertices().len()
            ),
            (2, 4, 2)
        );
        assert!(
            open_pair
                .faces()
                .iter()
                .all(|face| { (face.surface().domain_u(), face.surface().domain_v()) == domains })
        );
        let summed_volume =
            2.0 * std::f64::consts::PI.powi(2) * 4.0 * (1.25_f64.powi(2) + 0.75_f64.powi(2));
        assert!(
            (open_pair.signed_volume(document.tolerance()).unwrap() - summed_volume).abs() < 1e-6
        );
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.25 Solid=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
            panic!("toroidal solid offset is a B-rep")
        };
        assert_eq!(
            (
                shell.faces().len(),
                shell.edges().len(),
                shell.vertices().len()
            ),
            (2, 4, 2)
        );
        let shell_volume = 2.0 * std::f64::consts::PI.powi(2) * 4.0 * (1.25_f64.powi(2) - 1.0);
        assert!((shell.signed_volume(document.tolerance()).unwrap() - shell_volume).abs() < 1e-6);
        registry.execute(&mut document, "Undo").unwrap();

        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "OffsetSrf 0.25 Solid=Yes BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided toroidal solid offset is a B-rep")
        };
        assert_eq!(
            (
                shell.faces().len(),
                shell.edges().len(),
                shell.vertices().len()
            ),
            (2, 4, 2)
        );
        let shell_volume =
            2.0 * std::f64::consts::PI.powi(2) * 4.0 * (1.25_f64.powi(2) - 0.75_f64.powi(2));
        assert!((shell.signed_volume(document.tolerance()).unwrap() - shell_volume).abs() < 1e-6);
    }

    #[test]
    fn collapsing_or_self_intersecting_torus_offset_is_atomic() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Torus 1,2,3 4 1").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        for distance in ["-1.25", "3.5"] {
            assert!(matches!(
                registry.execute(
                    &mut document,
                    &format!("OffsetSrf {distance} DeleteInput=Yes")
                ),
                Err(CommandError::OffsetSurfaceCollapsedTorus)
            ));
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }

    #[test]
    fn reversed_toroidal_face_offsets_inward_along_its_normal() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let surface = NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap();
        let mut source = Brep::try_surface_face(surface, tolerance).unwrap();
        source.reverse_orientation();
        let options = parse(&["0.25"], tolerance).unwrap();
        let Some(Geometry::Brep(offset)) =
            toroidal_offset(&Geometry::Brep(source), options, tolerance).unwrap()
        else {
            panic!("reversed toroidal face offsets as a B-rep")
        };
        let (_, major, minor) = offset.faces()[0]
            .surface()
            .canonical_torus(tolerance)
            .unwrap()
            .unwrap();
        assert!((major - 4.0).abs() < 1e-10);
        assert!((minor - 0.75).abs() < 1e-10);
        assert!(offset.faces()[0].is_reversed());
    }

    #[test]
    fn exact_cone_offsets_match_rhino_open_face_geometry() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap(),
            ))
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        for (distance, apex_x, apex_z) in [
            (0.25, 1.208012573584461, 2.861324950943693),
            (-0.25, 0.791987426415539, 3.138675049056307),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("OffsetSrf {distance} DeleteInput=Yes"),
                )
                .unwrap();
            let Geometry::NurbsSurface(offset) = document.objects().next().unwrap().geometry()
            else {
                panic!("open cone offset is a NURBS surface")
            };
            let apex = offset.control_points()[0].point().to_array();
            assert!((apex[0] - apex_x).abs() < 1e-12);
            assert!((apex[2] - apex_z).abs() < 1e-12);
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        }
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(
                &mut document,
                "OffsetSrf 0.25 BothSides=Yes DeleteInput=Yes",
            )
            .unwrap();
        let Geometry::Brep(pair) = document.objects().next().unwrap().geometry() else {
            panic!("two-sided cone offset is a B-rep")
        };
        assert_eq!(
            (
                pair.faces().len(),
                pair.edges().len(),
                pair.vertices().len()
            ),
            (2, 6, 4)
        );
    }

    #[test]
    fn reversed_cone_face_offsets_along_its_reversed_normal() {
        let tolerance = Tolerance::DEFAULT;
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            tolerance,
        )
        .unwrap();
        let surface = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        let mut source = Brep::try_surface_face(surface, tolerance).unwrap();
        source.reverse_orientation();
        let options = parse(&["0.25"], tolerance).unwrap();
        let Some(Geometry::Brep(offset)) =
            conical_offset(&Geometry::Brep(source), options, tolerance).unwrap()
        else {
            panic!("reversed cone face offsets as a B-rep")
        };
        assert!(offset.faces()[0].is_reversed());
        let apex = offset.faces()[0].surface().control_points()[0].point();
        assert!((apex.x() - 0.791987426415539).abs() < 1e-12);
    }

    #[test]
    fn exact_cone_solid_offsets_match_rhino_shell_topology_and_volume() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap(),
            ))
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        for (command, edges, volume) in [
            (
                "OffsetSrf 0.25 Solid=Yes DeleteInput=Yes",
                7,
                6.252635399278535,
            ),
            (
                "OffsetSrf -0.25 Solid=Yes DeleteInput=Yes",
                7,
                5.074538139644014,
            ),
            (
                "OffsetSrf 0.25 Solid=Yes BothSides=Yes DeleteInput=Yes",
                8,
                11.327173538922558,
            ),
        ] {
            registry.execute(&mut document, command).unwrap();
            let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
                panic!("solid cone offset is a B-rep")
            };
            assert_eq!(
                (
                    shell.faces().len(),
                    shell.edges().len(),
                    shell.vertices().len()
                ),
                (4, edges, 4)
            );
            assert!((shell.signed_volume(document.tolerance()).unwrap() - volume).abs() < 1e-6);
            registry.execute(&mut document, "Undo").unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
        }
    }

    #[test]
    fn negative_height_cone_solid_offset_closes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            document.tolerance(),
        )
        .unwrap();
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 2.0, -3.0).unwrap(),
            ))
            .unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry
            .execute(&mut document, "OffsetSrf 0.25 Solid=Yes DeleteInput=Yes")
            .unwrap();
        let Geometry::Brep(shell) = document.objects().next().unwrap().geometry() else {
            panic!("solid cone offset is a B-rep")
        };
        assert_eq!(
            (
                shell.faces().len(),
                shell.edges().len(),
                shell.vertices().len()
            ),
            (4, 7, 4)
        );
        assert!(
            (shell.signed_volume(document.tolerance()).unwrap() - 6.252635399278535).abs() < 1e-6
        );
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
