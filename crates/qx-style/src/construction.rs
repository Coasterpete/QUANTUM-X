//! Versioned visual construction data, independent of the solver and renderer.
//! Local axes: X=T forward, Y=N rider up, Z=L lateral; SI metres/radians.
use crate::{Result, identifier, relative_asset, require};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    DualRail,
    SingleRail,
    Hybrid,
    Wooden,
    Custom,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Running,
    Structure,
    Hardware,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profile {
    Circle {
        radius_m: f64,
        sides: u32,
    },
    Box {
        height_m: f64,
        width_m: f64,
    },
    /// One simple closed CCW loop in (N,L). Do not repeat the first vertex.
    Polygon {
        points: Vec<[f64; 2]>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub id: String,
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sweep {
    pub id: String,
    pub role: Role,
    pub profile: Profile,
    /// Translation in local T/N/L, applied once to the authoritative frame.
    pub offset_m: [f64; 3],
    pub rotation_rad: f64,
    pub material: String,
    pub visible: bool,
    pub cap_ends: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    TrackFrame,
    WorldAxes,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HardwareShape {
    Box {
        size_m: [f64; 3],
    },
    /// Reserved asset contract. Generation returns an explicit unresolved-asset
    /// error until an importer is provided; never silently substitutes a box.
    Asset {
        id: String,
        path: String,
        version: u32,
        scale: [f64; 3],
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repeater {
    pub id: String,
    pub shape: HardwareShape,
    pub spacing_m: f64,
    pub start_m: f64,
    pub end_margin_m: f64,
    pub offset_m: [f64; 3],
    /// Applied X then Y then Z rotation, in radians.
    pub rotation_rad: [f64; 3],
    pub alignment: Alignment,
    pub material: String,
    pub visible: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackConstruction {
    pub version: u32,
    pub id: String,
    pub strategy: Strategy,
    pub materials: Vec<Material>,
    pub continuous: Vec<Sweep>,
    pub repeated: Vec<Repeater>,
}
fn dimension(v: f64) -> bool {
    v.is_finite() && (1e-5..=1000.).contains(&v)
}
fn bounded(v: &[f64], limit: f64) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() <= limit)
}
impl Profile {
    pub fn points(&self) -> Result<Vec<[f64; 2]>> {
        let points = match self {
            Self::Circle { radius_m, sides } => {
                require(
                    dimension(*radius_m) && (3..=128).contains(sides),
                    "invalid circle radius or side count",
                )?;
                (0..*sides)
                    .map(|i| {
                        let a = i as f64 * std::f64::consts::TAU / *sides as f64;
                        [radius_m * a.cos(), radius_m * a.sin()]
                    })
                    .collect()
            }
            Self::Box { height_m, width_m } => {
                require(
                    dimension(*height_m) && dimension(*width_m),
                    "invalid box profile dimensions",
                )?;
                let (h, w) = (height_m / 2., width_m / 2.);
                vec![[-h, -w], [h, -w], [h, w], [-h, w]]
            }
            Self::Polygon { points } => {
                require(
                    (3..=128).contains(&points.len()),
                    "polygon must have 3..128 vertices",
                )?;
                points.clone()
            }
        };
        require(
            points.iter().all(|p| bounded(p, 1000.)),
            "nonfinite or oversized profile",
        )?;
        let n = points.len();
        let cross = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        };
        let area: f64 = (0..n)
            .map(|i| {
                let a = points[i];
                let b = points[(i + 1) % n];
                a[0] * b[1] - b[0] * a[1]
            })
            .sum();
        require(
            area > 1e-10,
            "profile must have positive area and CCW winding",
        )?;
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            require(
                (b[0] - a[0]).hypot(b[1] - a[1]) >= 1e-5,
                "duplicate or tiny profile edge",
            )?;
            require(
                cross(points[(i + n - 1) % n], a, b).abs() > 1e-12,
                "collinear profile vertices",
            )?;
            for j in i + 1..n {
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                let (c, d) = (points[j], points[(j + 1) % n]);
                let intersects = cross(a, b, c) * cross(a, b, d) <= 0.
                    && cross(c, d, a) * cross(c, d, b) <= 0.
                    && a[0].min(b[0]) <= c[0].max(d[0])
                    && c[0].min(d[0]) <= a[0].max(b[0])
                    && a[1].min(b[1]) <= c[1].max(d[1])
                    && c[1].min(d[1]) <= a[1].max(b[1]);
                require(!intersects, "self-intersecting profile")?;
            }
        }
        Ok(points)
    }
}
impl TrackConstruction {
    pub fn validate(&self) -> Result<()> {
        require(
            self.version == 1 && identifier(&self.id),
            "unsupported construction version or invalid ID",
        )?;
        require(
            (1..=64).contains(&self.materials.len())
                && (1..=64).contains(&self.continuous.len())
                && self.repeated.len() <= 64,
            "invalid construction counts",
        )?;
        let mut mats = HashSet::new();
        for m in &self.materials {
            require(
                identifier(&m.id) && mats.insert(&m.id),
                "invalid or duplicate material ID",
            )?;
            require(
                m.base_color
                    .iter()
                    .chain([&m.metallic, &m.roughness])
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "invalid material properties",
            )?;
        }
        let mut ids = HashSet::new();
        for c in &self.continuous {
            require(
                identifier(&c.id) && ids.insert(&c.id) && mats.contains(&c.material),
                "invalid/duplicate component ID or missing material",
            )?;
            require(
                bounded(&c.offset_m, 1000.) && c.rotation_rad.is_finite(),
                "invalid sweep transform",
            )?;
            c.profile.points()?;
        }
        let count = self
            .continuous
            .iter()
            .filter(|c| c.role == Role::Running)
            .count();
        require(
            match self.strategy {
                Strategy::SingleRail => count == 1,
                Strategy::DualRail => count == 2,
                _ => count > 0,
            },
            "construction strategy has incorrect primary running component count",
        )?;
        for c in &self.repeated {
            require(
                identifier(&c.id) && ids.insert(&c.id) && mats.contains(&c.material),
                "invalid/duplicate repeater ID or missing material",
            )?;
            require(
                dimension(c.spacing_m)
                    && c.start_m.is_finite()
                    && c.start_m >= 0.
                    && c.end_margin_m.is_finite()
                    && c.end_margin_m >= 0.,
                "invalid hardware spacing or endpoint margins",
            )?;
            require(
                bounded(&c.offset_m, 1000.) && bounded(&c.rotation_rad, 1e6),
                "invalid hardware transform",
            )?;
            match &c.shape {
                HardwareShape::Box { size_m } => require(
                    size_m.iter().all(|v| dimension(*v)),
                    "invalid hardware dimensions",
                )?,
                HardwareShape::Asset {
                    id,
                    path,
                    version,
                    scale,
                } => require(
                    identifier(id)
                        && relative_asset(path)
                        && *version > 0
                        && scale.iter().all(|v| dimension(*v)),
                    "invalid component asset reference",
                )?,
            }
        }
        Ok(())
    }
    /// Engine validation data only: all dimensions are provisional, original,
    /// and unrelated to the conceptual production catalog entries.
    pub fn generic_dual() -> Self {
        let mut c = Self {
            version: 1,
            id: "validation-dual".into(),
            strategy: Strategy::DualRail,
            materials: vec![
                material("running", [0.62, 0.72, 0.8], 0.65, 0.28),
                material("structure", [0.19, 0.34, 0.43], 0.5, 0.55),
                material("hardware", [0.42, 0.5, 0.55], 0.45, 0.5),
            ],
            continuous: vec![],
            repeated: vec![],
        };
        for (id, lateral) in [("rail-left", -0.55), ("rail-right", 0.55)] {
            c.continuous.push(Sweep {
                id: id.into(),
                role: Role::Running,
                profile: Profile::Circle {
                    radius_m: 0.065,
                    sides: 12,
                },
                offset_m: [0., 0., lateral],
                rotation_rad: 0.,
                material: "running".into(),
                visible: true,
                cap_ends: true,
            });
        }
        c.continuous.push(Sweep {
            id: "spine".into(),
            role: Role::Structure,
            profile: Profile::Box {
                height_m: 0.28,
                width_m: 0.24,
            },
            offset_m: [0., -0.38, 0.],
            rotation_rad: 0.,
            material: "structure".into(),
            visible: true,
            cap_ends: true,
        });
        c.repeated.push(Repeater {
            id: "cross-tie".into(),
            shape: HardwareShape::Box {
                size_m: [0.12, 0.22, 1.22],
            },
            spacing_m: 1.2,
            start_m: 0.25,
            end_margin_m: 0.25,
            offset_m: [0., -0.16, 0.],
            rotation_rad: [0.; 3],
            alignment: Alignment::TrackFrame,
            material: "hardware".into(),
            visible: true,
        });
        c
    }
    pub fn generic_single() -> Self {
        let mut c = Self::generic_dual();
        c.id = "validation-single".into();
        c.strategy = Strategy::SingleRail;
        c.continuous = vec![Sweep {
            id: "primary-beam".into(),
            role: Role::Running,
            profile: Profile::Polygon {
                points: vec![
                    [-0.45, -0.13],
                    [0.1, -0.13],
                    [0.1, -0.25],
                    [0.22, -0.25],
                    [0.22, 0.25],
                    [0.1, 0.25],
                    [0.1, 0.13],
                    [-0.45, 0.13],
                ],
            },
            offset_m: [0.; 3],
            rotation_rad: 0.,
            material: "running".into(),
            visible: true,
            cap_ends: true,
        }];
        c.repeated[0].id = "beam-bracket".into();
        c.repeated[0].shape = HardwareShape::Box {
            size_m: [0.16, 0.1, 0.65],
        };
        c.repeated[0].offset_m = [0., -0.5, 0.];
        c.repeated[0].spacing_m = 1.8;
        c
    }
}
fn material(id: &str, base_color: [f32; 3], metallic: f32, roughness: f32) -> Material {
    Material {
        id: id.into(),
        base_color,
        metallic,
        roughness,
    }
}
impl Default for TrackConstruction {
    fn default() -> Self {
        Self::generic_dual()
    }
}
