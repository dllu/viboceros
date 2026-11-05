use serde_json::Value;
pub fn read(base: &std::path::Path, family: &str) -> Value {
    use std::io::Read;
    let path = base.join(format!("observations/{family}.json"));
    let raw = if path.exists() {
        std::fs::read(path).unwrap()
    } else {
        let bytes = std::fs::read(path.with_extension("json.gz")).unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut raw)
            .unwrap();
        raw
    };
    serde_json::from_slice(&raw).unwrap()
}
