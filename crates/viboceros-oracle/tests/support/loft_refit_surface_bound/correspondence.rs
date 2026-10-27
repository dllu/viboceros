//! Propose one ordered V correspondence and certify every full row.
use super::*;

pub(super) fn bound(a: &Value, b: &Value, limit: f64) -> Result<f64, String> {
    let nu = a["control_count"][0].as_u64().unwrap() as usize;
    let rows = (0..nu)
        .map(|u| (curve(a, 1, u, &a["knots_v"]), curve(b, 1, u, &b["knots_v"])))
        .collect::<Vec<_>>();
    // A single monotone V correspondence must certify every control row.
    // Choose a nonconstant row, avoiding singular point-end rows.
    let extent = |c: &NurbsCurve| {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for p in c.control_points() {
            for (i, x) in p.point().to_array().into_iter().enumerate() {
                low[i] = low[i].min(x);
                high[i] = high[i].max(x);
            }
        }
        (0..3).map(|i| high[i] - low[i]).fold(0_f64, f64::max)
    };
    let reference = rows
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| extent(&a.0).total_cmp(&extent(&b.0)))
        .unwrap()
        .0;
    let (source, target) = &rows[reference];
    let sd = source.domain();
    let td = target.domain();
    let sw = *sd.end() - *sd.start();
    let tw = *td.end() - *td.start();
    if !sw.is_finite() || !tw.is_finite() || sw <= 0. || tw <= 0. {
        return Err("unsupported correspondence chart".into());
    }
    let mut cache =
        std::collections::BTreeMap::from([(0_f64.to_bits(), 0.), (1_f64.to_bits(), 1.)]);
    for group in
        source.knots()[source.degree()..=source.control_points().len()].chunk_by(|a, b| a == b)
    {
        let f = (group[0] - *sd.start()) / sw;
        if f > 0. && f < 1. {
            cache.entry(f.to_bits()).or_insert(f64::NAN);
        }
    }
    for group in
        target.knots()[target.degree()..=target.control_points().len()].chunk_by(|a, b| a == b)
    {
        let q = (group[0] - *td.start()) / tw;
        if q > 0. && q < 1. {
            let point = target.evaluate(group[0]).map_err(|e| e.to_string())?;
            let t = source
                .point_getter_parameter_from_seed(
                    point,
                    source.parameter_at(q).map_err(|e| e.to_string())?,
                    viboceros_geometry::Tolerance::DEFAULT,
                )
                .map_err(|e| e.to_string())?;
            let f = (t - *sd.start()) / sw;
            if f > 0. && f < 1. {
                cache.insert(f.to_bits(), q);
            }
        }
    }
    for intervals in [8, 16, 32, 64, 128, 256, 512, 1024] {
        for i in 1..intervals {
            cache
                .entry((i as f64 / intervals as f64).to_bits())
                .or_insert(f64::NAN);
        }
        if cache.len() > 2049 {
            return Err("surface correspondence station limit".into());
        }
        for (&bits, q) in &mut cache {
            if !q.is_finite() {
                let f = f64::from_bits(bits);
                let point = source
                    .evaluate(source.parameter_at(f).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                let t = target
                    .point_getter_parameter_from_seed(
                        point,
                        target.parameter_at(f).map_err(|e| e.to_string())?,
                        viboceros_geometry::Tolerance::DEFAULT,
                    )
                    .map_err(|e| e.to_string())?;
                *q = (t - *td.start()) / tw;
            }
        }
        let mut pairs: Vec<[f64; 2]> = Vec::new();
        for (&bits, &q) in &cache {
            let f = f64::from_bits(bits);
            // Closest queries only propose stations. Drop conflicting interior
            // proposals; every remaining full interval still needs its proof.
            if f > 0. && f < 1. && (q <= 0. || q >= 1. || pairs.last().is_some_and(|p| p[1] >= q)) {
                continue;
            }
            pairs.push([f, q]);
        }
        if pairs.windows(2).any(|p| p[0][1] >= p[1][1]) {
            return Err("surface correspondence is not ordered".into());
        }
        if let Some(bounds) = rows
            .iter()
            .map(|(a, b)| {
                a.mapped_curve_deviation_bound(b, &pairs, limit)
                    .ok()
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()
        {
            return Ok(bounds.into_iter().fold(0_f64, f64::max));
        }
    }
    Err("complete shared surface correspondence unresolved".into())
}
