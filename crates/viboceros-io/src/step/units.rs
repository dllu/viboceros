//! Resolve physical length units from Part 21 records before table conversion.
mod angle;
use super::StepError;
use monstertruck::step::load::step_p21::ast::{
    DataSection, EntityInstance, Name, Parameter, Record,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use viboceros_geometry::{LengthUnitSystem, Tolerance};

pub(super) fn conversion_to_target(
    data: &mut DataSection,
    target: &LengthUnitSystem,
    tolerance: Tolerance,
) -> Result<(f64, Tolerance), StepError> {
    angle::normalize(data)?;
    let source = LengthUnitSystem::Custom {
        name: "STEP file units".into(),
        meters_per_unit: uniform_meters_per_unit(data)?,
    };
    let scale = source.scale_to(target)?;
    let source_tolerance = Tolerance::try_new(
        tolerance.absolute() / scale,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    Ok((scale, source_tolerance))
}

fn invalid(message: &str) -> StepError {
    StepError::InvalidLengthUnits(message.into())
}
fn list(parameter: &Parameter) -> Result<&[Parameter], StepError> {
    match parameter {
        Parameter::List(items) => Ok(items),
        _ => Err(invalid("expected a parameter list")),
    }
}
fn reference(parameter: &Parameter) -> Result<u64, StepError> {
    match parameter {
        Parameter::Ref(Name::Entity(id)) => Ok(*id),
        _ => Err(invalid("expected a unit entity reference")),
    }
}
fn component<'a>(records: &'a [Record], name: &str) -> Option<&'a Record> {
    records.iter().find(|record| record.name == name)
}

pub(super) fn uniform_meters_per_unit(data: &DataSection) -> Result<f64, StepError> {
    let mut resolver = resolver(data)?;
    if resolver.entities.values().any(|records| {
        component(records, "GEOMETRIC_REPRESENTATION_CONTEXT").is_some()
            && component(records, "GLOBAL_UNIT_ASSIGNED_CONTEXT").is_none()
            && component(records, "PARAMETRIC_REPRESENTATION_CONTEXT").is_none()
    }) {
        return Err(invalid(
            "geometric representation context has no unit assignment",
        ));
    }
    let contexts = resolver
        .entities
        .values()
        .filter_map(|records| component(records, "GLOBAL_UNIT_ASSIGNED_CONTEXT"))
        .collect::<Vec<_>>();
    let mut result = None;
    let mut non_radian_angle = false;
    for context in contexts {
        let args = list(&context.parameter)?;
        if args.len() != 1 {
            return Err(invalid("invalid global unit assignment"));
        }
        let mut length = None;
        for unit in list(&args[0])? {
            let id = reference(unit)?;
            let records = resolver
                .entities
                .get(&id)
                .ok_or_else(|| invalid("missing assigned unit"))?;
            if component(records, "PLANE_ANGLE_UNIT").is_some() {
                non_radian_angle |= resolver.angle_scale_and_base(id)?.0 != 1.0;
            }
            if component(records, "LENGTH_UNIT").is_some() {
                if length.is_some() {
                    return Err(invalid("multiple length units in one context"));
                }
                length = Some(resolver.length_scale(id)?);
            }
        }
        let length = length.ok_or_else(|| invalid("context has no supported length unit"))?;
        if result.is_some_and(|previous| previous != length) {
            return Err(invalid("mixed length-unit contexts are not yet supported"));
        }
        result = Some(length);
    }
    if non_radian_angle && !is_angle_independent_geometry(data) {
        return Err(invalid(
            "non-radian angular contexts cannot be used with angular geometry parameters",
        ));
    }
    result.ok_or_else(|| invalid("missing global length-unit assignment"))
}

fn resolver(data: &DataSection) -> Result<Resolver<'_>, StepError> {
    let mut entities = HashMap::with_capacity(data.entities.len());
    for entity in &data.entities {
        let (id, records) = match entity {
            EntityInstance::Simple { id, record } => (*id, std::slice::from_ref(record)),
            EntityInstance::Complex { id, subsuper } => (*id, subsuper.0.as_slice()),
        };
        if records.len() > 1 {
            let mut names = HashSet::with_capacity(records.len());
            if records
                .iter()
                .any(|record| !names.insert(record.name.as_str()))
            {
                return Err(invalid("duplicate complex-entity component"));
            }
        }
        if entities.insert(id, records).is_some() {
            return Err(invalid("duplicate entity identifier"));
        }
    }
    Ok(Resolver {
        entities,
        active: BTreeSet::new(),
        scales: BTreeMap::new(),
    })
}

