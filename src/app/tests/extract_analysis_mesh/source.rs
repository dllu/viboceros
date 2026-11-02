use super::*;
use viboceros_document::{
    BlockContent, BlockMember, BlockReference, ObjectAttributes, ObjectColorSource,
};
use viboceros_geometry::{AffineTransform3, MeshFace, SubD, Vector3};
fn created(commands: &[&str]) -> Geometry {
    let r = viboceros_command::CommandRegistry::with_builtins();
    let mut d = Document::default();
    for input in commands {
        r.execute(&mut d, input).unwrap();
    }
    d.objects().last().unwrap().geometry().clone()
}
fn plane_mesh() -> viboceros_geometry::TriangleMesh {
    viboceros_geometry::TriangleMesh::try_new_faces(
        [[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn geometry(name: &str) -> Geometry {
    match name {
        "plane" | "block" | "block_multi" => created(&["SrfPt 0,0,0 3,0,0 3,2,0 0,2,0"]),
        "mesh" => Geometry::Mesh(plane_mesh()),
        "subd_plane" => Geometry::SubD(
            SubD::try_from_mesh(&plane_mesh(), Default::default(), Tolerance::DEFAULT).unwrap(),
        ),
        "line" => created(&["Line 0,0,0 3,0,0"]),
        "extrusion" => created(&["Circle 0,0,0 2", "SelAll", "ExtrudeCrv 5 Solid=Yes"]),
        source => viboceros_oracle::curvature_analysis::Fixture {
            source: source.into(),
            style: "Gaussian".into(),
            range: [-1., 1.],
            preselect: true,
            steps: vec![],
        }
        .source(Tolerance::DEFAULT)
        .unwrap(),
    }
}
pub(super) fn install(app: &mut VibocerosApp, name: &str) -> ObjectId {
    let default_layer = app.document.layers().next().unwrap().id();
    let source_layer = app
        .document
        .add_layer("ExtractionSource", ColorRgb::BLACK)
        .unwrap();
    let destination = app
        .document
        .add_layer("ExtractionDestination", ColorRgb::BLACK)
        .unwrap();
    app.document.set_current_layer(destination).unwrap();
    let attrs = ObjectAttributes::on_layer(source_layer)
        .with_name("Analysis source")
        .with_object_color(ColorRgb::new(10, 20, 30))
        .try_with_wire_density(7)
        .unwrap()
        .try_with_user_text("analysis-probe", "preserved")
        .unwrap();
    let shape = geometry(name);
    let id = if name.starts_with("block") {
        let raw = ObjectAttributes::on_layer(default_layer);
        let mut members = vec![BlockMember::new(
            BlockContent::Geometry(shape.clone().into()),
            raw,
        )];
        if name == "block_multi" {
            members[0] = BlockMember::new(
                BlockContent::Geometry(shape.clone().into()),
                ObjectAttributes::on_layer(source_layer)
                    .with_name("First member")
                    .with_color_source(ObjectColorSource::Parent)
                    .try_with_wire_density(2)
                    .unwrap(),
            );
            members.push(BlockMember::new(
                BlockContent::Geometry(
                    shape
                        .transformed(
                            AffineTransform3::from_translation(
                                Vector3::try_from([0., 0., 3.]).unwrap(),
                            ),
                            Tolerance::DEFAULT,
                        )
                        .unwrap()
                        .into(),
                ),
                attrs
                    .clone()
                    .with_name("Second member")
                    .try_with_wire_density(5)
                    .unwrap(),
            ));
            members.push(BlockMember::new(
                BlockContent::Geometry(geometry("line").into()),
                ObjectAttributes::on_layer(default_layer),
            ));
            members.push(BlockMember::new(
                BlockContent::Geometry(geometry("mesh").into()),
                ObjectAttributes::on_layer(default_layer),
            ));
        }
        let definition = app
            .document
            .add_block_definition("Analysis source block", members)
            .unwrap();
        app.document
            .add_block_instance_with_attributes(
                BlockReference::try_new(
                    definition,
                    AffineTransform3::from_translation(Vector3::try_from([5., 2., 1.]).unwrap()),
                )
                .unwrap(),
                attrs,
            )
            .unwrap()
    } else {
        let id = app
            .document
            .add_geometry_with_attributes(shape, attrs)
            .unwrap();
        app.document
            .set_object_geometry_user_text([id], "geometry-probe", Some("source geometry"))
            .unwrap();
        id
    };
    app.document
        .add_group(Some("Analysis source group".into()), [id])
        .unwrap();
    id
}
