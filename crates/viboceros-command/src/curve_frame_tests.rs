use super::*;

#[test]
fn freeform_array_uses_incoming_corner_tangents_and_undoes_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Polyline 0,0,0 1,0,0 1,1,0 1,1,1",
        "SelLast",
        "SetObjectName Rail",
        "Line 0,0,0 1,0,0",
        "SelLast",
        "SetObjectName Witness",
    ] {
        registry.execute(&mut document, command).unwrap();
    }
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(
            &mut document,
            "ArrayCrv 4 Orientation=Freeform PathName=Rail",
        )
        .unwrap();
    for (start, end) in [
        ([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]),
        ([1.0, 1.0, 0.0], [1.0, 2.0, 0.0]),
        ([1.0, 1.0, 1.0], [1.0, 1.0, 2.0]),
    ] {
        assert!(document.objects().any(|object| {
            matches!(object.geometry(), Geometry::Line(line)
                if line.start().to_array() == start && line.end().to_array() == end)
        }));
    }
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}
