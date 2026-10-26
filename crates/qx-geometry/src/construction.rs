//! Renderer-independent f64 procedural construction. No source sample is changed.
//! Profiles live in (N,L); hardware uses local (T,N,L). Repeated meshes are shared.
use crate::{
    Result, ensure,
    math::{Frame, Quaternion, Vec3},
    solver::{CenterlineSample, GeneratedTrack},
};
use qx_style::construction::{
    Alignment, HardwareShape, Material, Profile, Role, TrackConstruction,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position_m: Vec3,
    pub normal: Vec3,
    pub uv: [f64; 2],
    pub station_m: f64,
    pub source_station_m: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}
impl Bounds {
    pub fn point(p: Vec3) -> Self {
        Self { min: p, max: p }
    }
    pub fn include(&mut self, p: Vec3) {
        self.min = Vec3::new(
            self.min.x.min(p.x),
            self.min.y.min(p.y),
            self.min.z.min(p.z),
        );
        self.max = Vec3::new(
            self.max.x.max(p.x),
            self.max.y.max(p.y),
            self.max.z.max(p.z),
        );
    }
    pub fn union(&mut self, b: Self) {
        self.include(b.min);
        self.include(b.max);
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub bounds: Bounds,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub station_m: f64,
    pub source_station_m: f64,
    pub position_m: Vec3,
    pub frame: Frame,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub id: String,
    pub role: Role,
    pub material: String,
    pub visible: bool,
    /// Continuous meshes are in world coordinates. Repeated meshes are local;
    /// one shared mesh plus placements avoids duplicating hardware vertices.
    pub mesh: Mesh,
    pub placements: Vec<Placement>,
    pub bounds: Bounds,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TrackMesh {
    pub components: Vec<Component>,
    pub materials: Vec<Material>,
    pub bounds: Bounds,
    pub stats: MeshStats,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshStats {
    pub vertices: usize,
    pub triangles: usize,
    pub rendered_triangles: usize,
    pub instances: usize,
    pub bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct MeshLimits {
    pub max_vertices: usize,
    pub max_indices: usize,
    pub max_instances: usize,
    pub max_bytes: usize,
}
impl Default for MeshLimits {
    fn default() -> Self {
        Self {
            max_vertices: 2_000_000,
            max_indices: 12_000_000,
            max_instances: 100_000,
            max_bytes: 384 * 1024 * 1024,
        }
    }
}
struct Budget {
    limits: MeshLimits,
    vertices: usize,
    indices: usize,
    instances: usize,
    bytes: usize,
}
impl Budget {
    fn reserve(&mut self, v: usize, i: usize, p: usize) -> Result<()> {
        let add = |a: usize, b: usize| {
            a.checked_add(b)
                .ok_or_else(|| crate::GeometryError("mesh count overflow".into()))
        };
        self.vertices = add(self.vertices, v)?;
        self.indices = add(self.indices, i)?;
        self.instances = add(self.instances, p)?;
        let bytes = v
            .checked_mul(std::mem::size_of::<MeshVertex>())
            .and_then(|x| x.checked_add(i.checked_mul(4)?))
            .and_then(|x| x.checked_add(p.checked_mul(std::mem::size_of::<Placement>())?))
            .ok_or_else(|| crate::GeometryError("mesh allocation overflow".into()))?;
        self.bytes = add(self.bytes, bytes)?;
        ensure(
            self.vertices <= self.limits.max_vertices
                && self.vertices <= u32::MAX as usize
                && self.indices <= self.limits.max_indices
                && self.indices <= u32::MAX as usize
                && self.instances <= self.limits.max_instances
                && self.bytes <= self.limits.max_bytes,
            "procedural mesh budget exceeded; reduce sample/profile counts or increase hardware spacing",
        )
    }
}
fn allocated<T>(n: usize) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| crate::GeometryError("procedural mesh allocation failed".into()))?;
    Ok(v)
}
fn v3(a: [f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}
fn validate_samples(track: &GeneratedTrack) -> Result<()> {
    ensure(
        track.samples.len() >= 2,
        "track mesh requires at least two samples",
    )?;
    for s in &track.samples {
        ensure(
            s.position_m.is_finite() && s.distance_m.is_finite(),
            "nonfinite track sample",
        )?;
    }
    ensure(
        track.samples[0].distance_m == 0.,
        "first sample station must be zero",
    )?;
    for pair in track.samples.windows(2) {
        let h = pair[1].distance_m - pair[0].distance_m;
        ensure(
            h >= 1e-9 && (pair[1].position_m - pair[0].position_m).length() > 1e-10,
            "duplicate station or zero/tiny segment",
        )?;
        let angle = pair[0]
            .frame
            .orientation()
            .conjugate()
            .compose(pair[1].frame.orientation())
            .rotation_vector()?
            .length();
        ensure(
            angle <= 0.75,
            "frame discontinuity exceeds 0.75 rad; increase solver sampling",
        )?;
        ensure(
            (pair[1].position_m - pair[0].position_m)
                .dot(pair[0].frame.tangent() + pair[1].frame.tangent())
                > 0.,
            "sample direction opposes frame tangent",
        )?;
    }
    Ok(())
}
/// Ear clipping supports a simple concave loop. Input winding and intersections
/// are validated by qx-style; all cap triangles have +T winding.
fn cap_triangles(p: &[[f64; 2]]) -> Result<Vec<[usize; 3]>> {
    let mut ring: Vec<_> = (0..p.len()).collect();
    let mut out = vec![];
    let cross = |a: usize, b: usize, c: usize| {
        (p[b][0] - p[a][0]) * (p[c][1] - p[a][1]) - (p[b][1] - p[a][1]) * (p[c][0] - p[a][0])
    };
    while ring.len() > 3 {
        let ear = (0..ring.len()).find(|&i| {
            let a = ring[(i + ring.len() - 1) % ring.len()];
            let b = ring[i];
            let c = ring[(i + 1) % ring.len()];
            cross(a, b, c) > 1e-12
                && !ring.iter().any(|&j| {
                    j != a
                        && j != b
                        && j != c
                        && cross(a, b, j) >= -1e-12
                        && cross(b, c, j) >= -1e-12
                        && cross(c, a, j) >= -1e-12
                })
        });
        let i = ear.ok_or_else(|| crate::GeometryError("cannot triangulate profile cap".into()))?;
        out.push([
            ring[(i + ring.len() - 1) % ring.len()],
            ring[i],
            ring[(i + 1) % ring.len()],
        ]);
        ring.remove(i);
    }
    out.push([ring[0], ring[1], ring[2]]);
    Ok(out)
}
fn sweep(
    samples: &[CenterlineSample],
    profile: &Profile,
    offset: Vec3,
    rotation: f64,
    caps: bool,
    budget: &mut Budget,
) -> Result<Mesh> {
    let points = profile
        .points()
        .map_err(|e| crate::GeometryError(e.to_string()))?;
    let n = points.len();
    let rings = samples.len();
    let count = rings
        .checked_mul(n * 2)
        .and_then(|v| v.checked_add(if caps { n * 2 } else { 0 }))
        .ok_or_else(|| crate::GeometryError("vertex count overflow".into()))?;
    let index_count = (rings - 1)
        .checked_mul(n * 6)
        .and_then(|v| v.checked_add(if caps { (n - 2) * 6 } else { 0 }))
        .ok_or_else(|| crate::GeometryError("index count overflow".into()))?;
    budget.reserve(count, index_count, 0)?;
    let mut vertices = allocated(count)?;
    let mut indices = allocated(index_count)?;
    let (sn, cs) = rotation.sin_cos();
    let local: Vec<_> = points
        .iter()
        .map(|p| offset + Vec3::new(0., p[0] * cs - p[1] * sn, p[0] * sn + p[1] * cs))
        .collect();
    let world =
        |i: usize, j: usize| samples[i].position_m + samples[i].frame.local_to_world(local[j]);
    let mut bounds = Bounds::point(world(0, 0));
    let mut perimeter = vec![0.];
    for j in 0..n {
        perimeter.push(perimeter[j] + (local[(j + 1) % n] - local[j]).length());
    }
    for (i, sample) in samples.iter().enumerate() {
        for j in 0..n {
            for side in 0..2 {
                let k = (j + side) % n;
                let pos = world(i, k);
                bounds.include(pos);
                let along = world((i + 1).min(rings - 1), k) - world(i.saturating_sub(1), k);
                ensure(
                    along.dot(sample.frame.tangent()) > 1e-12,
                    "profile/offset folds back along track; reduce dimensions or refine samples",
                )?;
                let edge = sample.frame.local_to_world(local[(j + 1) % n] - local[j]);
                let normal = if matches!(profile, Profile::Circle { .. }) {
                    let radial = sample.frame.local_to_world(local[k] - offset);
                    (radial - along * (radial.dot(along) / along.dot(along))).normalized()?
                } else {
                    edge.cross(along).normalized()?
                };
                vertices.push(MeshVertex {
                    position_m: pos,
                    normal,
                    uv: [perimeter[j + side], sample.distance_m],
                    station_m: sample.distance_m,
                    source_station_m: sample.distance_m,
                });
            }
        }
    }
    for i in 0..rings - 1 {
        for j in 0..n {
            let a = (i * n * 2 + j * 2) as u32;
            let b = a + 1;
            let c = a + (n * 2) as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    if caps {
        let triangles = cap_triangles(&points)?;
        for (i, sign) in [(0, -1.), (rings - 1, 1.)] {
            let base = vertices.len() as u32;
            for (j, point) in points.iter().enumerate() {
                vertices.push(MeshVertex {
                    position_m: world(i, j),
                    normal: samples[i].frame.tangent() * sign,
                    uv: *point,
                    station_m: samples[i].distance_m,
                    source_station_m: samples[i].distance_m,
                });
            }
            for t in &triangles {
                let [a, b, c] = t.map(|j| base + j as u32);
                indices.extend_from_slice(&if sign > 0. { [a, b, c] } else { [c, b, a] });
            }
        }
    }
    let mesh = Mesh {
        vertices,
        indices,
        bounds,
    };
    mesh.validate()?;
    Ok(mesh)
}
impl Mesh {
    /// Validates finite attributes, index bounds, nondegenerate faces and
    /// outward winding against supplied normals. Does not certify collisions.
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.vertices.is_empty() && self.indices.len().is_multiple_of(3),
            "empty mesh or incomplete triangle",
        )?;
        for v in &self.vertices {
            ensure(
                v.position_m.is_finite()
                    && v.normal.is_finite()
                    && (v.normal.length() - 1.).abs() < 1e-8
                    && v.uv.iter().all(|x| x.is_finite())
                    && v.station_m.is_finite(),
                "invalid mesh attribute",
            )?;
        }
        for t in self.indices.as_chunks::<3>().0.iter() {
            ensure(
                t.iter().all(|i| (*i as usize) < self.vertices.len()),
                "mesh index out of range",
            )?;
            let (a, b, c) = (
                self.vertices[t[0] as usize],
                self.vertices[t[1] as usize],
                self.vertices[t[2] as usize],
            );
            let cross = (b.position_m - a.position_m).cross(c.position_m - a.position_m);
            ensure(
                cross.length() > 1e-18 && cross.dot(a.normal + b.normal + c.normal) > 0.,
                "degenerate or folded sweep triangle; reduce profile/offset dimensions or refine solver samples",
            )?;
        }
        Ok(())
    }
}
pub fn build(
    track: &GeneratedTrack,
    definition: &TrackConstruction,
    limits: MeshLimits,
) -> Result<TrackMesh> {
    definition
        .validate()
        .map_err(|e| crate::GeometryError(e.to_string()))?;
    validate_samples(track)?;
    let mut budget = Budget {
        limits,
        vertices: 0,
        indices: 0,
        instances: 0,
        bytes: 0,
    };
    let mut components = vec![];
    let mut bounds = Bounds::point(track.samples[0].position_m);
    for c in &definition.continuous {
        let mesh = sweep(
            &track.samples,
            &c.profile,
            v3(c.offset_m),
            c.rotation_rad,
            c.cap_ends,
            &mut budget,
        )?;
        bounds.union(mesh.bounds);
        components.push(Component {
            id: c.id.clone(),
            role: c.role,
            material: c.material.clone(),
            visible: c.visible,
            bounds: mesh.bounds,
            mesh,
            placements: vec![],
        });
    }
    for c in &definition.repeated {
        let size = match &c.shape {
            HardwareShape::Box { size_m } => *size_m,
            HardwareShape::Asset { id, path, .. } => {
                return Err(crate::GeometryError(format!(
                    "unresolved component asset {id}: {path}; GLB import is not implemented"
                )));
            }
        };
        let end = track.length_m() - c.end_margin_m;
        let count = if end < c.start_m {
            0
        } else {
            let steps = ((end - c.start_m) / c.spacing_m).floor();
            ensure(
                steps.is_finite() && steps < budget.limits.max_instances as f64,
                "repeated hardware instance budget exceeded",
            )?;
            steps as usize + 1
        };
        budget.reserve(0, 0, count)?;
        let frame = Frame::new(Quaternion::IDENTITY)?;
        let template = [
            CenterlineSample {
                distance_m: 0.,
                design_time_s: 0.,
                position_m: Vec3::X * (-size[0] / 2.),
                frame,
            },
            CenterlineSample {
                distance_m: size[0],
                design_time_s: 0.,
                position_m: Vec3::X * (size[0] / 2.),
                frame,
            },
        ];
        let mesh = sweep(
            &template,
            &Profile::Box {
                height_m: size[1],
                width_m: size[2],
            },
            Vec3::ZERO,
            0.,
            true,
            &mut budget,
        )?;
        let rotation = Quaternion::axis_angle(Vec3::Z, c.rotation_rad[2])?
            .compose(Quaternion::axis_angle(Vec3::Y, c.rotation_rad[1])?)
            .compose(Quaternion::axis_angle(Vec3::X, c.rotation_rad[0])?);
        let mut placements = allocated(count)?;
        let mut component_bounds = None::<Bounds>;
        for k in 0..count {
            let station = c.start_m + k as f64 * c.spacing_m;
            let sample = track.sample_at_distance(station)?;
            let base = if c.alignment == Alignment::TrackFrame {
                sample.frame
            } else {
                frame
            };
            let placement = Placement {
                station_m: station,
                source_station_m: station,
                position_m: sample.position_m + base.local_to_world(v3(c.offset_m)),
                frame: Frame::new(base.orientation().compose(rotation))?,
            };
            for vertex in &mesh.vertices {
                let p = placement.position_m + placement.frame.local_to_world(vertex.position_m);
                if let Some(b) = &mut component_bounds {
                    b.include(p);
                } else {
                    component_bounds = Some(Bounds::point(p));
                }
            }
            placements.push(placement);
        }
        let cb = component_bounds.unwrap_or(Bounds::point(track.samples[0].position_m));
        bounds.union(cb);
        components.push(Component {
            id: c.id.clone(),
            role: Role::Hardware,
            material: c.material.clone(),
            visible: c.visible,
            mesh,
            placements,
            bounds: cb,
        });
    }
    let rendered_triangles = components
        .iter()
        .map(|c| {
            c.mesh.indices.len() / 3
                * if c.role == Role::Hardware {
                    c.placements.len()
                } else {
                    1
                }
        })
        .sum();
    Ok(TrackMesh {
        components,
        materials: definition.materials.clone(),
        bounds,
        stats: MeshStats {
            vertices: budget.vertices,
            triangles: budget.indices / 3,
            rendered_triangles,
            instances: budget.instances,
            bytes: budget.bytes,
        },
    })
}

/// Build physical track from the existing constrained rider-to-track inverse.
/// Physical arc station drives placement/UVs, authored station drives selection.
/// One-sided duplicate derivative records share the same pose and are coalesced
/// only in this derived mesh input, never in the author's original data.
pub fn build_reference(
    source: &GeneratedTrack,
    reference: &crate::heartline::ReferenceTrack,
    definition: &TrackConstruction,
    limits: MeshLimits,
) -> Result<TrackMesh> {
    ensure(
        reference.role == crate::heartline::ReferencePathRole::TrackCenterline,
        "mesh reference must be physical track",
    )?;
    let mut track = source.clone();
    track.samples.clear();
    let mut mapping = Vec::<(f64, f64)>::new();
    for p in &reference.points {
        if let Some(last) = track.samples.last()
            && p.path_distance_m == last.distance_m
        {
            ensure(
                (p.position_m - last.position_m).length() < 1e-7,
                "inverse reference has a discontinuous pose at a shared station",
            )?;
            continue;
        }
        track.samples.push(CenterlineSample {
            distance_m: p.path_distance_m,
            design_time_s: 0.,
            position_m: p.position_m,
            frame: p.frame,
        });
        mapping.push((p.path_distance_m, p.source_distance_m));
    }
    let to_source = |s: f64| {
        let i = mapping.partition_point(|p| p.0 < s);
        if i == 0 {
            mapping[0].1
        } else if i >= mapping.len() {
            mapping[mapping.len() - 1].1
        } else {
            let a = mapping[i - 1];
            let b = mapping[i];
            a.1 + (b.1 - a.1) * (s - a.0) / (b.0 - a.0)
        }
    };
    let mut mesh = build(&track, definition, limits)?;
    for c in &mut mesh.components {
        if c.role == Role::Hardware {
            for p in &mut c.placements {
                p.source_station_m = to_source(p.station_m);
            }
        } else {
            for v in &mut c.mesh.vertices {
                v.source_station_m = to_source(v.station_m);
            }
        }
    }
    Ok(mesh)
}
