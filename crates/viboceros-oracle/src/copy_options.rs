//! Shared command preferences replayed across independent input documents.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CopyOptionsFixture {
    sources: Vec<Source>,
    steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    brep: crate::brep_source::BrepSourceFixture,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
enum Name {
    RememberCopyOptions,
    ExtractSrf,
    Rotate,
    Scale,
    Mirror,
}

impl Name {
    fn name(self) -> &'static str {
        match self {
            Self::RememberCopyOptions => "RememberCopyOptions",
            Self::ExtractSrf => "ExtractSrf",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Mirror => "Mirror",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
enum Finish {
    Complete,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Step {
    command: Name,
    copy: Option<bool>,
    finish: Finish,
}

pub(super) fn run(
    f: &CopyOptionsFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.sources.len() != 1 || !(1..=128).contains(&f.steps.len()) {
        return Err(ProbeError::FixtureInvariant("invalid Copy workflow"));
    }
    let registry = CommandRegistry::with_builtins();
    let mut records = Vec::new();
    for step in &f.steps {
        let mut document = Document::new(tolerance);
        let name = step.command.name();
        let id = match step.command {
            Name::RememberCopyOptions => None,
            Name::ExtractSrf => {
                Some(document.add_geometry(Geometry::Brep(f.sources[0].brep.build(tolerance)?))?)
            }
            _ => {
                let id = document.add_geometry(Geometry::Point(Point3::try_new(2., 3., 4.)?))?;
                document.select_objects_direct([id], SelectionMode::Replace)?;
                Some(id)
            }
        };
        document.clear_history()?;
        let snapshot = |doc: &Document| -> Result<Value, ProbeError> {
            Ok(json!(
                doc.objects()
                    .map(|object| Ok(json!({
                        "source":Some(object.id())==id,
                        "selected":doc.is_selected(object.id()),
                        "geometry":untrim::geometry_record(object.geometry(), tolerance)?,
                    })))
                    .collect::<Result<Vec<Value>, ProbeError>>()?
            ))
        };
        let before = snapshot(&document)?;
        let succeeded = if step.finish == Finish::Cancel {
            if step.command == Name::RememberCopyOptions && step.copy.is_some() {
                return Err(ProbeError::FixtureInvariant(
                    "setting choice cannot precede cancellation",
                ));
            }
            registry.begin_copy_options(name);
            false
        } else if step.command == Name::RememberCopyOptions {
            let input = match step.copy {
                Some(value) => format!("{name} {}", if value { "Yes" } else { "No" }),
                None => name.to_owned(),
            };
            registry.execute(&mut document, &input).is_ok()
        } else if step.command == Name::ExtractSrf {
            let default = registry.begin_copy_options(name).unwrap();
            let copy = step.copy.unwrap_or(default);
            let result = viboceros_command::ExtractSurfaceSelection::prepare(
                &document,
                [(id.unwrap(), 0)],
                copy,
                false,
            )?
            .commit(&mut document);
            if result.is_ok() {
                registry.complete_copy_options(name, copy);
            }
            result.is_ok()
        } else {
            let mut input = match step.command {
                Name::Rotate => "Rotate 0,0,0 90".to_owned(),
                Name::Scale => "Scale 0,0,0 2".to_owned(),
                Name::Mirror => "Mirror 0,0,0 0,1,0".to_owned(),
                _ => unreachable!(),
            };
            if let Some(value) = step.copy {
                input.push_str(if value { " Copy=Yes" } else { " Copy=No" });
            }
            registry.execute(&mut document, &input).is_ok()
        };
        records.push(
            json!({"command":step.command,"copy":step.copy,"finish":step.finish,
            "succeeded":succeeded,"before":before,"after":snapshot(&document)?}),
        );
    }
    Ok((json!({"records":records}), 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_defaults_completions_cancellations_and_reenabling_match_native() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/copy_options.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/copy_options.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        let mut expected = observed["results"][0]["value"].clone();
        assert_eq!(expected["records"].as_array().unwrap().len(), 50);
        assert_eq!(actual.results.len(), 1);
        assert_eq!(observed["results"][0]["id"], actual.results[0].id);
        for record in expected["records"].as_array_mut().unwrap() {
            let result = if record["finish"] == "Cancel" {
                "Cancel"
            } else if record["command"] == "Rotate"
                && record["after"].as_array().unwrap().len() == 2
            {
                "Nothing"
            } else {
                "Success"
            };
            let events = record["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["name"] == record["command"])
                .collect::<Vec<_>>();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["result"], result);
            assert_eq!(record["succeeded"], result == "Success");
            for key in ["history", "query", "events"] {
                record.as_object_mut().unwrap().remove(key);
            }
        }
        let mut actual = actual.results[0].value.clone();
        for (actual, expected) in actual["records"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(expected["records"].as_array_mut().unwrap())
        {
            let completed = actual["finish"] == "Complete";
            assert_eq!(
                actual["succeeded"], completed,
                "{} {}",
                actual["command"], actual["finish"]
            );
            // Rhino returns Nothing when Enter closes Rotate's Copy=Yes
            // repetition, although the edit and preference are committed.
            // Our registry's single-target API reports the edit's success.
            if expected["command"] != "Rotate" || !completed {
                assert_eq!(actual["succeeded"], expected["succeeded"]);
            } else {
                assert_ne!(actual["before"], actual["after"]);
            }
            actual.as_object_mut().unwrap().remove("succeeded");
            expected.as_object_mut().unwrap().remove("succeeded");
        }
        crate::test_json::close(&actual, &expected, "Copy workflows", 1e-9, 0.);
    }
}
