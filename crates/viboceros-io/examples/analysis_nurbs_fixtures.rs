//! Public native 3DM analysis-density inputs for the isolated Rhino oracle.
use viboceros_geometry::NurbsMeshDensity;
use viboceros_io::{ThreeDmModel, write_3dm_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: analysis_nurbs_fixtures OUTPUT_DIRECTORY")?;
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    for (name, density) in [
        ("density0", 0.),
        ("density25", 0.25),
        ("density50", 0.5),
        ("density80", 0.8),
        ("density100", 1.),
    ] {
        let path = directory.join(format!("{name}.3dm"));
        if path.exists() {
            continue;
        }
        let mut model = ThreeDmModel::new(vec![], vec![], vec![]);
        model.analysis_nurbs_density = NurbsMeshDensity::new(density).unwrap();
        write_3dm_file(path, &model)?;
    }
    Ok(())
}
