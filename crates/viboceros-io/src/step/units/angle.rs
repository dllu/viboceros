//! Normalize angular STEP values that the Monstertruck table reads as radians.
use super::*;

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
    let value = number(parameter)? * factor;
    if !value.is_finite() {
        return Err(invalid("angular geometry conversion overflows"));
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
                && !matches!(name, "CYLINDRICAL_SURFACE" | "CONICAL_SURFACE"))
            || name.starts_with("SURFACE_OF_")
            || name.contains("REVOL")
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

/// Converts conical semi-angles and axis-aligned 2D p-curves on cylinders and
/// cones. The source angular assignment is replaced by its validated SI radian
/// base after converting geometry; unrelated planar p-curves are untouched.
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

    let mut angular_lines = BTreeSet::new();
    let mut angular_representations = BTreeSet::new();
    let mut line_usage = HashMap::new();
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
        let angular = component(surface, "CONICAL_SURFACE").is_some()
            || component(surface, "CYLINDRICAL_SURFACE").is_some();
        if !angular
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
        let line_id = reference(&list(&items[1])?[0])?;
        if angular {
            record(&resolver, line_id, "LINE")?;
            angular_lines.insert(line_id);
            angular_representations.insert(representation_id);
        }
        if line_usage
            .insert(line_id, angular)
            .is_some_and(|prior| prior != angular)
        {
            return Err(invalid(
                "p-curve line is shared across incompatible surfaces",
            ));
        }
    }

    let mut point_ids = BTreeSet::new();
    let mut vector_ids = BTreeSet::new();
    for &id in &angular_lines {
        let line = record(&resolver, id, "LINE")?;
        let args = list(&line.parameter)?;
        if args.len() != 3 {
            return Err(invalid("invalid angular p-curve line"));
        }
        let point_id = reference(&args[1])?;
        let vector_id = reference(&args[2])?;
        let point = record(&resolver, point_id, "CARTESIAN_POINT")?;
        let point_args = list(&point.parameter)?;
        if point_args.len() != 2 || list(&point_args[1])?.len() != 2 {
            return Err(invalid("angular p-curve point is not two-dimensional"));
        }
        for coordinate in list(&point_args[1])? {
            number(coordinate)?;
        }
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
        if !((dx != 0.0 && dy == 0.0) || (dx == 0.0 && dy != 0.0)) {
            return Err(invalid(
                "non-radian angular p-curves require axis-aligned lines",
            ));
        }
        if number(&vector_args[2])? <= 0.0 {
            return Err(invalid("angular p-curve vector has no positive magnitude"));
        }
        point_ids.insert(point_id);
        if dy == 0.0 {
            vector_ids.insert(vector_id);
        }
    }

    // 2D point/vector entities may be shared. A reference from another curve
    // would otherwise reinterpret that curve after the in-place conversion.
    for (&id, records) in &resolver.entities {
        let Some(line) = component(records, "LINE") else {
            continue;
        };
        let args = list(&line.parameter)?;
        if args.len() != 3 {
            continue;
        }
        let point = reference(&args[1])?;
        let vector = reference(&args[2])?;
        if !angular_lines.contains(&id)
            && (point_ids.contains(&point) || vector_ids.contains(&vector))
        {
            return Err(invalid(
                "angular p-curve geometry is shared with another line",
            ));
        }
    }
    for (&id, records) in &resolver.entities {
        for record in *records {
            if record.name != "LINE"
                && (references_any(&record.parameter, &point_ids)
                    || references_any(&record.parameter, &vector_ids))
            {
                return Err(invalid(
                    "angular p-curve geometry is shared outside its line",
                ));
            }
            if !angular_representations.contains(&id)
                && references_any(&record.parameter, &angular_lines)
            {
                return Err(invalid(
                    "angular p-curve line is shared outside its representation",
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
    for &id in &point_ids {
        let point = indexed_record_mut(data, &indices, id, "CARTESIAN_POINT")?;
        let args = list_mut(&mut point.parameter)?;
        scale_number(&mut list_mut(&mut args[1])?[0], factor)?;
    }
    for &id in &vector_ids {
        let vector = indexed_record_mut(data, &indices, id, "VECTOR")?;
        scale_number(&mut list_mut(&mut vector.parameter)?[2], factor)?;
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
    fn rejects_diagonal_trims_and_shared_parameter_points() {
        let mut diagonal = degree_cone();
        let direction = diagonal
            .entities
            .iter_mut()
            .find(|entity| matches!(entity, EntityInstance::Simple { id: 15, .. }))
            .unwrap();
        let record = entity_records_mut(direction).first_mut().unwrap();
        list_mut(&mut record.parameter).unwrap()[1] =
            Parameter::List(vec![Parameter::Real(0.5), Parameter::Real(0.5)]);
        assert!(
            normalize(&mut diagonal)
                .unwrap_err()
                .to_string()
                .contains("axis-aligned")
        );

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
}
