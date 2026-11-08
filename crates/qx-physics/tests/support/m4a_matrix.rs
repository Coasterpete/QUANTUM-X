//! Small, bounded research matrices only; never a production solver.
pub type Matrix = Vec<Vec<f64>>;

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
pub fn max_abs(a: &[f64]) -> f64 {
    a.iter().fold(0_f64, |m, x| m.max(x.abs()))
}
pub fn multiply(a: &Matrix, x: &[f64]) -> Vec<f64> {
    a.iter().map(|r| dot(r, x)).collect()
}

/// Rows and angular columns use a characteristic length of ONE metre. No
/// threshold here is meaningful across arbitrary mechanism scales.
pub fn nullspace(a: &Matrix, tolerance: f64) -> (usize, Matrix) {
    let n = a[0].len();
    assert!(a.len() <= 256 && n <= 128);
    assert!(
        a.iter()
            .all(|r| r.len() == n && r.iter().all(|x| x.is_finite()))
    );
    let mut r = a.clone();
    let mut pivots = vec![];
    for col in 0..n {
        let row = pivots.len();
        let best = (row..r.len()).max_by(|&i, &j| r[i][col].abs().total_cmp(&r[j][col].abs()));
        let Some(best) = best else { break };
        if r[best][col].abs() <= tolerance {
            continue;
        }
        r.swap(row, best);
        let p = r[row][col];
        for x in &mut r[row] {
            *x /= p;
        }
        let pivot = r[row].clone();
        for (i, values) in r.iter_mut().enumerate() {
            if i != row {
                let p = values[col];
                for (value, reference) in values.iter_mut().zip(&pivot) {
                    *value -= p * reference;
                }
            }
        }
        pivots.push(col);
    }
    let mut basis = vec![];
    for free in (0..n).filter(|i| !pivots.contains(i)) {
        let mut v = vec![0.; n];
        v[free] = 1.;
        for (i, &col) in pivots.iter().enumerate() {
            v[col] = -r[i][free];
        }
        let norm = dot(&v, &v).sqrt();
        for x in &mut v {
            *x /= norm;
        }
        assert!(max_abs(&multiply(a, &v)) < tolerance * 100.);
        basis.push(v);
    }
    (pivots.len(), basis)
}

pub fn finite_jacobian(n: usize, h: f64, f: impl Fn(&[f64]) -> Vec<f64>) -> Matrix {
    let mut a = vec![vec![0.; n]; f(&vec![0.; n]).len()];
    for col in 0..n {
        let mut delta = vec![0.; n];
        delta[col] = h;
        let plus = f(&delta);
        delta[col] = -h;
        let minus = f(&delta);
        for (i, row) in a.iter_mut().enumerate() {
            row[col] = (plus[i] - minus[i]) / (2. * h);
        }
    }
    a
}

/// Exact square solve for research reaction checks; rejects singular systems.
pub fn solve(a: &Matrix, b: &[f64]) -> Vec<f64> {
    let n = a.len();
    assert_eq!(a[0].len(), n);
    let mut a: Matrix = a
        .iter()
        .zip(b)
        .map(|(r, b)| {
            let mut r = r.clone();
            r.push(*b);
            r
        })
        .collect();
    for col in 0..n {
        let best = (col..n)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap();
        a.swap(col, best);
        assert!(
            a[col][col].abs() > 1e-12,
            "singular research reaction matrix"
        );
        let p = a[col][col];
        for value in &mut a[col][col..=n] {
            *value /= p;
        }
        let pivot = a[col].clone();
        for (i, values) in a.iter_mut().enumerate() {
            if i != col {
                let p = values[col];
                for (value, reference) in values[col..=n].iter_mut().zip(&pivot[col..=n]) {
                    *value -= p * reference;
                }
            }
        }
    }
    a.iter().map(|r| r[n]).collect()
}

pub fn emit(name: &str, a: &Matrix, rank: usize) {
    // Rust debug lists of finite floats are also JSON. An independent NumPy
    // SVD checker consumes these lines, without any runtime dependency.
    println!("M4A_MATRIX {{\"name\":\"{name}\",\"rank\":{rank},\"a\":{a:?}}}");
}
