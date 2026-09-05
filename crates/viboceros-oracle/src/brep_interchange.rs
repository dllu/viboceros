//! Both readers inspect the same native-fitted B-rep, not separate morph fits.

use super::{BrepMorphFixture, OracleTemporaryFile, ProbeError, brep_morph, trimmed_brep};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{
    Brep, BrepFace, BrepTrim, Frame3, GeometryError, NurbsCurve, NurbsSurface, Point3, PointMorph,
    SurfaceIso, SurfacePointMorph, Tolerance, Vector3,
};
use viboceros_io::{
    ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file, write_3dm_file,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BrepInterchangeFixture {
    #[serde(flatten)]
    pub source: BrepInterchangeSource,
    /// The host assigns an owned, unique path for cross-reader comparisons.
    #[serde(default)]
    pub artifact_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrepInterchangeSource {
    PointGrid {
        #[serde(flatten)]
        fixture: Box<super::PointGridFixture>,
    },
    EdgeSurface {
        #[serde(flatten)]
        fixture: Box<super::EdgeSurfaceFixture>,
    },
    Loft {
        #[serde(flatten)]
        fixture: Box<super::LoftFixture>,
    },
    SurfaceMorph {
        #[serde(flatten)]
        fixture: Box<BrepMorphFixture>,
    },
    CubicLift {
        primitive: LiftPrimitive,
        fit_tolerance: f64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LiftPrimitive {
    Box,
    Cylinder,
    Cone,
    Sphere,
}

struct CubicLift;
impl PointMorph for CubicLift {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(
            p.x(),
            p.y(),
            p.z() + p.x().powi(2) + p.x() * p.y() * 0.25 + p.y().powi(3),
        )
    }
}

impl BrepInterchangeSource {
    fn tolerance(&self, tolerance: Tolerance) -> Result<Tolerance, GeometryError> {
        let absolute = match self {
            Self::Loft { .. } | Self::EdgeSurface { .. } | Self::PointGrid { .. } => {
                tolerance.absolute()
            }
            Self::SurfaceMorph { fixture } => fixture.fit_tolerance,
            Self::CubicLift { fit_tolerance, .. } => *fit_tolerance,
        };
        Tolerance::try_new(absolute, tolerance.relative(), tolerance.angular())
    }

    fn build(&self, tolerance: Tolerance) -> Result<Brep, ProbeError> {
        Ok(match self {
            Self::PointGrid { fixture } => super::point_grid::build_brep(fixture, tolerance)?,
            Self::EdgeSurface { fixture } => super::edge_surface::build_brep(fixture, tolerance)?,
            Self::Loft { fixture } => super::loft::build_brep(fixture, tolerance)?,
            Self::SurfaceMorph { fixture } => {
                let source = trimmed_brep::build(&fixture.source, tolerance)?;
                let target = super::nurbs_surface_from_definition(&fixture.surface)?;
                let frame = Frame3::try_from_directions(
                    Point3::try_from(fixture.source_origin)?,
                    Vector3::try_from(fixture.source_x)?,
                    Vector3::try_from(fixture.source_y)?,
                    tolerance,
                )?;
                SurfacePointMorph::try_new(
                    frame,
                    &target,
                    fixture.uv[0],
                    fixture.uv[1],
                    fixture.scale,
                    fixture.angle,
                    false,
                    tolerance,
                )?
                .morph_brep(&source, tolerance)?
            }
            Self::CubicLift { primitive, .. } => {
                let frame = Frame3::try_from_directions(
                    Point3::try_new(0.0, 0.0, 0.0)?,
                    Vector3::try_new(1.0, 0.0, 0.0)?,
                    Vector3::try_new(0.0, 1.0, 0.0)?,
                    tolerance,
                )?;
                let source = match primitive {
                    LiftPrimitive::Box => Brep::try_box(frame, [[0.0, 1.0]; 3], tolerance)?,
                    LiftPrimitive::Cylinder => Brep::try_cylinder(frame, 0.4, 0.0, 1.0, tolerance)?,
                    LiftPrimitive::Cone => Brep::try_cone(frame, 0.4, 1.0, tolerance)?,
                    LiftPrimitive::Sphere => {
                        Brep::try_surface_face(NurbsSurface::try_sphere(frame, 0.4)?, tolerance)?
                    }
                };
                CubicLift.morph_brep(&source, tolerance)?
            }
        })
    }
}

pub(super) fn run(
    fixture: &BrepInterchangeFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let tolerance = fixture.source.tolerance(tolerance)?;
    let source = fixture.source.build(tolerance)?;
    let expected = geometry_record(&source)?;
    let model = ThreeDmModel::new(
        vec![ThreeDmLayer {
            name: "Geometry".into(),
            color: [30, 60, 90],
            visible: true,
            locked: false,
        }],
        vec![],
        vec![ThreeDmObject::new(ThreeDmGeometry::Brep(source), 0)],
    );
    let temporary = OracleTemporaryFile::new("brep-interchange");
    let path = fixture
        .artifact_path
        .as_deref()
        .map(Path::new)
        .unwrap_or(&temporary.path);
    // Never overwrite an existing caller-owned file, even on a failed export.
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?,
    );
    write_3dm_file(path, &model)?;
    let mut decoded = read(path, tolerance)?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        decoded = std::hint::black_box(read(path, tolerance)?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let mut record = geometry_record(&decoded)?;
    if !roundtrip_equal(&record, &expected) {
        return Err(ProbeError::FixtureInvariant(
            "B-rep export changed native topology or geometry",
        ));
    }
    // Meshing is a separate usability check, excluded from reader timings.
    let mesh = decoded.polygon_mesh(0.0, false, false, tolerance)?;
    let topology = mesh.topology();
    let boundaries = mesh.boundary_polylines(tolerance)?;
    record["mesh"] = json!({"closed": topology.is_closed(), "manifold": topology.is_manifold(),
        "oriented": topology.orientation_conflict_edge_count() == 0,
        "boundary_loops": boundaries.len(), "boundaries_closed": boundaries.iter().all(|b| b.is_closed())});
    Ok((record, elapsed))
}

fn read(path: &Path, tolerance: Tolerance) -> Result<Brep, ProbeError> {
    let mut model = read_3dm_file(path, tolerance)?;
    if model.unsupported_object_count() != 0 || model.objects.len() != 1 {
        return Err(ProbeError::FixtureInvariant(
            "interchange must contain exactly one supported B-rep",
        ));
    }
    match model.objects.pop().unwrap().geometry {
        ThreeDmGeometry::Brep(brep) => Ok(brep),
        _ => Err(ProbeError::FixtureInvariant(
            "interchange geometry is not a B-rep",
        )),
    }
}

fn curve_record(curve: &NurbsCurve) -> Result<Value, GeometryError> {
    let samples = (0..=32)
        .map(|i| {
            curve
                .evaluate(curve.parameter_at(i as f64 / 32.0)?)
                .map(|p| p.to_array())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"definition": serialized_definition(super::nurbs_curve_definition_value(curve)), "samples": samples}),
    )
}

pub(super) fn geometry_record(brep: &Brep) -> Result<Value, GeometryError> {
    let edges = brep
        .edges()
        .iter()
        .map(|edge| {
            Ok(json!({
                "tolerance": edge.tolerance(), "curve": curve_record(edge.curve())?,
            }))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let faces = brep
        .faces()
        .iter()
        .map(face_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({"topology": brep_morph::topology(brep),
        "vertices": brep.vertices().iter().map(|v| json!({"point": v.point().to_array(), "tolerance": v.tolerance()})).collect::<Vec<_>>(),
        "edges": edges, "faces": faces}))
}

fn face_record(face: &BrepFace) -> Result<Value, GeometryError> {
    let surface = face.surface();
    let mut samples = Vec::new();
    for j in 0..=8 {
        for i in 0..=8 {
            samples.push(
                surface
                    .evaluate(
                        surface.parameter_at_u(i as f64 / 8.0)?,
                        surface.parameter_at_v(j as f64 / 8.0)?,
                    )?
                    .to_array(),
            );
        }
    }
    let loops = face
        .loops()
        .iter()
        .map(|boundary| {
            boundary
                .trims()
                .iter()
                .map(|trim| trim_record(trim, surface))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"definition": serialized_definition(super::nurbs_surface_definition_value(surface)), "samples": samples, "loops": loops}),
    )
}

fn trim_record(trim: &BrepTrim, surface: &NurbsSurface) -> Result<Value, GeometryError> {
    let mut lifted = Vec::new();
    for i in 0..=32 {
        let uv = trim
            .curve()
            .evaluate(trim.curve().parameter_at(i as f64 / 32.0)?)?;
        lifted.push(surface.evaluate(uv.x(), uv.y())?.to_array());
    }
    let iso = match trim.iso() {
        SurfaceIso::NotIso => 0,
        SurfaceIso::InteriorUConstant => 1,
        SurfaceIso::InteriorVConstant => 2,
        SurfaceIso::West => 3,
        SurfaceIso::South => 4,
        SurfaceIso::East => 5,
        SurfaceIso::North => 6,
    };
    Ok(json!({"iso": iso, "tolerance": trim.tolerance(),
        "definition": serialized_definition(super::nurbs_curve2_definition_value(trim.curve())), "lifted": lifted}))
}

// OpenNURBS stores the interior N+p-1 knot entries, omitting the two
// mathematically unused outer entries in our full vector. Its reader can
// reconstruct those entries differently for unclamped/periodic surfaces.
// Match Rhino's oracle representation (repeat first/last stored knot), without
// discarding ANY serialized coefficient, active domain, or geometric sample.
fn serialized_definition(mut definition: Value) -> Value {
    for key in ["knots", "knots_u", "knots_v"] {
        if let Some(knots) = definition.get_mut(key).and_then(Value::as_array_mut) {
            let last = knots.len() - 1;
            knots[0] = knots[1].clone();
            knots[last] = knots[last - 1].clone();
        }
    }
    definition
}

// Serialize/deserialize may round Euclidean rational control coordinates by a
// few ulps. Compare every coefficient AND sample, with a fixed IO tolerance
// independent of the much larger morph fitting tolerance. Integers stay exact.
fn roundtrip_equal(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() && b.is_f64() => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            (a - b).abs() <= 2e-12 + 1e-14 * a.abs().max(b.abs())
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| roundtrip_equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, a)| b.get(k).is_some_and(|b| roundtrip_equal(a, b)))
        }
        _ => actual == expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interchange_canonicalizes_only_the_two_unstored_outer_knots() {
        let source = json!({"knots": [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0], "degree": 2});
        let serialized = serialized_definition(source.clone());
        assert_eq!(
            serialized["knots"],
            json!([-2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 2.0])
        );
        assert_eq!(serialized["degree"], source["degree"]);
        let mut changed = source;
        changed["knots"][3] = json!(0.01);
        assert!(!roundtrip_equal(
            &serialized,
            &serialized_definition(changed)
        ));
    }

    #[test]
    fn interchange_never_overwrites_an_existing_artifact() {
        let file = OracleTemporaryFile::new("brep-interchange-existing");
        std::fs::write(&file.path, b"existing contents").unwrap();
        let fixture = BrepInterchangeFixture {
            source: BrepInterchangeSource::CubicLift {
                primitive: LiftPrimitive::Box,
                fit_tolerance: 1e-6,
            },
            artifact_path: Some(file.path.to_str().unwrap().into()),
        };
        let error = run(&fixture, 1, Tolerance::DEFAULT).unwrap_err();
        assert!(
            matches!(error, ProbeError::Io(e) if e.kind() == std::io::ErrorKind::AlreadyExists)
        );
        assert_eq!(std::fs::read(&file.path).unwrap(), b"existing contents");
    }

    #[test]
    fn roundtrip_comparison_does_not_use_the_morph_tolerance_or_ignore_structure() {
        assert!(roundtrip_equal(
            &json!({"p": [1.0 + 1e-13]}),
            &json!({"p": [1.0]})
        ));
        for actual in [
            json!({"p": [1.0 + 1e-9]}),
            json!({"p": [1.0, 1.0]}),
            json!({"q": [1.0]}),
        ] {
            assert!(!roundtrip_equal(&actual, &json!({"p": [1.0]})));
        }
        assert!(!roundtrip_equal(
            &json!(1000000000000001u64),
            &json!(1000000000000000u64)
        ));
    }

    #[test]
    fn fitted_breps_roundtrip_with_topology_coefficients_and_usable_meshes() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/brep_3dm_interchange.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 8);
        for result in response.results {
            let expected_boundaries = match result.id.as_str() {
                "brep-3dm-disk" => 1,
                "brep-3dm-warped-annulus" => 2,
                _ => 0,
            };
            assert_eq!(
                result.value["mesh"]["boundary_loops"], expected_boundaries,
                "{}",
                result.id
            );
            assert_eq!(
                result.value["mesh"]["boundaries_closed"], true,
                "{}",
                result.id
            );
            assert_eq!(
                result.value["mesh"]["closed"], result.value["topology"]["solid"],
                "{}",
                result.id
            );
            assert_eq!(result.value["mesh"]["manifold"], true, "{}", result.id);
            assert_eq!(result.value["mesh"]["oriented"], true, "{}", result.id);
        }
    }
}
