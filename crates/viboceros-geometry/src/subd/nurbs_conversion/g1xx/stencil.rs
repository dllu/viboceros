//! Linear work per fan: first Fourier harmonic and a cyclic compatibility solve.
pub(super) fn first_harmonic(vectors: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let n = vectors.len();
    let angle = std::f64::consts::TAU / n as f64;
    let phases = (0..n)
        .map(|i| (i as f64 * angle).sin_cos())
        .collect::<Vec<_>>();
    let mut sums = [[0.; 3]; 2];
    for (v, &(s, c)) in vectors.iter().zip(&phases) {
        for a in 0..3 {
            sums[0][a] += c * v[a];
            sums[1][a] += s * v[a];
        }
    }
    phases
        .iter()
        .map(|&(s, c)| std::array::from_fn(|a| 2. / n as f64 * (c * sums[0][a] + s * sums[1][a])))
        .collect()
}

pub(super) fn controls(p: [f64; 3], old: &[[[f64; 3]; 3]]) -> Vec<[[f64; 3]; 3]> {
    let n = old.len();
    let angle = std::f64::consts::TAU / n as f64;
    let cosine = angle.cos();
    let vectors = old
        .iter()
        .map(|data| std::array::from_fn(|a| data[0][a] - p[a]))
        .collect::<Vec<_>>();
    let tangents = first_harmonic(&vectors);
    let mut result = Vec::with_capacity(n);
    let mut rhs = Vec::with_capacity(n);
    for (i, &e1) in tangents.iter().enumerate() {
        let e3 = std::array::from_fn::<_, 3, _>(|a| old[i][1][a] - p[a]);
        result.push([
            std::array::from_fn(|a| p[a] + e1[a]),
            std::array::from_fn(|a| p[a] + (6. * e1[a] + e3[a]) / 5.),
            old[i][2],
        ]);
        let prev = (i + n - 1) % n;
        rhs.push(std::array::from_fn::<_, 3, _>(|a| {
            2. * e1[a] + cosine / 15. * (4. * e3[a] - 6. * e1[a])
                - (old[i][2][a] - p[a])
                - (old[prev][2][a] - p[a])
        }));
    }
    let delta = solve(&rhs);
    for (i, data) in result.iter_mut().enumerate() {
        for (a, &change) in delta[i].iter().enumerate() {
            data[2][a] += change;
        }
    }
    result
}

// Solve x_i + x_(i-1) = rhs_i in linear work. For even fans,
// select the solution whose alternating component is zero.
pub(super) fn solve(rhs: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let n = rhs.len();
    let mut values = vec![[0.; 3]; n];
    for i in 1..n {
        values[i] = std::array::from_fn(|a| rhs[i][a] - values[i - 1][a]);
    }
    let initial = if n.is_multiple_of(2) {
        let mut average = [0.; 3];
        for (i, v) in values.iter().enumerate() {
            let sign = if i.is_multiple_of(2) { 1. } else { -1. };
            for a in 0..3 {
                average[a] -= sign * v[a] / n as f64;
            }
        }
        average
    } else {
        std::array::from_fn(|a| (rhs[0][a] - values[n - 1][a]) / 2.)
    };
    for (i, v) in values.iter_mut().enumerate() {
        let sign = if i.is_multiple_of(2) { 1. } else { -1. };
        for (a, &first) in initial.iter().enumerate() {
            v[a] += sign * first;
        }
    }
    values
}
