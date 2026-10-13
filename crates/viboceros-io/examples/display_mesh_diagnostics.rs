//! Optional real-model inventory and per-object viewport tessellation profile.
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf, time::Instant};
use viboceros_geometry::Brep;
use viboceros_io::{ThreeDmGeometry, read_3dm_file_with_model_tolerance};

struct Options {
    path: PathBuf,
    samples: usize,
    names: Vec<String>,
    inventory_only: bool,
    include_hidden: bool,
}

fn options() -> Result<Options, Box<dyn std::error::Error>> {
    let usage = "display_mesh_diagnostics FILE.3dm [--samples 8] [--name SUBSTRING] [--inventory-only] [--include-hidden]";
    let mut arguments = std::env::args().skip(1);
    let path = arguments.next().ok_or(usage)?;
    if path == "--help" || path == "-h" {
        println!("{usage}");
        std::process::exit(0);
    }
    let mut result = Options {
        path: path.into(),
        samples: 8,
        names: Vec::new(),
        inventory_only: false,
        include_hidden: false,
    };
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--samples" => result.samples = arguments.next().ok_or(usage)?.parse()?,
            "--name" => result.names.push(arguments.next().ok_or(usage)?),
            "--inventory-only" => result.inventory_only = true,
            "--include-hidden" => result.include_hidden = true,
            _ => return Err(format!("unknown option {argument}: {usage}").into()),
        }
    }
    if result.samples == 0 {
        return Err("samples per span must be positive".into());
    }
    Ok(result)
}

fn inventory(brep: &Brep, samples: usize) -> Vec<Value> {
    brep.faces().iter().enumerate().map(|(index, face)| {
        let surface = face.surface();
        let spans = [surface.spans_u().count(), surface.spans_v().count()];
        let grid_points = spans[0].checked_mul(samples).and_then(|n| n.checked_add(1))
            .and_then(|u| spans[1].checked_mul(samples).and_then(|n| n.checked_add(1)).and_then(|v| u.checked_mul(v)));
        let trims = face.loops().iter().flat_map(|boundary| boundary.trims()).collect::<Vec<_>>();
        json!({
            "face": index,
            "degree": [surface.degree_u(), surface.degree_v()],
            "controls": [surface.control_point_count_u(), surface.control_point_count_v()],
            "knots": [surface.knots_u().len(), surface.knots_v().len()],
            "spans": spans,
            "untrimmed_grid_points": grid_points,
            "loops": face.loops().len(),
            "trims": trims.len(),
            "trim_spans": trims.iter().map(|trim| trim.curve().spans().count()).sum::<usize>(),
            "max_trim_controls": trims.iter().map(|trim| trim.curve().control_points().len()).max().unwrap_or(0),
            "max_trim_knots": trims.iter().map(|trim| trim.curve().knots().len()).max().unwrap_or(0),
        })
    }).collect()
}

fn emit(output: &mut impl Write, value: Value) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer(&mut *output, &value)?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options()?;
    eprintln!("reading {}", options.path.display());
    let started = Instant::now();
    let model = read_3dm_file_with_model_tolerance(&options.path)?;
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    emit(
        &mut output,
        json!({
            "type": "model", "path": options.path, "objects": model.objects.len(),
            "breps": model.objects.iter().filter(|object| matches!(object.geometry, ThreeDmGeometry::Brep(_))).count(),
            "meshes": model.objects.iter().filter(|object| matches!(object.geometry, ThreeDmGeometry::Mesh(_))).count(),
            "unsupported": model.unsupported_object_count(), "read_seconds": started.elapsed().as_secs_f64(),
            "samples_per_span": options.samples, "absolute_tolerance": model.tolerance.absolute(),
        }),
    )?;
    let mut total_triangles = 0_u64;
    let mut total_vertices = 0_u64;
    let mut failed = 0;
    let mut processed = 0;
    let mut object_seconds = 0.0;
    for (index, object) in model.objects.iter().enumerate() {
        let name = object.name.as_deref().unwrap_or("");
        let visible = object.visible && model.layers[object.layer_index].visible;
        if (!options.include_hidden && !visible)
            || (!options.names.is_empty()
                && !options.names.iter().any(|filter| name.contains(filter)))
        {
            continue;
        }
        let mut record =
            json!({"type": "object", "object": index, "name": name, "visible": visible});
        let started = Instant::now();
        match &object.geometry {
            ThreeDmGeometry::Brep(brep) => {
                record["geometry"] = json!("brep");
                record["support_faces"] = json!(inventory(brep, options.samples));
                record["edges"] = json!(brep.edges().len());
                record["max_edge_controls"] = json!(
                    brep.edges()
                        .iter()
                        .map(|edge| edge.curve().control_points().len())
                        .max()
                        .unwrap_or(0)
                );
                record["max_edge_knots"] = json!(
                    brep.edges()
                        .iter()
                        .map(|edge| edge.curve().knots().len())
                        .max()
                        .unwrap_or(0)
                );
                record["max_edge_spans"] = json!(
                    brep.edges()
                        .iter()
                        .map(|edge| edge.curve().spans().count())
                        .max()
                        .unwrap_or(0)
                );
                if !options.inventory_only {
                    eprintln!("meshing {index}: {name} ({} faces)", brep.faces().len());
                    match brep.display_mesh_with_face_sources(options.samples, model.tolerance) {
                        Ok((mesh, sources)) => {
                            let mut face_triangles = vec![0; brep.faces().len()];
                            for (face, &source) in mesh.faces().iter().zip(&sources) {
                                face_triangles[source] += face.indices().len() - 2;
                            }
                            record["face_triangles"] = json!(face_triangles);
                            record["triangles"] = json!(mesh.triangles().len());
                            record["vertices"] = json!(mesh.vertices().len());
                            total_triangles += mesh.triangles().len() as u64;
                            total_vertices += mesh.vertices().len() as u64;
                        }
                        Err(error) => {
                            record["error"] = json!(error.to_string());
                            failed += 1;
                        }
                    }
                }
            }
            ThreeDmGeometry::Mesh(mesh) => {
                record["geometry"] = json!("mesh");
                record["triangles"] = json!(mesh.triangles().len());
                record["vertices"] = json!(mesh.vertices().len());
                total_triangles += mesh.triangles().len() as u64;
                total_vertices += mesh.vertices().len() as u64;
            }
            _ => continue,
        }
        let seconds = started.elapsed().as_secs_f64();
        record["seconds"] = json!(seconds);
        object_seconds += seconds;
        processed += 1;
        emit(&mut output, record)?;
    }
    emit(
        &mut output,
        json!({"type": "summary", "processed": processed, "failed": failed,
        "triangles": total_triangles, "vertices": total_vertices, "mesh_seconds": object_seconds}),
    )?;
    Ok(())
}
