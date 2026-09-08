//! Resolve physical length units from Part 21 records before table conversion.
use super::StepError;
use monstertruck::step::load::step_p21::ast::{
    DataSection, EntityInstance, Name, Parameter, Record,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
    let mut resolver = Resolver {
        entities,
        active: BTreeSet::new(),
        scales: BTreeMap::new(),
    };
    if resolver.entities.values().any(|records| {
        component(records, "GEOMETRIC_REPRESENTATION_CONTEXT").is_some()
            && component(records, "GLOBAL_UNIT_ASSIGNED_CONTEXT").is_none()
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
                let si = component(records, "SI_UNIT")
                    .ok_or_else(|| invalid("non-radian angular contexts are not yet supported"))?;
                let args = list(&si.parameter)?;
                if !matches!(args, [Parameter::NotProvided, Parameter::Enumeration(name)] if name == "RADIAN")
                {
                    return Err(invalid("non-radian angular contexts are not yet supported"));
                }
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
    result.ok_or_else(|| invalid("missing global length-unit assignment"))
}

#[cfg(test)]
mod tests;

struct Resolver<'a> {
    entities: HashMap<u64, &'a [Record]>,
    active: BTreeSet<u64>,
    scales: BTreeMap<u64, f64>,
}
impl Resolver<'_> {
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
