use super::*;
use serde_json::Value;
fn close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-8_f64.max(1e-12 * a.abs().max(b.abs())),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert!(a.keys().eq(b.keys()), "{path}");
            for (k, a) in a {
                close(a, &b[k], &format!("{path}/{k}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn complete_native_delete_grips_application_and_history() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("fixtures/delete_grips.json")).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("observations/delete_grips.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 397);
    let mut failed = Vec::new();
    for (op, expected) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let mut op = op.clone();
        let filename = std::path::Path::new(op["artifact_path"].as_str().unwrap())
            .file_name()
            .unwrap();
        op["artifact_path"] =
            serde_json::json!(folder.join("fixtures/delete_grips").join(filename));
        let f: viboceros_oracle::delete_grips::Fixture = serde_json::from_value(op).unwrap();
        let state = viboceros_oracle::delete_grips::prepare(&f).unwrap();
        let mut app = test_app();
        app.document = state.document;
        let name = expected["id"].as_str().unwrap();
        let check = |app: &VibocerosApp, v: &Value, path: &str| {
            let current = viboceros_oracle::delete_grips::snapshot(
                &app.document,
                &state.ids,
                state.peer,
                state.marker,
                state.group,
            )
            .unwrap();
            close(&current, v, path);
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            check(&app, &expected["value"]["before"], name);
            for step in expected["value"]["steps"].as_array().unwrap() {
                check(&app, &step["before"], &format!("{name}/before"));
                app.command_input = step["command"].as_str().unwrap().into();
                app.run_command();
                assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
                check(
                    &app,
                    &step["after_script"],
                    &format!("{name}/{}", step["command"]),
                );
            }
        }));
        if outcome.is_err() {
            failed.push(name.to_owned());
        }
    }
    assert!(failed.is_empty(), "Delete UI differences: {failed:?}");
}
