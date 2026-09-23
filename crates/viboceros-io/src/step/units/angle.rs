//! Normalize angular STEP values that the Monstertruck table reads as radians.
use super::*;

type AngularAxes = [bool; 2];
const NO_ANGLES: AngularAxes = [false, false];
const U_ANGLE: AngularAxes = [true, false];
const BOTH_ANGLES: AngularAxes = [true, true];

fn assign_axes(
    usage: &mut BTreeMap<u64, AngularAxes>,
    id: u64,
    axes: AngularAxes,
) -> Result<(), StepError> {
    if usage
        .insert(id, axes)
        .is_some_and(|previous| previous != axes)
    {
        return Err(invalid(
            "parameter geometry is shared across incompatible angular axes",
        ));
    }
    Ok(())
}

fn entity_records(entity: &EntityInstance) -> &[Record] {
    match entity {
        EntityInstance::Simple { record, .. } => std::slice::from_ref(record),
        EntityInstance::Complex { subsuper, .. } => &subsuper.0,
    }
}

fn entity_records_mut(entity: &mut EntityInstance) -> &mut [Record] {
    match entity {
        EntityInstance::Simple { record, .. } => std::slice::from_mut(record),
        EntityInstance::Complex { subsuper, .. } => &mut subsuper.0,
    }
}

fn record<'a>(resolver: &'a Resolver<'_>, id: u64, name: &str) -> Result<&'a Record, StepError> {
    resolver
        .entities
        .get(&id)
        .and_then(|records| component(records, name))
        .ok_or_else(|| invalid("missing angular parameter geometry"))
}

fn number(parameter: &Parameter) -> Result<f64, StepError> {
    let value = match parameter {
        Parameter::Real(value) => *value,
        Parameter::Integer(value) => *value as f64,
        _ => return Err(invalid("angular geometry parameter is not numeric")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid("angular geometry parameter is not finite"))
    }
}

fn scale_number(parameter: &mut Parameter, factor: f64) -> Result<(), StepError> {
    let original = number(parameter)?;
    let value = original * factor;
    if !value.is_finite() {
        return Err(invalid("angular geometry conversion overflows"));
    }
    if original != 0.0 && value == 0.0 {
        return Err(invalid("angular geometry conversion underflows"));
    }
    *parameter = Parameter::Real(value);
    Ok(())
}

fn indexed_record_mut<'a>(
    data: &'a mut DataSection,
    indices: &HashMap<u64, usize>,
    id: u64,
    name: &str,
) -> Result<&'a mut Record, StepError> {
    let index = *indices
        .get(&id)
        .ok_or_else(|| invalid("missing angular parameter geometry"))?;
    entity_records_mut(&mut data.entities[index])
        .iter_mut()
        .find(|record| record.name == name)
        .ok_or_else(|| invalid("missing angular parameter geometry"))
}

fn unsupported_angular_geometry(data: &DataSection) -> bool {
    data.entities.iter().flat_map(entity_records).any(|record| {
        let name = record.name.as_str();
        matches!(name, "HYPERBOLA" | "PARABOLA" | "TRIMMED_CURVE")
            || (name.ends_with("_SURFACE")
                && !angle_independent_surface(name)
                && !matches!(
                    name,
                    "CYLINDRICAL_SURFACE"
                        | "CONICAL_SURFACE"
                        | "SPHERICAL_SURFACE"
                        | "TOROIDAL_SURFACE"
                ))
            || (name.starts_with("SURFACE_OF_") && name != "SURFACE_OF_REVOLUTION")
            || (name.contains("REVOL") && name != "SURFACE_OF_REVOLUTION")
            || name.contains("CIRCULAR")
    })
}

fn references_any(parameter: &Parameter, ids: &BTreeSet<u64>) -> bool {
    match parameter {
        Parameter::Ref(Name::Entity(id)) => ids.contains(id),
        Parameter::List(items) => items.iter().any(|item| references_any(item, ids)),
        Parameter::Typed { parameter, .. } => references_any(parameter, ids),
        _ => false,
    }
}

