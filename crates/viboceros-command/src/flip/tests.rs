use super::*;

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn solid() -> Brep {
    Brep::try_box(
        CommandContext::default().construction_plane,
        [[-1., 1.]; 3],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn mesh() -> TriangleMesh {
    TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(3., 0., 0.),
            point(0., 4., 0.),
            point(0., 0., 5.),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn mixed_flip_preserves_identity_metadata_uvs_and_skipped_group_members() {
    for post in [false, true] {
        let mut doc = Document::default();
        let b = solid();
        let open = b.sub_brep(&[0, 1, 2, 3, 4], doc.tolerance()).unwrap();
        let sources = [
            Geometry::Brep(b.clone()),
            Geometry::Brep(open.clone()),
            Geometry::NurbsSurface(b.faces()[0].surface().clone()),
            Geometry::Mesh(mesh()),
            Geometry::Line(
                LineSegment::try_new(point(0., 0., 0.), point(1., 1., 1.), doc.tolerance())
                    .unwrap(),
            ),
            Geometry::Point(point(3., 2., 1.)),
            Geometry::Brep(b.reversed()),
        ];
        let ids = sources
            .into_iter()
            .enumerate()
            .map(|(i, geometry)| {
                doc.add_geometry_with_attributes(
                    geometry,
                    ObjectAttributes::on_layer(doc.current_layer_id())
                        .with_name(format!("source-{i}"))
                        .with_object_color(ColorRgb::new(10, 20, 30)),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let group = doc
            .add_group(Some("Mixed".into()), ids.iter().copied())
            .unwrap();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        let result = if post {
            registry.execute_postselected(&mut doc, "Flip", Default::default())
        } else {
            registry.execute(&mut doc, "Flip")
        };
        assert_eq!(result.unwrap(), "Flipped 4 object(s)");
        let after = doc.objects().cloned().collect::<Vec<_>>();
        assert_eq!(after.len(), before.len());
        for (i, (old, new)) in before.iter().zip(&after).enumerate() {
            assert_eq!(new.id(), old.id());
            assert_eq!(new.attributes(), old.attributes());
            assert_eq!(new.group_ids(), &[group]);
            let skipped = matches!(i, 0 | 5 | 6);
            assert_eq!(doc.is_selected(new.id()), !post || skipped);
            if skipped {
                assert_eq!(new, old);
            }
        }
        let Geometry::Brep(flipped) = after[1].geometry() else {
            panic!("open B-rep")
        };
        assert_eq!(flipped.vertices(), open.vertices());
        assert_eq!(flipped.edges(), open.edges());
        for (a, b) in flipped.faces().iter().zip(open.faces()) {
            assert_ne!(a.is_reversed(), b.is_reversed());
            assert_eq!(a.surface(), b.surface());
            assert_eq!(a.loops(), b.loops());
        }
        let Geometry::Brep(plane) = after[2].geometry() else {
            panic!("oriented natural face")
        };
        assert!(plane.faces()[0].is_reversed());
        assert_eq!(plane.faces()[0].surface(), b.faces()[0].surface());
        let Geometry::Mesh(m) = after[3].geometry() else {
            panic!("mesh")
        };
        assert_eq!(m.vertices(), mesh().vertices());
        assert_eq!(m.triangles(), &[[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]]);
        let Geometry::Line(line) = after[4].geometry() else {
            panic!("line")
        };
        assert_eq!(line.start(), point(1., 1., 1.));
        assert_eq!(line.end(), point(0., 0., 0.));
        assert_eq!(doc.undo_label(), Some("Flip"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    }
}

#[test]
fn repeated_surface_flip_preserves_rational_parameterization_and_trim_topology() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let surface = crate::tests::rational_multi_span_surface();
    let original = Brep::try_surface_face(surface.clone(), doc.tolerance()).unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = doc.object(id).unwrap().clone();
    for reversed in [true, false, true, false] {
        registry.execute(&mut doc, "Flip").unwrap();
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!("B-rep")
        };
        assert_eq!(b.faces()[0].is_reversed(), reversed);
        assert_eq!(b.faces()[0].surface(), original.faces()[0].surface());
        assert_eq!(b.faces()[0].loops(), original.faces()[0].loops());
        assert_eq!(b.edges(), original.edges());
        assert_eq!(b.vertices(), original.vertices());
    }
    for _ in 0..4 {
        registry.execute(&mut doc, "Undo").unwrap();
    }
    assert_eq!(doc.object(id), Some(&before));
    for _ in 0..4 {
        registry.execute(&mut doc, "Redo").unwrap();
    }
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::Brep(original)
    );
}

#[test]
fn all_skipped_inputs_preserve_geometry_selection_and_redo() {
    for post in [false, true] {
        let mut doc = Document::default();
        let sphere =
            NurbsSurface::try_sphere(CommandContext::default().construction_plane, 2.).unwrap();
        let b = solid();
        let ids = [
            Geometry::Point(point(0., 0., 0.)),
            Geometry::Brep(b.clone()),
            Geometry::Brep(b.reversed()),
            Geometry::NurbsSurface(sphere),
        ]
        .into_iter()
        .map(|g| doc.add_geometry(g).unwrap())
        .collect::<Vec<_>>();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 99,99,99").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        let before = format!("{doc:?}");
        let message = if post {
            registry.execute_postselected(&mut doc, "Flip", Default::default())
        } else {
            registry.execute(&mut doc, "Flip")
        }
        .unwrap();
        assert_eq!(message, "Flipped 0 object(s)");
        assert_eq!(format!("{doc:?}"), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), 5);
    }
}

#[test]
fn periodic_open_surface_is_not_mistaken_for_a_closed_solid() {
    let mut doc = Document::default();
    let surface =
        NurbsSurface::try_cylinder(CommandContext::default().construction_plane, 2., -3., 7.)
            .unwrap();
    let original = Brep::try_surface_face(surface.clone(), doc.tolerance()).unwrap();
    assert!(!original.is_closed());
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    assert_eq!(
        registry.execute(&mut doc, "Flip").unwrap(),
        "Flipped 1 object(s)"
    );
    let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
        panic!("B-rep")
    };
    assert!(b.faces()[0].is_reversed());
    assert_eq!(b.faces()[0].surface(), original.faces()[0].surface());
    assert_eq!(b.faces()[0].loops(), original.faces()[0].loops());
    assert_eq!(b.edges(), original.edges());
    assert_eq!(b.vertices(), original.vertices());
}

#[test]
fn closed_but_inconsistently_oriented_breps_can_flip_and_keep_their_inconsistency() {
    for (mask, reverse, post) in [1_u8, 7].into_iter().flat_map(|mask| {
        [false, true]
            .into_iter()
            .flat_map(move |reverse| [false, true].map(|post| (mask, reverse, post)))
    }) {
        let mut doc = Document::default();
        let b = solid();
        let faces = b
            .faces()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                BrepFace::try_new(
                    f.surface().clone(),
                    (mask & (1 << i) != 0) ^ reverse,
                    f.loops().to_vec(),
                )
                .unwrap()
            })
            .collect();
        let b = Brep::try_new(
            b.vertices().to_vec(),
            b.edges().to_vec(),
            faces,
            doc.tolerance(),
        )
        .unwrap();
        assert!(b.is_closed());
        assert!(!b.is_solid());
        let id = doc.add_geometry(Geometry::Brep(b.clone())).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let before = doc.object(id).unwrap().clone();
        let registry = CommandRegistry::with_builtins();
        let message = if post {
            registry.execute_postselected(&mut doc, "Flip", Default::default())
        } else {
            registry.execute(&mut doc, "Flip")
        }
        .unwrap();
        assert_eq!(message, "Flipped 1 object(s)");
        let after = doc.object(id).unwrap().clone();
        let Geometry::Brep(flipped) = after.geometry() else {
            panic!("B-rep")
        };
        assert!(flipped.is_closed());
        assert!(!flipped.is_solid());
        assert_eq!(flipped.vertices(), b.vertices());
        assert_eq!(flipped.edges(), b.edges());
        for (f, source) in flipped.faces().iter().zip(b.faces()) {
            assert_ne!(f.is_reversed(), source.is_reversed());
            assert_eq!(f.surface(), source.surface());
            assert_eq!(f.loops(), source.loops());
        }
        assert_eq!(doc.is_selected(id), !post);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id), Some(&before));
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.object(id), Some(&after));
    }
}

#[test]
fn prompt_accepts_skipped_peers_and_invalid_arguments_do_not_mutate() {
    let prompt = FlipCommand.object_selection_prompt(&[]).unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Any);
    assert!(FlipCommand.object_selection_prompt(&["FlipAll"]).is_err());
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    assert!(matches!(
        registry.execute(&mut doc, "Flip"),
        Err(CommandError::NoObjectsSelected)
    ));
    let id = doc.add_geometry(Geometry::Mesh(mesh())).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute_postselected(&mut doc, "Flip Wrong", Default::default())
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}
