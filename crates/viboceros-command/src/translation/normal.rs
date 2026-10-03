//! Normal references use curve curvature and underlying surface orientation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalLocation {
    pub point: Point3,
    pub direction: UnitVector3,
}

/// Resolve a typed/snapped location on a chosen reference. B-rep face reversal
/// does not reverse Move Normal; the underlying surface orientation does.
pub fn normal_location(
    geometry: &Geometry,
    face: Option<usize>,
    pick: Point3,
    ignore_trims: bool,
    tolerance: Tolerance,
) -> Result<NormalLocation, CommandError> {
    if let Some(curve) = geometry.curve_ref() {
        if face.is_some() {
            return Err(CommandError::Usage(MOVE_NORMAL_USAGE));
        }
        let parameter = curve.closest_parameter(pick, tolerance)?;
        return normal_curve_location(curve, parameter);
    }
    let (surface, u, v) = match geometry {
        Geometry::NurbsSurface(surface) if face.is_none_or(|face| face == 0) => {
            let (u, v) = surface.closest_parameters(pick, tolerance)?;
            (surface, u, v)
        }
        Geometry::Brep(brep) => {
            let face = match face {
                Some(face) => face,
                None if brep.faces().len() == 1 => 0,
                None => {
                    brep.closest_face_parameters(pick, tolerance)?
                        .ok_or(CommandError::Usage(MOVE_NORMAL_USAGE))?
                        .0
                }
            };
            let surface = brep
                .faces()
                .get(face)
                .ok_or(CommandError::Usage(MOVE_NORMAL_USAGE))?
                .surface();
            let (u, v) = if ignore_trims {
                surface.closest_parameters(pick, tolerance)?
            } else {
                brep.closest_parameters_on_face(face, pick, tolerance)?
                    .ok_or(CommandError::Usage(MOVE_NORMAL_USAGE))?
            };
            (surface, u, v)
        }
        _ => return Err(CommandError::Usage(MOVE_NORMAL_USAGE)),
    };
    Ok(NormalLocation {
        point: surface.evaluate(u, v)?,
        direction: surface.normal_at(u, v)?,
    })
}

pub fn normal_curve_location(
    curve: CurveRef<'_>,
    parameter: Real,
) -> Result<NormalLocation, CommandError> {
    Ok(NormalLocation {
        point: curve.evaluate(parameter)?,
        direction: curve.curvature_vector(parameter)?.normalized_nonzero()?,
    })
}

pub const MOVE_NORMAL_USAGE: &str =
    "Move Normal=reference-id base distance|target [Face=index] [IgnoreTrims=Yes|No]";

pub(super) fn take_normal_arguments(
    arguments: &mut Vec<&str>,
    default_ignore_trims: bool,
) -> Result<Option<(ObjectId, Option<usize>, bool)>, CommandError> {
    let mut reference = None;
    let mut face = None;
    let mut ignore_trims = None;
    let mut invalid = false;
    arguments.retain(|argument| {
        let Some((name, value)) = argument.split_once('=') else {
            return true;
        };
        if option_name_eq(name, "Normal") {
            let id = value.parse::<ObjectId>().ok();
            invalid |= id.is_none() || reference.is_some();
            reference = id;
            false
        } else if option_name_eq(name, "Face") {
            let index = value.parse::<usize>().ok();
            invalid |= index.is_none() || face.is_some();
            face = index;
            false
        } else if option_name_eq(name, "IgnoreTrims") {
            let value = parse_yes_no(value);
            invalid |= value.is_none() || ignore_trims.is_some();
            ignore_trims = value;
            false
        } else {
            true
        }
    });
    if invalid || reference.is_none() && (face.is_some() || ignore_trims.is_some()) {
        return Err(CommandError::Usage(MOVE_NORMAL_USAGE));
    }
    Ok(reference.map(|id| (id, face, ignore_trims.unwrap_or(default_ignore_trims))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn failed_normal_edits_keep_geometry_history_and_move_preferences() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Point(point(2., 3., 4.)))
            .unwrap();
        let reference = document
            .add_geometry(Geometry::Circle(
                Circle3::try_new(
                    point(0., 0., 0.),
                    3.,
                    CommandContext::default().construction_plane.z_axis(),
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Move 0,0,0 3,4,0").unwrap();
        assert_eq!(registry.transform_scalar_default("Move"), Some(5.));
        document.clear_history().unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        for arguments in [
            "3,0,0 NaN",
            "3,0,0 1 Face=99",
            "3,0,0 1 Vertical",
            "3,0,0 1 IgnoreTrims=Maybe",
            "3,0,0 1 IgnoreTrims=Yes extra",
            "3,0,0 1 Face=0 Face=0",
            "3,0,0 1 IgnoreTrims=No IgnoreTrims=Yes",
        ] {
            assert!(
                registry
                    .execute(
                        &mut document,
                        &format!("Move Normal={reference} {arguments}")
                    )
                    .is_err(),
                "{arguments}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(document.is_selected(source));
            assert!(!document.can_undo());
            assert_eq!(registry.transform_scalar_default("Move"), Some(5.));
            assert!(
                !registry
                    .object_selection_prompt("Move Normal")
                    .unwrap()
                    .unwrap()
                    .options[0]
                    .value
            );
        }
        registry
            .execute(
                &mut document,
                &format!("Move Normal={reference} 3,0,0 -2 IgnoreTrims=Yes"),
            )
            .unwrap();
        assert_eq!(registry.transform_scalar_default("Move"), Some(2.));
        assert!(
            registry
                .object_selection_prompt("Move Normal")
                .unwrap()
                .unwrap()
                .options[0]
                .value
        );
    }

    #[test]
    fn a_selected_normal_reference_moves_once_using_its_original_curvature() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Point(point(2., 3., 4.)))
            .unwrap();
        let reference = document
            .add_geometry(Geometry::Circle(
                Circle3::try_new(
                    point(0., 0., 0.),
                    3.,
                    CommandContext::default().construction_plane.z_axis(),
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        document
            .select_objects_direct([source, reference], SelectionMode::Replace)
            .unwrap();
        document.clear_history().unwrap();
        registry
            .execute(&mut document, &format!("Move Normal={reference} 3,0,0 2"))
            .unwrap();
        assert_eq!(document.objects().len(), 2);
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Point(point(0., 3., 4.))
        );
        let Geometry::Circle(circle) = document.object(reference).unwrap().geometry() else {
            panic!("circle")
        };
        assert_eq!(circle.center(), point(-2., 0., 0.));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Point(point(2., 3., 4.))
        );
        assert!(!document.can_undo());
    }
}