/// Converts conical semi-angles and 2D line, polyline, and B-spline p-curves
/// on cylinders, cones, spheres, tori, and surfaces of revolution. The source
/// angular assignment is replaced by its validated SI radian base after
/// converting geometry; unrelated p-curves are untouched.
pub(super) fn normalize(data: &mut DataSection) -> Result<(), StepError> {
    if is_angle_independent_geometry(data)
        || !data.entities.iter().map(entity_records).any(|records| {
            component(records, "PLANE_ANGLE_UNIT").is_some()
                && component(records, "CONVERSION_BASED_UNIT").is_some()
        })
    {
        return Ok(());
    }
    let resolver = resolver(data)?;
    let mut angle_factor = None;
    let mut replacements = HashMap::new();
    for records in resolver.entities.values() {
        let Some(context) = component(records, "GLOBAL_UNIT_ASSIGNED_CONTEXT") else {
            continue;
        };
        let args = list(&context.parameter)?;
        if args.len() != 1 {
            return Err(invalid("invalid global unit assignment"));
        }
        for unit in list(&args[0])? {
            let id = reference(unit)?;
            if resolver
                .entities
                .get(&id)
                .is_some_and(|records| component(records, "PLANE_ANGLE_UNIT").is_some())
            {
                let (factor, base) = resolver.angle_scale_and_base(id)?;
                if angle_factor.is_some_and(|previous| previous != factor) {
                    return Err(invalid("mixed angular-unit contexts are not yet supported"));
                }
                angle_factor = Some(factor);
                if factor != 1.0 {
                    replacements.insert(id, base);
                }
            }
        }
    }
    let factor =
        angle_factor.ok_or_else(|| invalid("angular geometry has no assigned angle unit"))?;
    if factor == 1.0 {
        return Ok(());
    }
    if unsupported_angular_geometry(data) {
        return Err(invalid(
            "unsupported angular geometry in non-radian STEP context",
        ));
    }

    let mut angular_lines = BTreeMap::new();
    let mut angular_curves = BTreeMap::new();
    let mut angular_representations = BTreeSet::new();
    let mut curve_usage = BTreeMap::new();
    let mut point_axes = BTreeMap::new();
    for records in resolver.entities.values() {
        let Some(pcurve) = component(records, "PCURVE") else {
            continue;
        };
        let args = list(&pcurve.parameter)?;
        if args.len() != 3 {
            return Err(invalid("invalid p-curve definition"));
        }
        let basis = reference(&args[1])?;
        let surface = resolver
            .entities
            .get(&basis)
            .ok_or_else(|| invalid("missing p-curve basis surface"))?;
        let axes = if component(surface, "CONICAL_SURFACE").is_some()
            || component(surface, "CYLINDRICAL_SURFACE").is_some()
            || component(surface, "SURFACE_OF_REVOLUTION").is_some()
        {
            U_ANGLE
        } else if component(surface, "SPHERICAL_SURFACE").is_some()
            || component(surface, "TOROIDAL_SURFACE").is_some()
        {
            BOTH_ANGLES
        } else {
            NO_ANGLES
        };
        if axes == NO_ANGLES
            && !surface
                .iter()
                .any(|record| angle_independent_surface(&record.name))
        {
            return Err(invalid(
                "unsupported p-curve basis in non-radian STEP context",
            ));
        }
        let representation_id = reference(&args[2])?;
        let representation = record(&resolver, representation_id, "DEFINITIONAL_REPRESENTATION")?;
        let items = list(&representation.parameter)?;
        if items.len() != 3 || list(&items[1])?.len() != 1 {
            return Err(invalid("unsupported angular p-curve representation"));
        }
        let curve_id = reference(&list(&items[1])?[0])?;
        if axes != NO_ANGLES {
            let curve = resolver
                .entities
                .get(&curve_id)
                .ok_or_else(|| invalid("missing angular p-curve geometry"))?;
            if component(curve, "LINE").is_some() {
                assign_axes(&mut angular_lines, curve_id, axes)?;
            } else {
                let controls = if let Some(polyline) = component(curve, "POLYLINE") {
                    let args = list(&polyline.parameter)?;
                    if args.len() != 2 {
                        return Err(invalid("invalid angular p-curve polyline"));
                    }
                    list(&args[1])?
                } else if let Some(spline) = component(curve, "B_SPLINE_CURVE")
                    .or_else(|| component(curve, "B_SPLINE_CURVE_WITH_KNOTS"))
                {
                    let args = list(&spline.parameter)?;
                    if args.len() < 3 {
                        return Err(invalid("invalid angular p-curve B-spline"));
                    }
                    list(&args[2])?
                } else {
                    return Err(invalid("unsupported angular p-curve geometry"));
                };
                if controls.is_empty() {
                    return Err(invalid("angular p-curve has no control points"));
                }
                for control in controls {
                    assign_axes(&mut point_axes, reference(control)?, axes)?;
                }
            }
            assign_axes(&mut angular_curves, curve_id, axes)?;
            angular_representations.insert(representation_id);
        }
        assign_axes(&mut curve_usage, curve_id, axes)?;
    }

    let mut vector_edits = BTreeMap::new();
    for (&id, &axes) in &angular_lines {
        let line = record(&resolver, id, "LINE")?;
        let args = list(&line.parameter)?;
        if args.len() != 3 {
            return Err(invalid("invalid angular p-curve line"));
        }
        let point_id = reference(&args[1])?;
        let vector_id = reference(&args[2])?;
        let vector = record(&resolver, vector_id, "VECTOR")?;
        let vector_args = list(&vector.parameter)?;
        if vector_args.len() != 3 {
            return Err(invalid("invalid angular p-curve vector"));
        }
        let direction = record(&resolver, reference(&vector_args[1])?, "DIRECTION")?;
        let direction_args = list(&direction.parameter)?;
        if direction_args.len() != 2 || list(&direction_args[1])?.len() != 2 {
            return Err(invalid("angular p-curve direction is not two-dimensional"));
        }
        let coordinates = list(&direction_args[1])?;
        let (dx, dy) = (number(&coordinates[0])?, number(&coordinates[1])?);
        if dx == 0.0 && dy == 0.0 {
            return Err(invalid("angular p-curve direction is zero"));
        }
        let magnitude = number(&vector_args[2])?;
        if magnitude <= 0.0 {
            return Err(invalid("angular p-curve vector has no positive magnitude"));
        }
        assign_axes(&mut point_axes, point_id, axes)?;
        if let std::collections::btree_map::Entry::Vacant(entry) = vector_edits.entry(vector_id) {
            let scaled_x = if axes[0] { dx * factor } else { dx };
            let scaled_y = if axes[1] { dy * factor } else { dy };
            if !scaled_x.is_finite()
                || !scaled_y.is_finite()
                || (dx != 0.0 && scaled_x == 0.0)
                || (dy != 0.0 && scaled_y == 0.0)
            {
                return Err(invalid("angular p-curve vector cannot be scaled"));
            }
            let norm = scaled_x.hypot(scaled_y);
            let new_magnitude = magnitude * norm;
            if !norm.is_finite()
                || norm == 0.0
                || !new_magnitude.is_finite()
                || new_magnitude == 0.0
            {
                return Err(invalid("angular p-curve vector cannot be scaled"));
            }
            let (new_x, new_y) = (scaled_x / norm, scaled_y / norm);
            if (scaled_x != 0.0 && new_x == 0.0) || (scaled_y != 0.0 && new_y == 0.0) {
                return Err(invalid("angular p-curve direction underflows"));
            }
            let mut new_direction = direction.clone();
            let args = list_mut(&mut new_direction.parameter)?;
            let ratios = list_mut(&mut args[1])?;
            ratios[0] = Parameter::Real(new_x);
            ratios[1] = Parameter::Real(new_y);
            entry.insert((axes, new_direction, new_magnitude));
        } else if vector_edits
            .get(&vector_id)
            .is_some_and(|(used, _, _)| *used != axes)
        {
            return Err(invalid("vector is shared across incompatible angular axes"));
        }
    }
    for &id in point_axes.keys() {
        let point = record(&resolver, id, "CARTESIAN_POINT")?;
        let args = list(&point.parameter)?;
        if args.len() != 2 || list(&args[1])?.len() != 2 {
            return Err(invalid("angular p-curve point is not two-dimensional"));
        }
        for coordinate in list(&args[1])? {
            number(coordinate)?;
        }
    }
    let point_ids = point_axes.keys().copied().collect::<BTreeSet<_>>();
    let vector_ids = vector_edits.keys().copied().collect::<BTreeSet<_>>();
    let angular_curve_ids = angular_curves.keys().copied().collect::<BTreeSet<_>>();

    // A point, vector, or curve reused outside its angular p-curve would
    // reinterpret unrelated geometry after the in-place conversion.
    for (&id, records) in &resolver.entities {
        for record in *records {
            let allowed_control_reference = angular_curves.contains_key(&id)
                && matches!(
                    record.name.as_str(),
                    "LINE" | "POLYLINE" | "B_SPLINE_CURVE" | "B_SPLINE_CURVE_WITH_KNOTS"
                );
            if !allowed_control_reference
                && (references_any(&record.parameter, &point_ids)
                    || references_any(&record.parameter, &vector_ids))
            {
                return Err(invalid(
                    "angular p-curve geometry is shared outside its curve",
                ));
            }
            if !angular_representations.contains(&id)
                && references_any(&record.parameter, &angular_curve_ids)
            {
                return Err(invalid(
                    "angular p-curve is shared outside its representation",
                ));
            }
        }
    }

    drop(resolver);
    let indices = data
        .entities
        .iter()
        .enumerate()
        .map(|(index, entity)| {
            let id = match entity {
                EntityInstance::Simple { id, .. } | EntityInstance::Complex { id, .. } => *id,
            };
            (id, index)
        })
        .collect::<HashMap<_, _>>();
    for (&id, &axes) in &point_axes {
        let point = indexed_record_mut(data, &indices, id, "CARTESIAN_POINT")?;
        let args = list_mut(&mut point.parameter)?;
        let coordinates = list_mut(&mut args[1])?;
        for axis in 0..2 {
            if axes[axis] {
                scale_number(&mut coordinates[axis], factor)?;
            }
        }
    }
    let max_id = indices.keys().max().copied().unwrap_or(0);
    let direction_count = u64::try_from(vector_edits.len())
        .map_err(|_| invalid("too many angular p-curve directions"))?;
    max_id
        .checked_add(direction_count)
        .ok_or_else(|| invalid("no STEP entity identifier remains for angular conversion"))?;
    for (offset, (id, (_, direction, magnitude))) in vector_edits.into_iter().enumerate() {
        let next_id = max_id + 1 + offset as u64;
        {
            let vector = indexed_record_mut(data, &indices, id, "VECTOR")?;
            let args = list_mut(&mut vector.parameter)?;
            args[1] = Parameter::Ref(Name::Entity(next_id));
            args[2] = Parameter::Real(magnitude);
        }
        data.entities.push(EntityInstance::Simple {
            id: next_id,
            record: direction,
        });
    }
    for entity in &mut data.entities {
        for record in entity_records_mut(entity) {
            if record.name == "CONICAL_SURFACE" {
                let args = list_mut(&mut record.parameter)?;
                if args.len() != 4 {
                    return Err(invalid("invalid conical surface angle"));
                }
                scale_number(&mut args[3], factor)?;
            } else if record.name == "GLOBAL_UNIT_ASSIGNED_CONTEXT" {
                let args = list_mut(&mut record.parameter)?;
                for unit in list_mut(&mut args[0])? {
                    let id = reference(unit)?;
                    if let Some(&base) = replacements.get(&id) {
                        *unit = Parameter::Ref(Name::Entity(base));
                    }
                }
            }
        }
    }
    Ok(())
}

