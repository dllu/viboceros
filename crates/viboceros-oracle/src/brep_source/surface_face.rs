//! Exact parameter definitions of the shared `surface_face` input recipe.
use super::*;
use viboceros_geometry::{BrepLoop, BrepTrim};

pub(super) fn unit_trim_domains() -> [[f64; 2]; 4] {
    [[0., 1.]; 4]
}

pub(super) fn with_trim_domains(
    brep: Brep,
    domains: [[f64; 2]; 4],
    tolerance: Tolerance,
) -> Result<Brep, ProbeError> {
    let face = &brep.faces()[0];
    let ring = &face.loops()[0];
    if brep.faces().len() != 1 || face.loops().len() != 1 || ring.trims().len() != 4 {
        return Err(ProbeError::FixtureInvariant(
            "surface_face input requires four natural boundary trims",
        ));
    }
    let trims = ring
        .trims()
        .iter()
        .zip(domains)
        .map(|(trim, [start, end])| {
            let curve = trim.curve();
            // Only the scalar parameter changes. UV coordinates, weights,
            // spatial edges, topology, orientation and tolerances stay intact.
            if curve.degree() != 1 || curve.control_points().len() != 2 {
                return Err(ProbeError::FixtureInvariant(
                    "surface_face input requires linear natural boundary trims",
                ));
            }
            Ok(BrepTrim::try_new(
                trim.vertices(),
                trim.edge(),
                trim.is_reversed_3d(),
                NurbsCurve2::try_new_rational(
                    1,
                    curve.control_points().to_vec(),
                    vec![start, start, end, end],
                )?,
                trim.trim_type(),
                trim.iso(),
                trim.tolerance(),
            )?)
        })
        .collect::<Result<Vec<_>, ProbeError>>()?;
    Ok(Brep::try_new(
        brep.vertices().to_vec(),
        brep.edges().to_vec(),
        vec![BrepFace::try_new(
            face.surface().clone(),
            face.is_reversed(),
            vec![BrepLoop::try_new(ring.loop_type(), trims)?],
        )?],
        tolerance,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_surface_input_domains_are_explicit_and_do_not_change_geometry() {
        // Spatial edges use a local frame: the split is at edge parameter 1,
        // corresponding to surface u=3 and to the default trim parameter 0.5.
        let source = json!({"source":{"type":"surface_face","surface":{
            "degree_u":1,"degree_v":1,"control_point_count_u":2,"control_point_count_v":2,
            "control_points":[{"point":[0,0,0],"weight":1},{"point":[4,0,0],"weight":1},
                {"point":[0,4,0],"weight":1},{"point":[4,4,0],"weight":1}],
            "knots_u":[2,2,4,4],"knots_v":[-3,-3,5,5]
        }},"splits":[[0,[1]]]});
        let build = |value| {
            serde_json::from_value::<BrepSourceFixture>(value)
                .unwrap()
                .build(Tolerance::DEFAULT)
        };
        let unit = build(source.clone()).unwrap();
        let mut explicit = source.clone();
        explicit["source"]["trim_domains"] = json!(unit_trim_domains());
        assert_eq!(unit, build(explicit.clone()).unwrap());
        // Domains are assigned before edge splitting, not to output records.
        assert_eq!(
            unit.faces()[0].loops()[0].trims()[0].curve().domain(),
            0. ..=0.5
        );
        explicit["source"]["trim_domains"] = json!([[2, 4], [-3, 5], [2, 4], [-3, 5]]);
        let native = build(explicit.clone()).unwrap();
        assert_eq!(unit.vertices(), native.vertices());
        assert_eq!(unit.edges(), native.edges());
        assert_eq!(unit.faces()[0].surface(), native.faces()[0].surface());
        assert_eq!(
            native.faces()[0].loops()[0].trims()[0].curve().domain(),
            2. ..=3.
        );
        for domains in [
            json!([[1, 1], [0, 1], [0, 1], [0, 1]]),
            json!([[1, 0], [0, 1], [0, 1], [0, 1]]),
        ] {
            explicit["source"]["trim_domains"] = domains;
            assert!(build(explicit.clone()).is_err());
        }
        assert_eq!(unit, build(source).unwrap());
    }
}
