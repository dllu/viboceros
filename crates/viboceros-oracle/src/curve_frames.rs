use super::curve_join_close::CurveInput;
use super::*;
use viboceros_geometry::FrameTransportOptions;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_frame_fixtures_execute_and_keep_translation_invariants() {
        let mut values = std::collections::BTreeMap::new();
        for (source, count) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/curve_frames.json"),
                21,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/curve_frames_multispan.json"),
                1,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/curve_frames_diagnostics.json"),
                7,
            ),
            (
                include_str!(
                    "../../../tools/rhino_oracle/fixtures/curve_array_corner_diagnostics.json"
                ),
                1,
            ),
        ] {
            let request = serde_json::from_str(source).unwrap();
            let response = run_request(&request).unwrap();
            assert_eq!(response.results.len(), count);
            for result in response.results {
                assert!(values.insert(result.id, result.value).is_none());
            }
        }
        let base = values["frames-spatial"]["samples"].as_array().unwrap();
        let translated = values["frames-spatial-translated"]["samples"]
            .as_array()
            .unwrap();
        assert_eq!(base.len(), translated.len());
        for (a, b) in base.iter().zip(translated) {
            assert_eq!(a["rotation"], b["rotation"]);
            assert_eq!(a["tangent"], b["tangent"]);
        }
        assert_eq!(
            values["frames-spatial-single"]["samples"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            values["frames-stationary-endpoint"]["available"],
            json!(true)
        );
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurveFramesFixture {
    pub curve: CurveInput,
    pub parameters: Vec<f64>,
    #[serde(default)]
    pub domain: Option<[f64; 2]>,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub translation: Option<[f64; 3]>,
}

pub(super) fn run(
    fixture: &CurveFramesFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let mut curve = fixture.curve.geometry()?;
    if let Some([a, b]) = fixture.domain {
        curve = curve.try_reparameterized(a..=b)?;
    }
    if fixture.reversed {
        curve = curve.reversed(tolerance)?;
    }
    if let Some(offset) = fixture.translation {
        let geometry = Geometry::from(curve).transformed(
            AffineTransform3::from_translation(Vector3::try_from(offset)?),
            tolerance,
        )?;
        curve = geometry
            .curve_ref()
            .ok_or(ProbeError::FixtureInvariant(
                "curve frame source is not a curve",
            ))?
            .to_owned();
    }
    let view = curve.as_ref();
    let compute = || {
        view.rotation_minimizing_frames(
            &fixture.parameters,
            None,
            FrameTransportOptions {
                angular_tolerance: tolerance.angular(),
                side: viboceros_geometry::ParameterSide::Left,
                ..Default::default()
            },
        )
    };
    let mut frames = compute()?;
    let start = Instant::now();
    for _ in 0..iterations {
        frames = black_box(compute()?);
    }
    let elapsed =
        u64::try_from(start.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    let seed = frames[0].axes().map(|a| a.as_vector().to_array());
    let samples = frames
        .into_iter()
        .zip(&fixture.parameters)
        .map(|(frame, parameter)| {
            let axes = frame.axes().map(|axis| axis.as_vector().to_array());
            let rotation: [[f64; 3]; 3] = std::array::from_fn(|i| {
                std::array::from_fn(|j| (0..3).map(|k| axes[k][i] * seed[k][j]).sum())
            });
            json!({
                "parameter": parameter,
                "point": frame.origin().to_array(),
                "tangent": frame.z_axis().as_vector().to_array(),
                "rotation": rotation,
            })
        })
        .collect::<Vec<_>>();
    Ok((
        json!({
            "domain": [*view.domain().start(), *view.domain().end()],
            "available": true,
            "samples": samples,
        }),
        elapsed,
    ))
}
