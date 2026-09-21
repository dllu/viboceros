//! Shared native sources for B-rep command comparisons.
use super::*;
use crate::{curve_join_close::CurveInput, object_source::ObjectSource};

/// Shared topology-preserving source preparation for geometry and command probes.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(super) struct BrepSourceFixture {
    source: BrepCommandSource,
    #[serde(default)]
    reversed: bool,
    edge_order: Option<Vec<usize>>,
    #[serde(default)]
    splits: Vec<(usize, Vec<f64>)>,
    pub(super) artifact_path: Option<String>,
}

impl BrepSourceFixture {
    pub(super) fn build(&self, tolerance: Tolerance) -> Result<Brep, ProbeError> {
        let mut brep = match self.source.geometry(tolerance)? {
            Geometry::Brep(brep) => brep,
            Geometry::NurbsSurface(surface) => Brep::try_surface_face(surface, tolerance)?,
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "B-rep source needs a surface or B-rep",
                ));
            }
        };
        if let Some(order) = &self.edge_order {
            brep = brep.reordered_edges(order, tolerance)?;
        }
        if !self.splits.is_empty() {
            brep = brep.try_split_edges_at_parameters(&self.splits, tolerance)?;
        }
        if self.reversed {
            brep = brep.reversed();
        }
        Ok(brep)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(super) enum BrepCommandSource {
    Primitive(Primitive),
    Object(ObjectSource),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Primitive {
    Tube {
        radii: [f64; 2],
        height: f64,
    },
    MeshBrep {
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
    SurfaceFace {
        surface: NurbsSurfaceDefinition,
        trim_bounds: Option<[[f64; 2]; 2]>,
    },
    Box {
        min: [f64; 3],
        max: [f64; 3],
        keep_faces: Option<Vec<usize>>,
    },
    Extrusion {
        curve: CurveInput,
        vector: [f64; 3],
    },
}

impl BrepCommandSource {
    pub(super) fn geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        Ok(match self {
            Self::Object(source) => source.geometry(tolerance)?,
            Self::Primitive(Primitive::Tube { radii, height }) => Geometry::Brep(
                Brep::try_tube(
                    viboceros_command::CommandContext::default().construction_plane,
                    *radii,
                    *height,
                    tolerance,
                )?
                .sub_brep(&[0, 1], tolerance)?,
            ),

            Self::Primitive(Primitive::MeshBrep { vertices, faces }) => {
                let source = ObjectSource::Vertices(crate::object_source::VertexSource::Mesh {
                    vertices: vertices.clone(),
                    faces: faces.clone(),
                })
                .geometry(tolerance)?;
                let Geometry::Mesh(mesh) = source else {
                    unreachable!()
                };
                Geometry::Brep(Brep::try_from_mesh(&mesh, true, tolerance)?)
            }
            Self::Primitive(Primitive::SurfaceFace {
                surface,
                trim_bounds,
            }) => {
                let surface = nurbs_surface_from_definition(surface)?;
                Geometry::Brep(if let Some([u, v]) = trim_bounds {
                    Brep::try_rectangular_surface_face(
                        surface,
                        u[0]..=u[1],
                        v[0]..=v[1],
                        tolerance,
                    )?
                } else {
                    Brep::try_surface_face(surface, tolerance)?
                })
            }
            Self::Primitive(Primitive::Box {
                min,
                max,
                keep_faces,
            }) => {
                let brep = Brep::try_box(
                    viboceros_command::CommandContext::default().construction_plane,
                    std::array::from_fn(|i| [min[i], max[i]]),
                    tolerance,
                )?;
                Geometry::Brep(if let Some(faces) = keep_faces {
                    brep.duplicate_faces(faces, tolerance)?
                } else {
                    brep
                })
            }
            Self::Primitive(Primitive::Extrusion { curve, vector }) => Geometry::Brep(
                Brep::try_extruded_curve(
                    &curve.geometry()?.as_ref().to_nurbs()?,
                    Vector3::try_new(0., 0., 0.)?,
                    Vector3::try_from(*vector)?,
                    tolerance,
                )?
                .duplicate_faces(&[0], tolerance)?,
            ),
        })
    }
}

pub(super) fn reorder_edges(
    brep: &Brep,
    order: &[usize],
    tolerance: Tolerance,
) -> Result<Brep, ProbeError> {
    Ok(brep.reordered_edges(order, tolerance)?)
}

pub(super) fn write_shared_artifact(
    geometry: &Geometry,
    path: &str,
    tolerance: Tolerance,
) -> Result<(), ProbeError> {
    use viboceros_io::{
        ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file,
    };
    let geometry = match geometry {
        Geometry::Brep(brep) => ThreeDmGeometry::Brep(brep.clone()),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "B-rep source artifact requires B-rep",
            ));
        }
    };
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Source".into(),
            color: [0, 0, 0],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(geometry, 0)],
    );
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?,
    );
    write_3dm_file(path, &model)?;
    let decoded = read_3dm_file(path, tolerance)?;
    let (
        ThreeDmGeometry::Brep(source),
        Some(ThreeDmObject {
            geometry: ThreeDmGeometry::Brep(restored),
            ..
        }),
    ) = (&model.objects[0].geometry, decoded.objects.first())
    else {
        return Err(ProbeError::FixtureInvariant(
            "B-rep source artifact lost its B-rep",
        ));
    };
    if !super::brep_interchange::roundtrip_equal(
        &super::brep_interchange::geometry_record(source)?,
        &super::brep_interchange::geometry_record(restored)?,
    ) {
        return Err(ProbeError::FixtureInvariant(
            "B-rep source artifact changed topology or geometry",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_face_fixture_keeps_underlying_surface_and_rejects_invalid_trim_bounds() {
        let source = serde_json::json!({"source":{"type":"surface_face","surface":{
            "degree_u":1,"degree_v":1,"control_point_count_u":2,"control_point_count_v":2,
            "control_points":[{"point":[0,0,0],"weight":1},{"point":[4,0,0],"weight":1},
                {"point":[0,4,0],"weight":1},{"point":[4,4,0],"weight":1}],
            "knots_u":[0,0,1,1],"knots_v":[0,0,1,1]
        },"trim_bounds":[[0.125,0.875],[0.25,0.75]]}});
        let fixture: BrepSourceFixture = serde_json::from_value(source.clone()).unwrap();
        let brep = fixture.build(Tolerance::DEFAULT).unwrap();
        assert_eq!(brep.faces()[0].surface().domain_u(), 0. ..=1.);
        assert_eq!(brep.faces()[0].surface().domain_v(), 0. ..=1.);
        let points = brep
            .vertices()
            .iter()
            .map(|v| v.point().to_array())
            .collect::<Vec<_>>();
        assert_eq!(
            points,
            [[0.5, 1., 0.], [3.5, 1., 0.], [3.5, 3., 0.], [0.5, 3., 0.]]
        );
        for bounds in [
            serde_json::json!([[0.5, 0.5], [0.25, 0.75]]),
            serde_json::json!([[-0.1, 0.5], [0., 1.]]),
            serde_json::json!([[0.75, 0.25], [0., 1.]]),
        ] {
            let mut invalid = source.clone();
            invalid["source"]["trim_bounds"] = bounds;
            let invalid: BrepSourceFixture = serde_json::from_value(invalid).unwrap();
            assert!(invalid.build(Tolerance::DEFAULT).is_err());
        }
        assert_eq!(fixture, serde_json::from_value(source).unwrap());
    }
}
