//! Time complete geometry deformation separately from fixtures and validation.
use std::time::Instant;
use viboceros_command::stretch;
#[allow(dead_code)]
#[path = "../tests/support/stretch.rs"]
mod support;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let requested = args.first().map(String::as_str);
    let iterations = args.get(1).map_or(Ok(1), |s| s.parse::<usize>())?;
    if !(1..=20).contains(&iterations) || args.len() > 3 {
        return Err(
            "usage: profile_morph_fits [case-id] [iterations:1..20] [artifact-directory]".into(),
        );
    }
    let cases = support::rows()
        .into_iter()
        .filter(|(op, _)| {
            let id = op["id"].as_str().unwrap();
            requested.map_or_else(
                || {
                    id.starts_with("axis_")
                        && matches!(op["shape"].as_str(), Some("Curve" | "Surface" | "Box"))
                },
                |name| id == name,
            )
        })
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return Err("unknown Stretch fixture".into());
    }
    for (op, value) in cases {
        let (mut doc, sources) = support::setup(&op, &value);
        let (start, end) = support::axis(&op);
        let target = support::targets(&op)[0];
        let options = support::options(&op);
        let mut times = Vec::new();
        let mut result = None;
        for _ in 0..iterations {
            let began = Instant::now();
            let deformed =
                stretch::deformed_geometries(&doc, &sources.ids, start, end, target, options);
            times.push(began.elapsed().as_nanos());
            result = Some(deformed?);
        }
        doc.replace_object_geometries(result.unwrap())?;
        let maximum_native_error = sources.check(&doc, &op, &value, "after");
        if let Some(folder) = args.get(2) {
            std::fs::create_dir_all(folder)?;
            let path =
                std::path::Path::new(folder).join(format!("{}.3dm", op["id"].as_str().unwrap()));
            viboceros_command::CommandRegistry::with_builtins()
                .execute(&mut doc, &format!("Export3dm {}", path.display()))?;
        }
        times.sort_unstable();
        println!(
            "{}",
            serde_json::json!({"id":op["id"],"iterations":iterations,"median_ns":times[times.len()/2],"minimum_ns":times[0],"maximum_native_error":maximum_native_error,"native_smlib":cfg!(feature="native-smlib")})
        );
    }
    Ok(())
}