// A conic edge without an explicit angular trim is bounded by its 3D vertices,
// so its arc can be reconstructed in radians regardless of the declared angle
// unit. PCURVEs on non-angular surfaces are similarly independent. Angular
// surfaces and explicit parameter trims need normalization before table
// conversion; Monstertruck otherwise interprets some values as radians.
fn angle_independent_surface(name: &str) -> bool {
    matches!(
        name,
        "PLANE"
            | "SURFACE"
            | "BOUNDED_SURFACE"
            | "B_SPLINE_SURFACE"
            | "B_SPLINE_SURFACE_WITH_KNOTS"
            | "RATIONAL_B_SPLINE_SURFACE"
            | "BEZIER_SURFACE"
            | "QUASI_UNIFORM_SURFACE"
            | "UNIFORM_SURFACE"
    )
}

fn is_angle_independent_geometry(data: &DataSection) -> bool {
    data.entities.iter().all(|entity| {
        let records = match entity {
            EntityInstance::Simple { record, .. } => std::slice::from_ref(record),
            EntityInstance::Complex { subsuper, .. } => subsuper.0.as_slice(),
        };
        records.iter().all(|record| {
            let name = record.name.as_str();
            !(matches!(name, "HYPERBOLA" | "PARABOLA" | "TRIMMED_CURVE")
                || (name.ends_with("_SURFACE") && !angle_independent_surface(name))
                || name.starts_with("SURFACE_OF_")
                || name.contains("REVOL")
                || name.contains("CIRCULAR"))
        })
    })
}

#[cfg(test)]
mod tests;

