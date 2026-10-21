use viboceros_command::CommandRegistry;
#[path = "support/face_create.rs"]
mod native;

#[test]
fn direct_mesh_faces_match_native_geometry_ngons_attributes_and_grouped_history() {
    let mut gaps = Vec::new();
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut scene.document, &native::command(value))
            .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
        let check = native::check(
            &scene.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        if check["qualified"] != true {
            gaps.push(row["id"].clone());
        }
        assert_eq!(scene.document.undo_label(), Some("3DFace"));
        scene.document.undo().unwrap();
        assert_eq!(
            native::check(
                &scene.document,
                scene.source,
                scene.original.as_ref(),
                value,
                "undo"
            )["qualified"],
            true
        );
        scene.document.redo().unwrap();
        assert_eq!(
            native::check(
                &scene.document,
                scene.source,
                scene.original.as_ref(),
                value,
                "redo"
            ),
            check
        );
        println!(
            "FACE_CREATE_COMMAND {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"],"check":check,"grouped_history":true})
        );
    }
    assert!(gaps.is_empty(), "{gaps:?}");
}

#[test]
fn invalid_faces_and_options_fail_without_consuming_redo() {
    let mut scene = native::setup(&native::rows()[0]["value"]);
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut scene.document, "Point 9,9,9")
        .unwrap();
    scene.document.undo().unwrap();
    for command in [
        "3DFace 0,0,0 1,0,0",
        "3DFace 0,0,0 1,0,0 2,0,0",
        "3DFace Output=SubD 0,0,0 1,0,0 1,1,0",
        "3DFace Chain=Yes",
        "3DFace WeldAngle=NaN",
        "3DFace WeldAngle=181",
        "3DFace FromEdge=Yes Mode=MultipleFaces",
        "3DFace FromEdge=Yes Edge=0",
        "3DFace InPlane=Maybe",
    ] {
        assert!(
            registry.execute(&mut scene.document, command).is_err(),
            "{command}"
        );
        assert_eq!(scene.document.objects().count(), 0);
        assert_eq!(scene.document.redo_label(), Some("Point"));
    }
    use viboceros_command::face_create::{Draft, Preferences};
    for angle in [f64::NAN, f64::INFINITY, -1., 181.] {
        assert!(
            Draft::new(
                Preferences {
                    weld_angle: angle,
                    ..Preferences::default()
                },
                None,
                None
            )
            .is_err()
        );
    }
}

#[test]
fn edge_chain_scripts_retain_all_native_mesh_and_rejection_results() {
    use viboceros_command::face_create::{Draft, Token};
    use viboceros_geometry::Point3;
    let mut gaps = vec![];
    for row in native::drafts()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        let header = native::draft_header(value);
        let words = header.split_whitespace().collect::<Vec<_>>();
        let input = registry.face_create_input(&words[1..]).unwrap();
        let mut draft = Draft::new(
            input.preferences,
            viboceros_command::face_create::source(&scene.document, scene.source),
            None,
        )
        .unwrap();
        let mut tokens = vec![];
        for action in value["input"]["points"].as_array().unwrap() {
            let (text, token) = if let Some(pick) = action.get("edge_pick") {
                let edge = native::edge_index(&draft.current_mesh().unwrap().unwrap(), pick);
                (format!("Edge={edge}"), Token::Edge(edge))
            } else if let Some(word) = action.as_str() {
                (
                    word.to_owned(),
                    if word == "Undo" {
                        Token::Undo
                    } else {
                        Token::Enter
                    },
                )
            } else {
                let p: [f64; 3] = serde_json::from_value(action.clone()).unwrap();
                (
                    p.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    Token::Point(Point3::try_from(p).unwrap()),
                )
            };
            let _ = draft.accept(token);
            tokens.push(text);
        }
        let command = format!("{header} {} Enter Enter", tokens.join(" "));
        let result = registry.execute(&mut scene.document, &command);
        let check = native::check(
            &scene.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        if check["qualified"] != true {
            gaps.push(row["id"].clone());
        }
        if result.is_ok() {
            scene.document.undo().unwrap();
            assert_eq!(
                native::check(
                    &scene.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "undo"
                )["qualified"],
                true
            );
            scene.document.redo().unwrap();
            assert_eq!(
                native::check(
                    &scene.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "redo"
                ),
                check
            );
        } else {
            assert!(scene.document.undo_label().is_none());
            if let Some(source) = scene.source {
                assert!(scene.original.as_ref().unwrap().shares_storage_with(
                    scene.document.object(source).unwrap().geometry_snapshot()
                ));
            } else {
                assert_eq!(scene.document.objects().count(), 0);
            }
        }
        println!(
            "FACE_DRAFT_COMMAND {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"],"check":check,"grouped_history":true,"local_error":result.err().map(|e|e.to_string())})
        );
    }
    assert!(gaps.is_empty(), "{gaps:?}");
    println!("FACE_DRAFT_COMMAND_GAPS {gaps:?}");
}
