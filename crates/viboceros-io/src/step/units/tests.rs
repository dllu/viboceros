use super::*;

fn scale(records: &str) -> Result<f64, StepError> {
    let data: DataSection = format!("DATA;\n{records}\nENDSEC;").parse().unwrap();
    uniform_meters_per_unit(&data)
}

#[test]
fn rejects_ambiguous_unit_components_and_non_length_dimensions() {
    for records in [
        "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2)); #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) SI_UNIT(.KILO.,.METRE.));",
        "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2)); #2 = (LENGTH_UNIT() NAMED_UNIT(#3) SI_UNIT($,.METRE.)); #3 = DIMENSIONAL_EXPONENTS(0.,1.,0.,0.,0.,0.,0.);",
    ] {
        assert!(
            scale(records).is_err(),
            "accepted conflicting unit definition: {records}"
        );
    }
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
            #4 = (CONVERSION_BASED_UNIT('bad',#3) LENGTH_UNIT() NAMED_UNIT(#5));
            #5 = DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);"
        );
        assert!(
            scale(&records)
                .unwrap_err()
                .to_string()
                .contains("factor must be finite and positive")
        );
    }
    let records = "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2));
        #2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.));
        #3 = (GEOMETRIC_REPRESENTATION_CONTEXT(3) REPRESENTATION_CONTEXT('',''));";
    assert!(scale(records).is_err());
}

#[test]
fn conversion_cycles_and_depth_limits_are_checked_after_valid_dimensions() {
    let cyclic = "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2));
        #2 = (CONVERSION_BASED_UNIT('cycle',#3) LENGTH_UNIT() NAMED_UNIT(#4));
        #3 = LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.),#2);
        #4 = DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);";
    assert!(scale(cyclic).unwrap_err().to_string().contains("cyclic"));
    for count in [63, 64] {
        let mut records = String::from(
            "#2 = (LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.)); #3 = DIMENSIONAL_EXPONENTS(1.,0.,0.,0.,0.,0.,0.);",
        );
        let mut previous = 2;
        for index in 0..count {
            let unit = 100 + index;
            let measure = 200 + index;
            records.push_str(&format!("#{unit} = (CONVERSION_BASED_UNIT('chain',#{measure}) LENGTH_UNIT() NAMED_UNIT(#3)); #{measure} = LENGTH_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.),#{previous});"));
            previous = unit;
        }
        records.push_str(&format!(
            "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#{previous}));"
        ));
        if count == 63 {
            assert_eq!(scale(&records).unwrap(), 1.0);
        } else {
            assert!(
                scale(&records)
                    .unwrap_err()
                    .to_string()
                    .contains("excessively deep")
            );
        }
    }
}

#[test]
fn explicit_dimensions_require_exact_finite_length_exponents() {
    for (exponents, valid) in [
        ("1,0,0,0,0,0,0", true),
        ("1.,0.,0.,0.,0.,0.,0.", true),
        ("1.,0.,0.,0.,0.,0.", false),
        ("1.,0.,0.,0.,0.,0.,1.E999", false),
        ("1.,0.,0.,0.,0.,0.,0.0000000001", false),
        ("1.,0.,0.,0.,0.,0.,$", false),
    ] {
        let records = format!(
            "#1 = GLOBAL_UNIT_ASSIGNED_CONTEXT((#2)); #2 = (LENGTH_UNIT() NAMED_UNIT(#3) SI_UNIT($,.METRE.)); #3 = DIMENSIONAL_EXPONENTS({exponents});"
        );
        assert_eq!(scale(&records).is_ok(), valid, "{exponents}");
    }
}