fn list_mut(parameter: &mut Parameter) -> Result<&mut Vec<Parameter>, StepError> {
    match parameter {
        Parameter::List(items) => Ok(items),
        _ => Err(invalid("expected a parameter list")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn degree_cone() -> DataSection {
        "DATA;
        #1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2,#3));
        #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));
        #3 = (CONVERSION_BASED_UNIT('degree',#4) NAMED_UNIT(#6) PLANE_ANGLE_UNIT());
        #4 = MEASURE_WITH_UNIT(PLANE_ANGLE_MEASURE(0.017453292519943295),#5);
        #5 = (NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.));
        #6 = DIMENSIONAL_EXPONENTS(0.,0.,0.,0.,0.,0.,0.);
        #7 = CONICAL_SURFACE('',#8,2.,18.43494882292201);
        #9 = PCURVE('',#7,#10);
        #10 = DEFINITIONAL_REPRESENTATION('',(#11),#12);
        #11 = LINE('',#13,#14);
        #13 = CARTESIAN_POINT('',(60.,3.));
        #14 = VECTOR('',#15,60.);
        #15 = DIRECTION('',(1.,0.));
        ENDSEC;"
            .parse()
            .unwrap()
    }

    #[test]
    fn normalizes_degree_cone_and_its_angular_iso_trim() {
        let mut data = degree_cone();
        normalize(&mut data).unwrap();
        let records = resolver(&data).unwrap();
        let context = record(&records, 1, "GLOBAL_UNIT_ASSIGNED_CONTEXT").unwrap();
        assert_eq!(
            reference(&list(&list(&context.parameter).unwrap()[0]).unwrap()[1]).unwrap(),
            5
        );
        let cone = record(&records, 7, "CONICAL_SURFACE").unwrap();
        assert!(
            (number(&list(&cone.parameter).unwrap()[3]).unwrap() - (1. / 3_f64).atan()).abs()
                < 1e-14
        );
        let point = record(&records, 13, "CARTESIAN_POINT").unwrap();
        assert!(
            (number(&list(&list(&point.parameter).unwrap()[1]).unwrap()[0]).unwrap()
                - std::f64::consts::FRAC_PI_3)
                .abs()
                < 1e-14
        );
        let vector = record(&records, 14, "VECTOR").unwrap();
        assert!(
            (number(&list(&vector.parameter).unwrap()[2]).unwrap() - std::f64::consts::FRAC_PI_3)
                .abs()
                < 1e-14
        );
        assert_eq!(uniform_meters_per_unit(&data).unwrap(), 1e-3);
    }

    #[test]
    fn converts_diagonal_trims_and_rejects_shared_parameter_points() {
        let mut diagonal = degree_cone();
        diagonal
            .entities
            .push("#16 = VECTOR('',#15,7.);".parse().unwrap());
        let direction = diagonal
            .entities
            .iter_mut()
            .find(|entity| matches!(entity, EntityInstance::Simple { id: 15, .. }))
            .unwrap();
        let direction_record = entity_records_mut(direction).first_mut().unwrap();
        list_mut(&mut direction_record.parameter).unwrap()[1] =
            Parameter::List(vec![Parameter::Real(0.5), Parameter::Real(0.5)]);
        normalize(&mut diagonal).unwrap();
        let records = resolver(&diagonal).unwrap();
        let original_direction = record(&records, 15, "DIRECTION").unwrap();
        let original_ratios = list(&list(&original_direction.parameter).unwrap()[1]).unwrap();
        assert_eq!(number(&original_ratios[0]).unwrap(), 0.5);
        assert_eq!(number(&original_ratios[1]).unwrap(), 0.5);
        assert_eq!(
            reference(&list(&record(&records, 16, "VECTOR").unwrap().parameter).unwrap()[1])
                .unwrap(),
            15
        );
        let transformed_vector = record(&records, 14, "VECTOR").unwrap();
        let args = list(&transformed_vector.parameter).unwrap();
        let transformed_id = reference(&args[1]).unwrap();
        assert_ne!(transformed_id, 15);
        let transformed_direction = record(&records, transformed_id, "DIRECTION").unwrap();
        let ratios = list(&list(&transformed_direction.parameter).unwrap()[1]).unwrap();
        let magnitude = number(&args[2]).unwrap();
        let factor = std::f64::consts::PI / 180.0;
        assert!((number(&ratios[0]).unwrap() * magnitude - 30. * factor).abs() < 1e-14);
        assert!((number(&ratios[1]).unwrap() * magnitude - 30.).abs() < 1e-14);

        let mut shared = degree_cone();
        shared
            .entities
            .push("#16 = LINE('',#13,#17);".parse().unwrap());
        assert!(
            normalize(&mut shared)
                .unwrap_err()
                .to_string()
                .contains("shared")
        );

        let mut shared_polyline = degree_cone();
        shared_polyline
            .entities
            .push("#16 = POLYLINE('',(#13,#13));".parse().unwrap());
        assert!(
            normalize(&mut shared_polyline)
                .unwrap_err()
                .to_string()
                .contains("shared")
        );

        let mut malformed_cone = degree_cone();
        let cone = malformed_cone
            .entities
            .iter_mut()
            .find(|entity| matches!(entity, EntityInstance::Simple { id: 7, .. }))
            .unwrap();
        *cone = "#7 = CONICAL_SURFACE('',#8,2.);".parse().unwrap();
        assert!(
            normalize(&mut malformed_cone)
                .unwrap_err()
                .to_string()
                .contains("invalid conical surface angle")
        );
    }

    #[test]
    fn converts_polyline_and_bspline_control_points_without_changing_axial_coordinates() {
        for source_curve in [
            "#11 = POLYLINE('',(#13,#17));",
            "#11 = B_SPLINE_CURVE_WITH_KNOTS('',1,(#13,#17),.UNSPECIFIED.,.F.,.F.,(2,2),(0.,1.),.UNSPECIFIED.);",
            "#11 = (BOUNDED_CURVE() B_SPLINE_CURVE('',1,(#13,#17),.UNSPECIFIED.,.F.,.F.) B_SPLINE_CURVE_WITH_KNOTS((2,2),(0.,1.),.UNSPECIFIED.) CURVE() GEOMETRIC_REPRESENTATION_ITEM() RATIONAL_B_SPLINE_CURVE((1.,0.5)) REPRESENTATION_ITEM(''));",
        ] {
            let mut data = degree_cone();
            let curve = data
                .entities
                .iter_mut()
                .find(|entity| matches!(entity, EntityInstance::Simple { id: 11, .. }))
                .unwrap();
            *curve = source_curve.parse().unwrap();
            data.entities
                .push("#17 = CARTESIAN_POINT('',(120.,6.));".parse().unwrap());
            normalize(&mut data).unwrap();
            let records = resolver(&data).unwrap();
            for (id, expected_u, expected_v) in [
                (13, std::f64::consts::FRAC_PI_3, 3.0),
                (17, 2. * std::f64::consts::FRAC_PI_3, 6.0),
            ] {
                let point = record(&records, id, "CARTESIAN_POINT").unwrap();
                let coordinates = list(&list(&point.parameter).unwrap()[1]).unwrap();
                assert!((number(&coordinates[0]).unwrap() - expected_u).abs() < 1e-14);
                assert_eq!(number(&coordinates[1]).unwrap(), expected_v);
            }
        }
    }

    #[test]
    fn converts_both_spherical_and_toroidal_parameters() {
        for surface in [
            "#7 = SPHERICAL_SURFACE('',#8,2.);",
            "#7 = TOROIDAL_SURFACE('',#8,3.,1.);",
        ] {
            let mut data = degree_cone();
            *data
                .entities
                .iter_mut()
                .find(|entity| matches!(entity, EntityInstance::Simple { id: 7, .. }))
                .unwrap() = surface.parse().unwrap();
            *data
                .entities
                .iter_mut()
                .find(|entity| matches!(entity, EntityInstance::Simple { id: 13, .. }))
                .unwrap() = "#13 = CARTESIAN_POINT('',(60.,30.));".parse().unwrap();
            *data
                .entities
                .iter_mut()
                .find(|entity| matches!(entity, EntityInstance::Simple { id: 15, .. }))
                .unwrap() = "#15 = DIRECTION('',(0.5,0.5));".parse().unwrap();
            normalize(&mut data).unwrap();
            let records = resolver(&data).unwrap();
            let point = record(&records, 13, "CARTESIAN_POINT").unwrap();
            let coordinates = list(&list(&point.parameter).unwrap()[1]).unwrap();
            assert!((number(&coordinates[0]).unwrap() - std::f64::consts::FRAC_PI_3).abs() < 1e-14);
            assert!((number(&coordinates[1]).unwrap() - std::f64::consts::FRAC_PI_6).abs() < 1e-14);
            let vector = record(&records, 14, "VECTOR").unwrap();
            let args = list(&vector.parameter).unwrap();
            let direction = record(&records, reference(&args[1]).unwrap(), "DIRECTION").unwrap();
            let ratios = list(&list(&direction.parameter).unwrap()[1]).unwrap();
            let magnitude = number(&args[2]).unwrap();
            let expected = 30. * std::f64::consts::PI / 180.;
            assert!((magnitude * number(&ratios[0]).unwrap() - expected).abs() < 1e-14);
            assert!((magnitude * number(&ratios[1]).unwrap() - expected).abs() < 1e-14);
        }
    }

    #[test]
    fn rejects_point_shared_by_one_and_two_angle_axis_surfaces() {
        let mut data = degree_cone();
        for entity in [
            "#18 = SPHERICAL_SURFACE('',#8,2.);",
            "#19 = PCURVE('',#18,#20);",
            "#20 = DEFINITIONAL_REPRESENTATION('',(#21),#12);",
            "#21 = LINE('',#13,#22);",
            "#22 = VECTOR('',#23,30.);",
            "#23 = DIRECTION('',(0.,1.));",
        ] {
            data.entities.push(entity.parse().unwrap());
        }
        assert!(
            normalize(&mut data)
                .unwrap_err()
                .to_string()
                .contains("incompatible angular axes")
        );
    }
}