struct Resolver<'a> {
    entities: HashMap<u64, &'a [Record]>,
    active: BTreeSet<u64>,
    scales: BTreeMap<u64, f64>,
}
impl Resolver<'_> {
    fn validate_angle_dimensions(&self, records: &[Record]) -> Result<(), StepError> {
        let named = component(records, "NAMED_UNIT")
            .ok_or_else(|| invalid("angular unit has no named-unit dimensions"))?;
        let args = list(&named.parameter)?;
        if args.len() != 1 {
            return Err(invalid("invalid angular named-unit dimensions"));
        }
        if matches!(args[0], Parameter::Omitted) && component(records, "SI_UNIT").is_some() {
            return Ok(());
        }
        let id = reference(&args[0])?;
        let dimensions = self
            .entities
            .get(&id)
            .and_then(|records| component(records, "DIMENSIONAL_EXPONENTS"))
            .ok_or_else(|| invalid("missing angular dimensional exponents"))?;
        let exponents = list(&dimensions.parameter)?;
        if exponents.len() != 7
            || exponents.iter().any(|exponent| {
                !matches!(exponent, Parameter::Real(value) if *value == 0.0)
                    && !matches!(exponent, Parameter::Integer(0))
            })
        {
            return Err(invalid("angular unit dimensions are not dimensionless"));
        }
        Ok(())
    }

    fn angle_scale_and_base(&self, id: u64) -> Result<(f64, u64), StepError> {
        let records = *self
            .entities
            .get(&id)
            .ok_or_else(|| invalid("missing assigned angular unit"))?;
        let angle = component(records, "PLANE_ANGLE_UNIT")
            .ok_or_else(|| invalid("conversion references a non-angular unit"))?;
        if !list(&angle.parameter)?.is_empty()
            || component(records, "LENGTH_UNIT").is_some()
            || component(records, "SOLID_ANGLE_UNIT").is_some()
            || component(records, "CONVERSION_BASED_UNIT_WITH_OFFSET").is_some()
        {
            return Err(invalid("conflicting or malformed angular unit"));
        }
        self.validate_angle_dimensions(records)?;
        if let Some(si) = component(records, "SI_UNIT") {
            if component(records, "CONVERSION_BASED_UNIT").is_some()
                || !matches!(list(&si.parameter)?, [Parameter::NotProvided, Parameter::Enumeration(name)] if name == "RADIAN")
            {
                return Err(invalid("unsupported SI angular unit"));
            }
            return Ok((1.0, id));
        }
        let conversion = component(records, "CONVERSION_BASED_UNIT")
            .ok_or_else(|| invalid("unsupported angular unit representation"))?;
        let args = list(&conversion.parameter)?;
        if args.len() != 2 {
            return Err(invalid("invalid conversion-based angular unit"));
        }
        let measure_id = reference(&args[1])?;
        let measure_records = *self
            .entities
            .get(&measure_id)
            .ok_or_else(|| invalid("missing angular conversion measure"))?;
        let measure = component(measure_records, "MEASURE_WITH_UNIT")
            .or_else(|| component(measure_records, "PLANE_ANGLE_MEASURE_WITH_UNIT"))
            .ok_or_else(|| invalid("unsupported angular conversion measure"))?;
        let args = list(&measure.parameter)?;
        if args.len() != 2 {
            return Err(invalid("invalid angular conversion measure"));
        }
        let Parameter::Typed { keyword, parameter } = &args[0] else {
            return Err(invalid("conversion requires a typed angular measure"));
        };
        if keyword != "PLANE_ANGLE_MEASURE" {
            return Err(invalid("conversion measure is not an angle"));
        }
        let factor = match parameter.as_ref() {
            Parameter::Real(value) => *value,
            Parameter::Integer(value) => *value as f64,
            _ => return Err(invalid("invalid angular conversion factor")),
        };
        if !factor.is_finite() || factor <= 0.0 {
            return Err(invalid(
                "angular conversion factor must be finite and positive",
            ));
        }
        let base_id = reference(&args[1])?;
        let base = *self
            .entities
            .get(&base_id)
            .ok_or_else(|| invalid("missing base angular unit"))?;
        let si = component(base, "SI_UNIT")
            .ok_or_else(|| invalid("angular conversion must be based on radians"))?;
        let base_angle = component(base, "PLANE_ANGLE_UNIT")
            .ok_or_else(|| invalid("angular conversion must be based on radians"))?;
        if !list(&base_angle.parameter)?.is_empty()
            || component(base, "LENGTH_UNIT").is_some()
            || component(base, "SOLID_ANGLE_UNIT").is_some()
            || component(base, "CONVERSION_BASED_UNIT").is_some()
            || component(base, "CONVERSION_BASED_UNIT_WITH_OFFSET").is_some()
            || !matches!(list(&si.parameter)?, [Parameter::NotProvided, Parameter::Enumeration(name)] if name == "RADIAN")
        {
            return Err(invalid("angular conversion must be based on radians"));
        }
        self.validate_angle_dimensions(base)?;
        Ok((factor, base_id))
    }

    fn validate_length_dimensions(&self, records: &[Record]) -> Result<(), StepError> {
        let length = component(records, "LENGTH_UNIT")
            .ok_or_else(|| invalid("conversion references a non-length unit"))?;
        if !list(&length.parameter)?.is_empty()
            || component(records, "PLANE_ANGLE_UNIT").is_some()
            || component(records, "SOLID_ANGLE_UNIT").is_some()
        {
            return Err(invalid("conflicting or malformed length-unit type"));
        }
        let named = component(records, "NAMED_UNIT")
            .ok_or_else(|| invalid("length unit has no named-unit dimensions"))?;
        let args = list(&named.parameter)?;
        if args.len() != 1 {
            return Err(invalid("invalid named-unit dimensions"));
        }
        if matches!(args[0], Parameter::Omitted) && component(records, "SI_UNIT").is_some() {
            // SI dimensions are derived from the SI unit name, checked below.
            return Ok(());
        }
        let id = reference(&args[0])?;
        let dimensions = self
            .entities
            .get(&id)
            .and_then(|records| component(records, "DIMENSIONAL_EXPONENTS"))
            .ok_or_else(|| invalid("missing dimensional exponents"))?;
        let exponents = list(&dimensions.parameter)?;
        if exponents.len() != 7 {
            return Err(invalid("expected seven dimensional exponents"));
        }
        for (index, exponent) in exponents.iter().enumerate() {
            let value = match exponent {
                Parameter::Real(value) => *value,
                Parameter::Integer(value) => *value as f64,
                _ => return Err(invalid("invalid dimensional exponent")),
            };
            let expected = if index == 0 { 1.0 } else { 0.0 };
            if value != expected {
                return Err(invalid("unit dimensions are not length"));
            }
        }
        Ok(())
    }

    fn length_scale(&mut self, id: u64) -> Result<f64, StepError> {
        if let Some(scale) = self.scales.get(&id) {
            return Ok(*scale);
        }
        if self.active.len() >= 64 || !self.active.insert(id) {
            return Err(invalid("cyclic or excessively deep unit conversion"));
        }
        let records = *self
            .entities
            .get(&id)
            .ok_or_else(|| invalid("missing referenced length unit"))?;
        self.validate_length_dimensions(records)?;
        if component(records, "CONVERSION_BASED_UNIT_WITH_OFFSET").is_some()
            || (component(records, "SI_UNIT").is_some()
                && component(records, "CONVERSION_BASED_UNIT").is_some())
        {
            return Err(invalid(
                "offset or conflicting length-unit definitions are unsupported",
            ));
        }
        let scale = if let Some(si) = component(records, "SI_UNIT") {
            let args = list(&si.parameter)?;
            if args.len() != 2
                || !matches!(&args[1], Parameter::Enumeration(name) if name == "METRE")
            {
                return Err(invalid("invalid SI length unit"));
            }
            match &args[0] {
                Parameter::NotProvided => 1.0,
                Parameter::Enumeration(prefix) => match prefix.as_str() {
                    "YOTTA" => 1e24,
                    "ZETTA" => 1e21,
                    "EXA" => 1e18,
                    "PETA" => 1e15,
                    "TERA" => 1e12,
                    "GIGA" => 1e9,
                    "MEGA" => 1e6,
                    "KILO" => 1e3,
                    "HECTO" => 1e2,
                    "DECA" => 1e1,
                    "DECI" => 1e-1,
                    "CENTI" => 1e-2,
                    "MILLI" => 1e-3,
                    "MICRO" => 1e-6,
                    "NANO" => 1e-9,
                    "PICO" => 1e-12,
                    "FEMTO" => 1e-15,
                    "ATTO" => 1e-18,
                    "ZEPTO" => 1e-21,
                    "YOCTO" => 1e-24,
                    _ => return Err(invalid("unknown SI prefix")),
                },
                _ => return Err(invalid("invalid SI prefix")),
            }
        } else if let Some(conversion) = component(records, "CONVERSION_BASED_UNIT") {
            let args = list(&conversion.parameter)?;
            if args.len() != 2 {
                return Err(invalid("invalid conversion-based length unit"));
            }
            let measure_id = reference(&args[1])?;
            let measure_records = *self
                .entities
                .get(&measure_id)
                .ok_or_else(|| invalid("missing conversion measure"))?;
            let measure = component(measure_records, "MEASURE_WITH_UNIT")
                .or_else(|| component(measure_records, "LENGTH_MEASURE_WITH_UNIT"))
                .ok_or_else(|| invalid("unsupported conversion measure"))?;
            let args = list(&measure.parameter)?;
            if args.len() != 2 {
                return Err(invalid("invalid conversion measure"));
            }
            let Parameter::Typed { keyword, parameter } = &args[0] else {
                return Err(invalid("conversion requires a typed length measure"));
            };
            if keyword != "LENGTH_MEASURE" {
                return Err(invalid("conversion measure is not a length"));
            }
            let factor = match parameter.as_ref() {
                Parameter::Real(value) => *value,
                Parameter::Integer(value) => *value as f64,
                _ => return Err(invalid("invalid length conversion factor")),
            };
            if !factor.is_finite() || factor <= 0.0 {
                return Err(invalid(
                    "length conversion factor must be finite and positive",
                ));
            }
            factor * self.length_scale(reference(&args[1])?)?
        } else {
            return Err(invalid("unsupported length-unit representation"));
        };
        if !scale.is_finite() || scale <= 0.0 {
            return Err(invalid("length conversion overflows or underflows"));
        }
        self.active.remove(&id);
        self.scales.insert(id, scale);
        Ok(scale)
    }
}
