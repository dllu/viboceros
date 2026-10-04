//! Complete world point arguments share the bounded drafting calculator.
use super::*;

pub(super) fn parse_point(arguments: &[&str]) -> Result<(Point3, usize), CommandError> {
    let first = arguments
        .first()
        .ok_or(CommandError::Usage("expected a point"))?;
    let body = first
        .strip_prefix('w')
        .or_else(|| first.strip_prefix('W'))
        .unwrap_or(first);
    let (coordinates, consumed) = if body.contains(',') {
        let coordinates: Vec<_> = body.split(',').collect();
        if !(2..=3).contains(&coordinates.len()) && !body.contains('(') {
            return Err(CommandError::Usage("point syntax is x,y or x,y,z"));
        }
        (coordinates, 1)
    } else {
        if arguments.len() < 3 {
            return Err(CommandError::Usage("point syntax is x y z or x,y,z"));
        }
        (vec![body, arguments[1], arguments[2]], 3)
    };

    // Ordinary numbers preserve their existing fast path and exact bits.
    // Calculator components share the bounded interactive point parser.
    let mut parsed = [0.0; 3];
    for (index, coordinate) in coordinates.iter().enumerate() {
        if let Ok(value) = coordinate.parse::<Real>()
            && let Some(component) = parsed.get_mut(index)
        {
            *component = value;
            continue;
        }
        let text = format!(
            "w{}",
            if consumed == 1 {
                body.to_owned()
            } else {
                coordinates.join(",")
            }
        );
        let point = viboceros_drafting::PointInput::parse_with_units(
            &text,
            &viboceros_geometry::LengthUnitSystem::Unset,
        )
        .ok_or_else(|| CommandError::InvalidNumber((*coordinate).to_owned()))?
        .map_err(|_| CommandError::InvalidNumber((*coordinate).to_owned()))?
        .resolve(CommandContext::default().construction_plane, None)
        .map_err(|_| CommandError::InvalidNumber((*coordinate).to_owned()))?;
        return Ok((point, consumed));
    }
    Ok((Point3::try_from(parsed)?, consumed))
}

#[cfg(test)]
mod point_precision_tests {
    use super::*;

    #[test]
    fn native_point_command_precision_matches_sixty_four_exact_coordinate_records() {
        use serde_json::Value;
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_input_precision.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/point_input_precision.json"
        ))
        .unwrap();
        let operations = fixture["operations"].as_array().unwrap();
        let rows = observed["results"].as_array().unwrap();
        assert_eq!(operations.len(), 64);
        assert_eq!(rows.len(), 64);
        for (op, row) in operations.iter().zip(rows) {
            let label = op["id"].as_str().unwrap();
            assert_eq!(row["id"], label);
            let registry = CommandRegistry::with_builtins();
            let mut document = Document::default();
            if op["prime"] == "ScalePositions" {
                for input in [
                    "Point 1,1,1",
                    "SelAll",
                    "ScalePositions Copy=No Mode=1D w0,0,0 w1,0,0 w2,0,0",
                    "SelAll",
                    "Delete",
                ] {
                    registry.execute(&mut document, input).unwrap();
                }
                document.clear_history().unwrap();
            }
            // This is the echoed prescribed recipe, independently checked
            // against the fixture generator in Python, not a Point output.
            let token = row["value"]["recipe"]["token"].as_str().unwrap();
            registry
                .execute(&mut document, &format!("Point {token}"))
                .unwrap_or_else(|error| panic!("{label}: {error}"));
            assert_eq!(document.objects().count(), 1, "{label}");
            let object = document.objects().next().unwrap();
            let Geometry::Point(p) = object.geometry() else {
                panic!("{label}: expected Point");
            };
            let actual = p.to_array().map(|x| format!("{:016x}", x.to_bits()));
            for phase in ["after", "after_script"] {
                let expected = &row["value"][phase][0]["bits"];
                for (axis, bits) in actual.iter().enumerate() {
                    assert_eq!(bits, expected[axis].as_str().unwrap(), "{label}: {phase}");
                }
            }
            assert!(!document.is_selected(object.id()), "{label}");
        }
    }

    #[test]
    fn complete_point_commands_accept_world_prefixes_and_cartesian_calculator_components() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        for (input, expected) in [
            (
                "Point w4503599627370497/37778931862957161709568,0,0",
                [f64::from_bits(0x3e80000000000001), 0., 0.],
            ),
            ("Point W1+2,4-1,1-3/4", [3., 3., 1.75]),
            ("Point pow(2,3),sqrt(16),sin(0)", [8., 4., 0.]),
            ("Point 1+2 4-1 5/16", [3., 3., 0.3125]),
        ] {
            registry.execute(&mut document, input).unwrap();
            let Geometry::Point(p) = document.objects().last().unwrap().geometry() else {
                panic!("point")
            };
            assert_eq!(
                p.to_array().map(f64::to_bits),
                expected.map(f64::to_bits),
                "{input}"
            );
        }
        let before = format!("{document:?}");
        for input in [
            "Point w1/0,0,0",
            "Point wNaN,0,0",
            "Point 1m,0,0",
            "Point r1,2,3",
            "Point pow(2,3,4),0,0",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(format!("{document:?}"), before, "{input}");
        }
    }
}
