//! Actual PlanarSrf output regions and source-retention observations.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    curves: Vec<curve_native::CutSource>,
    #[serde(default)]
    delete_input: bool,
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let sources = f
        .curves
        .iter()
        .map(|c| c.geometry(tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let run = || -> Result<Value, ProbeError> {
        let mut document = Document::new(tolerance);
        let ids = sources
            .iter()
            .map(|g| document.add_geometry(g.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        document.select_objects_direct(
            ids.iter().copied(),
            viboceros_document::SelectionMode::Replace,
        )?;
        viboceros_command::CommandRegistry::with_builtins().execute(
            &mut document,
            if f.delete_input {
                "PlanarSrf DeleteInput=Yes"
            } else {
                "PlanarSrf DeleteInput=No"
            },
        )?;
        let regions=document.objects().filter(|o|!ids.contains(&o.id())).map(|o| {
            let Geometry::Brep(b)=o.geometry() else {return Err(ProbeError::FixtureInvariant("planar B-rep"));};
            #[cfg(feature="native-smlib")]
            let bounds=viboceros_smlib::Shape::from_brep(b,tolerance).and_then(|s|s.tight_bounds()).map_err(CommandError::from)?;
            #[cfg(not(feature="native-smlib"))]
            let bounds=b.bounds();
            Ok(json!({"faces":b.faces().len(),"loops":b.faces().iter().map(|f|f.loops().len()).collect::<Vec<_>>(),
                "area":b.area(tolerance)?,"min":bounds.min().to_array(),"max":bounds.max().to_array()}))
        }).collect::<Result<Vec<_>,ProbeError>>()?;
        Ok(
            json!({"regions":regions,"retained_sources":ids.iter().filter(|&&id|document.object(id).is_some()).count()}),
        )
    };
    let mut value = run()?;
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        value = std::hint::black_box(run()?);
    }
    Ok((
        value,
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}
