//! Reproducible public 3DM settings inputs for the native headless oracle.
use viboceros_geometry::SubDMeshDensity;
use viboceros_io::{ThreeDmModel, write_3dm_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: analysis_subd_fixtures OUTPUT_DIRECTORY")?;
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    for (level, adaptive) in [
        (0, true),
        (0, false),
        (2, true),
        (3, false),
        (5, true),
        (6, false),
    ] {
        let path = directory.join(format!(
            "level{level}-{}.3dm",
            if adaptive { "adaptive" } else { "absolute" }
        ));
        if path.exists() {
            continue;
        }
        let mut model = ThreeDmModel::new(vec![], vec![], vec![]);
        model.analysis_subd_density = SubDMeshDensity::new(level, adaptive).unwrap();
        write_3dm_file(&path, &model)?;
    }
    Ok(())
}
