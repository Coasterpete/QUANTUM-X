//! Editable support construction; these solids carry no structural certification.
use crate::{Result, authoring::SectionId, ensure, math::Vec3, solver::GeneratedTrack};
use qx_style::construction::{Material, Profile};
use serde::{Deserialize, Serialize};

/// Adapt the already-computed rider-to-physical inverse for support attachments.
/// Section-relative fractions address physical arc metres within each section.
/// This derived view never replaces the authored curve or engineering snapshot.
pub fn physical_attachment_track(
    source: &GeneratedTrack,
    reference: &crate::heartline::ReferenceTrack,
) -> Result<GeneratedTrack> {
    ensure(
        reference.points.len() >= 2 && source.samples.len() >= 2,
        "physical support reference has insufficient samples",
    )?;
    ensure(
        reference.role == crate::heartline::ReferencePathRole::TrackCenterline,
        "support reference must be physical track",
    )?;
    let mut track = source.clone();
    track.samples.clear();
    track.graphs.clear();
    track.smooth_ranges.clear();
    for p in &reference.points {
        if track
            .samples
            .last()
            .is_some_and(|s| s.distance_m == p.path_distance_m)
        {
            continue;
        }
        track.samples.push(crate::solver::CenterlineSample {
            distance_m: p.path_distance_m,
            design_time_s: 0.,
            position_m: p.position_m,
            frame: p.frame,
        });
    }
    let station = |source_m: f64| -> Result<f64> {
        reference
            .points
            .iter()
            .find(|p| (p.source_distance_m - source_m).abs() < 1e-7)
            .map(|p| p.path_distance_m)
            .ok_or_else(|| {
                crate::GeometryError("physical reference is missing a region boundary".into())
            })
    };
    for section in &mut track.sections {
        section.start_distance_m = station(section.start_distance_m)?;
        section.end_distance_m = station(section.end_distance_m)?;
        section.start_sample = track
            .samples
            .partition_point(|p| p.distance_m < section.start_distance_m);
        section.end_sample = track
            .samples
            .partition_point(|p| p.distance_m < section.end_distance_m)
            .min(track.samples.len() - 1);
    }
    Ok(track)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub section_id: SectionId,
    pub fraction: f64,
    pub offset_m: Vec3,
}
impl Attachment {
    pub fn at_station(track: &GeneratedTrack, station: f64, offset_m: Vec3) -> Result<Self> {
        ensure(
            station.is_finite() && station >= 0. && station <= track.length_m(),
            "support station outside track",
        )?;
        let r = track
            .sections
            .iter()
            .find(|r| station <= r.end_distance_m)
            .ok_or_else(|| crate::GeometryError("support track region unavailable".into()))?;
        Ok(Self {
            section_id: r.id,
            fraction: (station - r.start_distance_m) / (r.end_distance_m - r.start_distance_m),
            offset_m,
        })
    }
    pub fn station(&self, track: &GeneratedTrack) -> Result<f64> {
        let r = track
            .sections
            .iter()
            .find(|r| r.id == self.section_id)
            .ok_or_else(|| {
                crate::GeometryError(format!(
                    "support attachment region {} no longer exists",
                    self.section_id.0
                ))
            })?;
        Ok(r.start_distance_m + self.fraction * (r.end_distance_m - r.start_distance_m))
    }
    pub fn resolve(&self, track: &GeneratedTrack) -> Result<Vec3> {
        let s = track.sample_at_distance(self.station(track)?)?;
        Ok(s.position_m + s.frame.local_to_world(self.offset_m))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodePosition {
    World {
        position_m: Vec3,
        foundation: bool,
    },
    Track {
        attachment: Attachment,
    },
    /// Foundation X/Z follows its attachment; elevation is an authored world Y.
    Foundation {
        attachment: Attachment,
        elevation_m: f64,
    },
}
impl NodePosition {
    pub fn resolve(&self, track: &GeneratedTrack) -> Result<Vec3> {
        match self {
            Self::World { position_m, .. } => Ok(*position_m),
            Self::Track { attachment } => attachment.resolve(track),
            Self::Foundation {
                attachment,
                elevation_m,
            } => {
                let mut p = attachment.resolve(track)?;
                p.y = *elevation_m;
                Ok(p)
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: u64,
    pub name: String,
    pub position: NodePosition,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemberProfile {
    Circular { diameter_m: f64 },
    Rectangular { width_m: f64, height_m: f64 },
}
impl MemberProfile {
    pub fn profile(&self) -> Profile {
        match *self {
            Self::Circular { diameter_m } => Profile::Circle {
                radius_m: diameter_m / 2.,
                sides: 16,
            },
            Self::Rectangular { width_m, height_m } => Profile::Box { width_m, height_m },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub id: u64,
    pub start_node: u64,
    pub end_node: u64,
    pub profile: MemberProfile,
    pub roll_rad: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimberFamily {
    TraditionalBent,
    HybridLattice,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimberRecipe {
    pub family: TimberFamily,
    pub start_m: f64,
    pub end_m: f64,
    pub spacing_m: f64,
    pub width_m: f64,
    pub foundation_elevation_m: f64,
    pub member_width_m: f64,
    pub member_height_m: f64,
}
impl Default for TimberRecipe {
    fn default() -> Self {
        Self {
            family: TimberFamily::TraditionalBent,
            start_m: 0.,
            end_m: 10.,
            spacing_m: 3.,
            width_m: 3.,
            foundation_elevation_m: -5.,
            member_width_m: 0.18,
            member_height_m: 0.22,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub id: u64,
    pub name: String,
    pub nodes: Vec<Node>,
    pub members: Vec<Member>,
    pub material: Material,
    pub visible: bool,
    pub recipe: Option<TimberRecipe>,
}
impl Structure {
    pub fn empty(id: u64) -> Self {
        Self {
            id,
            name: format!("Support {id}"),
            nodes: vec![],
            members: vec![],
            material: Material {
                id: format!("support-{id}"),
                library: None,
                base_color: [0.42, 0.24, 0.10],
                metallic: 0.,
                roughness: 0.8,
            },
            visible: true,
            recipe: None,
        }
    }
    pub fn validate(&self, section_ids: &[SectionId]) -> Result<()> {
        ensure(
            self.id > 0 && !self.name.trim().is_empty() && self.name.len() <= 1024,
            "invalid support identity",
        )?;
        ensure(
            self.nodes.len() <= 8192 && self.members.len() <= 16384,
            "support node/member budget exceeded",
        )?;
        let mut ids = std::collections::HashSet::new();
        let attachment = |a: &Attachment| {
            ensure(
                section_ids.contains(&a.section_id)
                    && a.fraction.is_finite()
                    && (0. ..=1.).contains(&a.fraction)
                    && a.offset_m.is_finite()
                    && a.offset_m.length() <= 10000.,
                "invalid support attachment; referenced region must exist",
            )
        };
        for n in &self.nodes {
            ensure(
                n.id > 0 && ids.insert(n.id) && n.name.len() <= 1024,
                "duplicate/invalid support node ID",
            )?;
            match &n.position {
                NodePosition::World { position_m, .. } => ensure(
                    position_m.is_finite() && position_m.length() <= 1e8,
                    "invalid support position",
                )?,
                NodePosition::Track { attachment: a } => attachment(a)?,
                NodePosition::Foundation {
                    attachment: a,
                    elevation_m,
                } => {
                    attachment(a)?;
                    ensure(
                        elevation_m.is_finite() && elevation_m.abs() <= 1e8,
                        "invalid foundation elevation",
                    )?;
                }
            }
        }
        let mut members = std::collections::HashSet::new();
        for m in &self.members {
            ensure(
                m.id > 0
                    && members.insert(m.id)
                    && ids.contains(&m.start_node)
                    && ids.contains(&m.end_node)
                    && m.start_node != m.end_node
                    && m.roll_rad.is_finite(),
                "invalid member ID, connection or roll",
            )?;
            m.profile
                .profile()
                .validate()
                .map_err(|e| crate::GeometryError(e.to_string()))?;
        }
        ensure(
            !self.material.id.is_empty()
                && self.material.id.len() <= 128
                && self
                    .material
                    .base_color
                    .iter()
                    .chain([&self.material.metallic, &self.material.roughness])
                    .all(|x| x.is_finite() && (0. ..=1.).contains(x)),
            "invalid support material",
        )?;
        if let Some(binding) = &self.material.library {
            binding
                .validate()
                .map_err(|e| crate::GeometryError(e.to_string()))?;
        }
        if let Some(r) = &self.recipe {
            r.validate()?;
        }
        Ok(())
    }
}
impl TimberRecipe {
    pub fn validate(&self) -> Result<()> {
        ensure(
            [
                self.start_m,
                self.end_m,
                self.spacing_m,
                self.width_m,
                self.foundation_elevation_m,
                self.member_width_m,
                self.member_height_m,
            ]
            .iter()
            .all(|v| v.is_finite())
                && self.start_m >= 0.
                && self.end_m > self.start_m
                && self.end_m <= 1e7
                && (0.5..=30.).contains(&self.spacing_m)
                && (0.5..=30.).contains(&self.width_m)
                && self.foundation_elevation_m.abs() <= 1e8
                && (0.03..=1.).contains(&self.member_width_m)
                && (0.03..=1.).contains(&self.member_height_m)
                && (self.end_m - self.start_m) / self.spacing_m <= 511.,
            "invalid timber recipe (maximum 512 bents)",
        )
    }
}
/// Original bent/lattice construction recipe. Produces ordinary editable nodes
/// and members; explicit regeneration replaces a structure, never every frame.
pub fn timber(id: u64, recipe: TimberRecipe, track: &GeneratedTrack) -> Result<Structure> {
    recipe.validate()?;
    ensure(
        recipe.end_m <= track.length_m(),
        "timber range exceeds track",
    )?;
    let mut s = Structure::empty(id);
    s.name = match recipe.family {
        TimberFamily::TraditionalBent => "Traditional timber bents",
        TimberFamily::HybridLattice => "Hybrid timber lattice",
    }
    .into();
    let count = ((recipe.end_m - recipe.start_m) / recipe.spacing_m).ceil() as usize;
    let connect = |s: &mut Structure, a, b| {
        s.members.push(Member {
            id: s.members.len() as u64 + 1,
            start_node: a,
            end_node: b,
            profile: MemberProfile::Rectangular {
                width_m: recipe.member_width_m,
                height_m: recipe.member_height_m,
            },
            roll_rad: 0.,
        });
    };
    for i in 0..=count {
        let station = recipe.start_m + (recipe.end_m - recipe.start_m) * i as f64 / count as f64;
        for side in [-1., 1.] {
            let attachment = Attachment::at_station(
                track,
                station,
                Vec3::new(0., -0.5, side * recipe.width_m / 2.),
            )?;
            ensure(
                attachment.resolve(track)?.y > recipe.foundation_elevation_m + 0.25,
                "timber attachment must be above foundation by at least 0.25 m",
            )?;
            s.nodes.push(Node {
                id: s.nodes.len() as u64 + 1,
                name: format!("Bent {} footing", i + 1),
                position: NodePosition::Foundation {
                    attachment: attachment.clone(),
                    elevation_m: recipe.foundation_elevation_m,
                },
            });
            s.nodes.push(Node {
                id: s.nodes.len() as u64 + 1,
                name: format!("Bent {} track mount", i + 1),
                position: NodePosition::Track { attachment },
            });
        }
        let b = i as u64 * 4 + 1;
        for (a, c) in [
            (b, b + 1),
            (b + 2, b + 3),
            (b + 1, b + 3),
            (b, b + 3),
            (b + 2, b + 1),
        ] {
            connect(&mut s, a, c);
        }
        if recipe.family == TimberFamily::HybridLattice && i > 0 {
            for (a, c) in [
                (b - 3, b + 1),
                (b - 1, b + 3),
                (b - 4, b + 1),
                (b - 3, b),
                (b - 2, b + 3),
                (b - 1, b + 2),
            ] {
                connect(&mut s, a, c);
            }
        }
    }
    s.recipe = Some(recipe);
    Ok(s)
}
/// Append distinct support solids to the established PBR mesh representation.
pub fn append_mesh(
    track: &GeneratedTrack,
    structures: &[Structure],
    mesh: &mut crate::construction::TrackMesh,
) -> Result<()> {
    use crate::construction::{Bounds, Component};
    for s in structures {
        s.validate(&track.sections.iter().map(|s| s.id).collect::<Vec<_>>())?;
        let material = format!("support/material/{}", s.id);
        let mut mat = s.material.clone();
        mat.id = material.clone();
        mesh.materials.push(mat);
        let nodes: std::collections::HashMap<_, _> = s
            .nodes
            .iter()
            .map(|n| Ok((n.id, n.position.resolve(track)?)))
            .collect::<Result<_>>()?;
        for m in &s.members {
            let a = *nodes
                .get(&m.start_node)
                .ok_or_else(|| crate::GeometryError("support start node missing".into()))?;
            let b = *nodes
                .get(&m.end_node)
                .ok_or_else(|| crate::GeometryError("support end node missing".into()))?;
            let mut solid =
                crate::construction::member_solid(a, b, &m.profile.profile(), m.roll_rad)?;
            ensure(
                mesh.stats.vertices + solid.vertices.len() <= 2_000_000,
                "combined support/track vertex budget exceeded",
            )?;
            for vertex in &mut solid.vertices {
                vertex.source_station_m = -(mesh.components.len() as f64 + 1.);
            }
            let bounds: Bounds = solid.bounds;
            mesh.stats.vertices += solid.vertices.len();
            mesh.stats.triangles += solid.indices.len() / 3;
            mesh.stats.rendered_triangles += solid.indices.len() / 3;
            mesh.stats.bytes += solid.vertices.len()
                * std::mem::size_of::<crate::construction::MeshVertex>()
                + solid.indices.len() * 4;
            mesh.bounds.union(bounds);
            mesh.components.push(Component {
                id: format!("support/{}/{}", s.id, m.id),
                role: qx_style::construction::Role::Structure,
                material: material.clone(),
                visible: s.visible,
                mesh: solid,
                placements: vec![],
                bounds,
            });
        }
    }
    Ok(())
}
