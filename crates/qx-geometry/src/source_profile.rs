//! Conservative conversion of a prismatic source mesh to its exact section.
//! Rejects variable sections, holes, end details and disconnected geometry.
use crate::{Result, construction::Mesh, ensure};
use qx_style::construction::Profile;

const EPS: f64 = 2e-6;
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn on_edge(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> bool {
    let length = distance(a, b);
    (distance(a, p) + distance(p, b) - length).abs() < EPS
        && ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])).abs() < EPS * length
}

/// T is the extrusion axis. Returned (N,L) points retain the source origin,
/// bevels and dimensions. Triangulation-only collinear points are coalesced.
pub fn extract(mesh: &Mesh) -> Result<Profile> {
    ensure(
        mesh.vertices.len() <= 16384 && mesh.indices.len() <= 65536,
        "source section extraction budget exceeded",
    )?;
    mesh.validate()?;
    let (lo, hi) = (mesh.bounds.min.x, mesh.bounds.max.x);
    ensure(hi - lo > 1e-4, "source has no extrusion length")?;
    let mid = (lo + hi) * 0.5;
    let mut points: Vec<[f64; 2]> = vec![];
    let mut edges = vec![];
    for tri in mesh.indices.as_chunks::<3>().0 {
        let v = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices[i as usize].position_m);
        let mut hits = vec![];
        for k in 0..3 {
            let (a, b) = (v[k], v[(k + 1) % 3]);
            if (a.x <= mid && b.x > mid) || (b.x <= mid && a.x > mid) {
                let p = a + (b - a) * ((mid - a.x) / (b.x - a.x));
                let p = [p.y, p.z];
                if !hits.iter().any(|q| distance(p, *q) < EPS) {
                    hits.push(p);
                }
            }
        }
        if hits.len() != 2 {
            continue;
        }
        let mut ids = vec![];
        for p in hits {
            let i = points
                .iter()
                .position(|q| distance(p, *q) < EPS)
                .unwrap_or_else(|| {
                    points.push(p);
                    points.len() - 1
                });
            ids.push(i);
        }
        ensure(points.len() <= 512, "source section is too complex")?;
        if ids[0] != ids[1] {
            edges.push([ids[0], ids[1]]);
        }
    }
    ensure(points.len() >= 3, "source has no closed cross section")?;
    let mut neighbors = vec![vec![]; points.len()];
    for [a, b] in edges {
        neighbors[a].push(b);
        neighbors[b].push(a);
    }
    ensure(
        neighbors.iter().all(|n| n.len() == 2),
        "source section is open or branched",
    )?;
    let mut order = vec![0];
    let (mut previous, mut current) = (0, neighbors[0][0]);
    while current != 0 {
        ensure(
            !order.contains(&current),
            "source section has intersecting loops",
        )?;
        order.push(current);
        let next = *neighbors[current].iter().find(|&&n| n != previous).unwrap();
        previous = current;
        current = next;
    }
    ensure(
        order.len() == points.len(),
        "multiple source section loops are unsupported",
    )?;
    let mut ring: Vec<_> = order.into_iter().map(|i| points[i]).collect();
    loop {
        let n = ring.len();
        let remove = (0..n).find(|&i| on_edge(ring[i], ring[(i + n - 1) % n], ring[(i + 1) % n]));
        if let Some(i) = remove {
            ring.remove(i);
        } else {
            break;
        }
        ensure(ring.len() >= 3, "degenerate source section")?;
    }
    let area = |ring: &[[f64; 2]]| {
        (0..ring.len())
            .map(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>()
            * 0.5
    };
    if area(&ring) < 0. {
        ring.reverse();
    }
    let profile = Profile::Polygon {
        points: ring.clone(),
    };
    profile
        .validate()
        .map_err(|e| crate::GeometryError(e.to_string()))?;
    let mut cap_area = [0.; 2];
    let mut side_area = vec![0.; ring.len()];
    for tri in mesh.indices.as_chunks::<3>().0 {
        let v: Vec<_> = tri
            .iter()
            .map(|&i| mesh.vertices[i as usize].position_m)
            .collect();
        let a = (v[1] - v[0]).cross(v[2] - v[0]).length() * 0.5;
        if let Some(k) = [lo, hi]
            .iter()
            .position(|end| v.iter().all(|p| (p.x - end).abs() < EPS))
        {
            cap_area[k] += a;
        } else {
            let k = (0..ring.len()).find(|&k| {
                v.iter()
                    .all(|p| on_edge([p.y, p.z], ring[k], ring[(k + 1) % ring.len()]))
            });
            ensure(
                k.is_some(),
                "source is not prismatic; preserve it as discrete hardware",
            )?;
            side_area[k.unwrap()] += a;
        }
    }
    let close = |a: f64, b: f64| (a - b).abs() < EPS * (1. + b);
    ensure(
        cap_area.iter().all(|a| close(*a, area(&ring))),
        "source end caps do not cover the section",
    )?;
    ensure(
        side_area.iter().enumerate().all(|(i, a)| {
            close(
                *a,
                distance(ring[i], ring[(i + 1) % ring.len()]) * (hi - lo),
            )
        }),
        "source sides do not form a complete extrusion",
    )?;
    Ok(profile)
}
