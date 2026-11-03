use serde_json::Value;
use std::{collections::BTreeMap, io::Read, sync::OnceLock};
use viboceros_geometry::{Point3, SubD};

pub fn captures() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let bytes =
            include_bytes!("../../../../tools/rhino_oracle/observations/subd_to_nurbs_sdk.json.gz");
        let mut json = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_end(&mut json)
            .unwrap();
        serde_json::from_slice(&json).unwrap()
    })
}

pub fn point(value: &Value) -> Point3 {
    Point3::try_from(std::array::from_fn(|i| value[i].as_f64().unwrap())).unwrap()
}

/// Reconstruct the independently captured public control topology and tags.
pub fn source(before: &Value) -> SubD {
    let points = before["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect();
    let faces = before["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect()
        })
        .collect();
    let mut subd = SubD::try_from_polygons(points, faces).unwrap();
    let pair = |a: u32, b: u32| if a < b { [a, b] } else { [b, a] };
    let edges: BTreeMap<_, _> = subd
        .edges()
        .iter()
        .enumerate()
        .map(|(i, &[a, b])| (pair(a, b), i))
        .collect();
    let crease_tags = before["edges"]
        .as_array()
        .unwrap()
        .iter()
        .zip(before["edge_tags"].as_array().unwrap())
        .filter(|(_, tag)| tag.as_u64().unwrap() == 2)
        .map(|(e, _)| {
            let key = pair(e[0].as_u64().unwrap() as u32, e[1].as_u64().unwrap() as u32);
            (edges[&key], 2)
        })
        .collect::<Vec<_>>();
    subd = subd.try_with_edge_tags(&crease_tags).unwrap();
    let vertex_tags = before["vertex_tags"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, t)| (i, t.as_u64().unwrap() as u8))
        .collect::<Vec<_>>();
    subd = subd.try_with_vertex_tags(&vertex_tags).unwrap();
    for (&actual, native) in subd
        .surface_points()
        .iter()
        .zip(before["surface_points"].as_array().unwrap())
    {
        assert!(actual.distance_to(point(native)).unwrap() < 1e-12);
    }
    subd
}

pub fn case(scene: &str) -> &'static Value {
    let id = format!("{scene}_False_3");
    &captures()["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"].as_str() == Some(&id))
        .unwrap()["value"]
}
