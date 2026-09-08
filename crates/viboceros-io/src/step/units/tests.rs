use super::*;

fn scale(records: &str) -> Result<f64, StepError> {
    let data: DataSection = format!("DATA;\n{records}\nENDSEC;").parse().unwrap();
    uniform_meters_per_unit(&data)
}

#[test]
fn resolves_si_prefixes_and_nested_conversion_measures() {
    for (prefix, expected) in [
        ("$", 1.0),
        (".MILLI.", 1e-3),
        (".MICRO.", 1e-6),
        (".KILO.", 1e3),
        (".NANO.", 1e-9),
    ] {
        assert_eq!(scale(&format!("#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2)); #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT({prefix},.METRE.));")).unwrap(), expected);
    }
    // An inch based on millimetres, then a foot based on inches. The
    // second conversion measure uses the complex-entity representation.
    let records = "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#6));
        #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));
        #3 = LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(25.4),#2);
        #4 = (CONVERSION_BASED_UNIT('inch',#3) LENGTH_UNIT() NAMED_UNIT(#7));
        #5 = (LENGTH_MEASURE_WITH_UNIT() MEASURE_WITH_UNIT(LENGTH_MEASURE(12.),#4));
        #6 = (CONVERSION_BASED_UNIT('foot',#5) LENGTH_UNIT() NAMED_UNIT(#7));
        #7 = DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);";
    assert!((scale(records).unwrap() - 0.3048).abs() < 1e-15);
}

#[test]
fn rejects_invalid_factors_and_missing_geometry_context_units() {
    for factor in ["0.", "-1.", "1.E999"] {
        let records = format!(
            "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#4));
            #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.));
            #3 = LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE({factor}),#2);
            #4 = (CONVERSION_BASED_UNIT('bad',#3) LENGTH_UNIT() NAMED_UNIT(*));"
        );
        assert!(scale(&records).is_err());
    }
    let records = "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2));
        #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.));
        #3 = (GEOMETRIC_REPRESENTATION_CONTEXT(3) REPRESENTATION_CONTEXT('',''));";
    assert!(scale(records).is_err());
}
