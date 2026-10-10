//! Generate a native curved Boolean result and export its Rust tessellation as OBJ.
use std::{
    fs::File,
    io::{BufWriter, Write},
};
use viboceros_geometry::{Point3, Tolerance};
use viboceros_smlib::{BooleanOperation, Solid, Tessellation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/viboceros-smlib-plate.obj".into());
    let mut plate = Solid::box_solid(Point3::try_new(0., 0., 0.)?, [12., 12., 1.5])?;
    for x in 0..4 {
        for y in 0..4 {
            let cutter = Solid::cylinder(
                Point3::try_new(1.5 + 3. * x as f64, 1.5 + 3. * y as f64, -1.)?,
                0.75,
                3.5,
            )?;
            plate = plate.boolean(&cutter, BooleanOperation::Difference)?;
        }
    }
    let properties = plate.properties(1e-8)?;
    let expected = 216. - 16. * std::f64::consts::PI * 0.75_f64.powi(2) * 1.5;
    if !properties.manifold || (properties.volume - expected).abs() > expected * 1e-6 {
        return Err("Native plate failed analytic qualification".into());
    }
    let mesh = plate.tessellate(Tessellation::default(), Tolerance::DEFAULT)?;
    if !mesh.topology().is_closed() || mesh.topology().orientation_conflict_edge_count() != 0 {
        return Err("Rust mesh failed closed-manifold qualification".into());
    }
    let mut file = BufWriter::new(File::create(&path)?);
    writeln!(file, "# Viboceros native SMLib perforated plate")?;
    for p in mesh.vertices() {
        let [x, y, z] = p.to_array();
        writeln!(file, "v {x:.17} {y:.17} {z:.17}")?;
    }
    for [a, b, c] in mesh.triangles() {
        writeln!(file, "f {} {} {}", a + 1, b + 1, c + 1)?;
    }
    file.flush()?;
    println!(
        "Wrote {path}: {} vertices, {} triangles, solid volume {:.12}, expected {:.12}",
        mesh.vertices().len(),
        mesh.triangles().len(),
        properties.volume,
        expected
    );
    Ok(())
}
