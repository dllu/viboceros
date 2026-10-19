//! Rewrites an independently captured file through the public 3dm model API.
use viboceros_geometry::Tolerance;
use viboceros_io::{read_3dm_file, write_3dm_file};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: roundtrip_3dm INPUT.3dm OUTPUT.3dm".into());
    }
    let model = read_3dm_file(&args[0], Tolerance::DEFAULT)?;
    if model.unsupported_object_count() != 0 {
        return Err("input contains unsupported objects".into());
    }
    write_3dm_file(&args[1], &model)?;
    Ok(())
}
