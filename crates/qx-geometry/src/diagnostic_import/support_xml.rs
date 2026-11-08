use super::{MAX_BEAMS, MAX_INPUT_BYTES, MAX_SUPPORT_NODES, bounds, number, position};
use crate::{
    GeometryError, Result,
    construction::{Bounds, Component, TrackMesh},
    ensure,
    math::Vec3,
};
use qx_style::construction::{Material, Profile, Role};
use roxmltree::Node;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct SupportNode {
    pub id: String,
    pub position_m: Vec3,
    pub kind: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RailConnector {
    pub node_ids: Vec<String>,
    pub kind: String,
    pub track_index: u32,
    pub station_m: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Footer {
    pub node_id: String,
    pub base_type: String,
    pub connection_type: String,
    pub rotation_rad: f64,
    pub height_above_terrain_m: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Prefab {
    pub path: String,
    pub station_m: f64,
    pub track_index: u32,
    pub atomized: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Beam {
    pub start: String,
    pub end: String,
    pub kind: String,
    pub diameter_m: Option<f64>,
    pub rotation_rad: f64,
    pub vertical_rotation: bool,
    pub start_extra_m: f64,
    pub end_extra_m: f64,
    pub beam_nodes: Vec<(String, f64, String)>,
}
impl Beam {
    pub fn endpoints(&self, nodes: &BTreeMap<String, SupportNode>) -> Result<(Vec3, Vec3)> {
        let a = nodes
            .get(&self.start)
            .ok_or_else(|| GeometryError(format!("missing start node {}", self.start)))?
            .position_m;
        let b = nodes
            .get(&self.end)
            .ok_or_else(|| GeometryError(format!("missing end node {}", self.end)))?
            .position_m;
        let direction = (b - a).normalized()?;
        ensure(
            (b - a).length() + self.start_extra_m + self.end_extra_m > 1e-5,
            "beam trims consume entire member",
        )?;
        Ok((
            a - direction * self.start_extra_m,
            b + direction * self.end_extra_m,
        ))
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSupports {
    pub nodes: BTreeMap<String, SupportNode>,
    pub beams: Vec<Beam>,
    pub connectors: Vec<RailConnector>,
    pub footers: Vec<Footer>,
    pub prefabs: Vec<Prefab>,
    pub bounds: Option<Bounds>,
    /// Counts by feature, never silently discard unimplemented metadata.
    pub unsupported: BTreeMap<String, usize>,
    /// Retains all properties, including unknown XML. Never written into a project.
    pub source_xml: String,
}
fn required<'a>(node: Node<'a, '_>, key: &str) -> Result<&'a str> {
    node.attribute(key)
        .ok_or_else(|| GeometryError(format!("{} missing {key}", node.tag_name().name())))
}
fn child_number(node: Node<'_, '_>, key: &str, default: f64) -> Result<f64> {
    let mut children = node.children().filter(|n| n.has_tag_name(key));
    let value = children
        .next()
        .map(|n| number(n.text().unwrap_or("")))
        .transpose()?
        .unwrap_or(default);
    ensure(children.next().is_none(), format!("duplicate {key}"))?;
    Ok(value)
}
fn world_position(node: Node<'_, '_>) -> Result<Vec3> {
    let mut children = node.children().filter(|n| n.has_tag_name("pos"));
    let pos = children
        .next()
        .ok_or_else(|| GeometryError("support node has no world pos".into()))?;
    ensure(children.next().is_none(), "duplicate world pos")?;
    let values = pos
        .text()
        .unwrap_or("")
        .split_whitespace()
        .map(number)
        .collect::<Result<Vec<_>>>()?;
    ensure(values.len() == 3, "support pos must have three components")?;
    position(Vec3::new(values[0], values[1], values[2]))
}
impl ImportedSupports {
    fn warn(&mut self, feature: impl Into<String>) {
        *self.unsupported.entry(feature.into()).or_default() += 1;
    }
    fn properties(&mut self, node: Node<'_, '_>, attrs: &[&str], children: &[&str]) {
        for a in node.attributes() {
            if !attrs.contains(&a.name()) {
                self.warn(format!("{}/@{}", node.tag_name().name(), a.name()));
            }
        }
        for c in node.children().filter(Node::is_element) {
            if !children.contains(&c.tag_name().name()) {
                self.warn(format!(
                    "{}/{}",
                    node.tag_name().name(),
                    c.tag_name().name()
                ));
            }
        }
    }
    fn insert(&mut self, node: Node<'_, '_>, p: Vec3, kind: &str) -> Result<String> {
        let id = required(node, "id")?;
        ensure(
            !id.trim().is_empty() && id.len() <= 128,
            "node ID must contain 1..128 bytes",
        )?;
        ensure(
            self.nodes.len() < MAX_SUPPORT_NODES,
            "support node budget exceeded",
        )?;
        ensure(
            !self.nodes.contains_key(id),
            format!("duplicate support node ID {id}"),
        )?;
        self.nodes.insert(
            id.into(),
            SupportNode {
                id: id.into(),
                position_m: p,
                kind: kind.into(),
            },
        );
        Ok(id.into())
    }
    fn parse_children(&mut self, parent: Node<'_, '_>) -> Result<()> {
        for n in parent.children().filter(Node::is_element) {
            match n.tag_name().name() {
                "freenode" => {
                    self.properties(n, &["id"], &["pos"]);
                    self.insert(n, world_position(n)?, "free")?;
                }
                "footernode" => {
                    self.properties(
                        n,
                        &["id", "contype", "basetype"],
                        &["pos", "rotation", "height_above_terrain"],
                    );
                    let id = self.insert(n, world_position(n)?, "footer")?;
                    let height = child_number(n, "height_above_terrain", 0.)?;
                    ensure(
                        (0. ..=1000.).contains(&height),
                        "footer height outside limits",
                    )?;
                    self.footers.push(Footer {
                        node_id: id,
                        base_type: required(n, "basetype")?.into(),
                        connection_type: required(n, "contype")?.into(),
                        rotation_rad: child_number(n, "rotation", 0.)?,
                        height_above_terrain_m: height,
                    });
                    self.warn("footer solid dimensions unavailable; world-position marker only");
                }
                "rasc" => {
                    self.properties(
                        n,
                        &["type", "center_rails_coord", "custom_track_index"],
                        &["subnode", "colormode_mainspine"],
                    );
                    let station = number(required(n, "center_rails_coord")?)?;
                    ensure(station >= 0., "negative rail connector station")?;
                    let index = required(n, "custom_track_index")?
                        .parse::<u32>()
                        .map_err(|_| GeometryError("invalid custom_track_index".into()))?;
                    let mut ids = vec![];
                    for sub in n.children().filter(|c| c.has_tag_name("subnode")) {
                        self.properties(sub, &["id"], &["pos"]);
                        ids.push(self.insert(sub, world_position(sub)?, "rail_connector")?);
                    }
                    ensure(!ids.is_empty(), "rail connector has no positioned subnode")?;
                    self.connectors.push(RailConnector {
                        node_ids: ids,
                        kind: required(n, "type")?.into(),
                        track_index: index,
                        station_m: station,
                    });
                    self.warn(
                        "rail connector mesh/orientation unavailable; world-position marker only",
                    );
                    if n.children().any(|n| n.has_tag_name("colormode_mainspine")) {
                        self.warn("coaster spine color unavailable; diagnostic material used");
                    }
                }
                "beam" => self.parse_beam(n)?,
                "prefab" => {
                    self.properties(
                        n,
                        &["path", "center_rails_coord", "custom_track_index"],
                        &["atomization"],
                    );
                    let station_m = number(required(n, "center_rails_coord")?)?;
                    ensure(station_m >= 0., "negative prefab station")?;
                    let track_index = required(n, "custom_track_index")?
                        .parse::<u32>()
                        .map_err(|_| GeometryError("invalid prefab track index".into()))?;
                    let atomizations = n
                        .children()
                        .filter(|c| c.has_tag_name("atomization"))
                        .collect::<Vec<_>>();
                    ensure(atomizations.len() <= 1, "duplicate prefab atomization")?;
                    let mut atomized = false;
                    if let Some(atomization) = atomizations.first() {
                        self.properties(*atomization, &[], &["supports"]);
                        let containers = atomization
                            .children()
                            .filter(|c| c.has_tag_name("supports"))
                            .collect::<Vec<_>>();
                        ensure(
                            containers.len() == 1,
                            "atomization requires one supports container",
                        )?;
                        // NL2 examples embed explicit world-position nodes, with global
                        // IDs. No prefab path lookup or inferred transform is applied.
                        self.parse_children(containers[0])?;
                        atomized = true;
                    } else {
                        self.warn("prefab without explicit atomization omitted");
                    }
                    self.prefabs.push(Prefab {
                        path: required(n, "path")?.into(),
                        station_m,
                        track_index,
                        atomized,
                    });
                }
                // Unknown transforms cannot be flattened safely. Keep their XML,
                // report the omission, and never render untransformed children.
                _ => self.warn(format!(
                    "unsupported structure/subtree {}",
                    n.tag_name().name()
                )),
            }
        }
        Ok(())
    }
    fn parse_beam(&mut self, n: Node<'_, '_>) -> Result<()> {
        ensure(self.beams.len() < MAX_BEAMS, "support beam budget exceeded")?;
        self.properties(
            n,
            &["start", "end", "type", "size1"],
            &[
                "beamnode",
                "rotation",
                "vertical",
                "start_extra_length",
                "end_extra_length",
            ],
        );
        let start = required(n, "start")?.to_owned();
        let end = required(n, "end")?.to_owned();
        let a = self
            .nodes
            .get(&start)
            .ok_or_else(|| GeometryError(format!("beam start {start} must precede beam")))?
            .position_m;
        let b = self
            .nodes
            .get(&end)
            .ok_or_else(|| GeometryError(format!("beam end {end} must precede beam")))?
            .position_m;
        ensure(
            (b - a).length() > 1e-5 && (b - a).length() <= 1e5,
            "degenerate or excessive beam length",
        )?;
        let kind = required(n, "type")?.to_owned();
        let size = n.attribute("size1").map(number).transpose()?;
        if let Some(size) = size {
            ensure((1e-5..=100.).contains(&size), "beam size1 outside limits")?;
        }
        let diameter = if kind == "1" {
            Some(size.ok_or_else(|| GeometryError("pipe beam requires size1 diameter".into()))?)
        } else {
            self.warn(format!("beam profile type {kind}; connectivity only"));
            None
        };
        let mut beam = Beam {
            start,
            end,
            kind,
            diameter_m: diameter,
            rotation_rad: child_number(n, "rotation", 0.)?,
            vertical_rotation: n.children().any(|n| n.has_tag_name("vertical")),
            start_extra_m: child_number(n, "start_extra_length", 0.)?,
            end_extra_m: child_number(n, "end_extra_length", 0.)?,
            beam_nodes: vec![],
        };
        ensure(
            beam.start_extra_m.abs() <= 1000. && beam.end_extra_m.abs() <= 1000.,
            "beam extra length exceeds limit",
        )?;
        beam.endpoints(&self.nodes)?;
        // NL2 beam-node pos is a fraction of the node-to-node member, before
        // visual end extensions. Nested nodes are available to subsequent beams.
        for child in n.children().filter(|c| c.has_tag_name("beamnode")) {
            self.properties(child, &["id", "pos", "type"], &[]);
            let t = number(required(child, "pos")?)?;
            ensure((0. ..=1.).contains(&t), "beam node fraction outside [0,1]")?;
            let kind = required(child, "type")?.to_owned();
            let id = self.insert(child, a + (b - a) * t, "beam_node")?;
            if kind != "0" {
                self.warn(format!(
                    "beamnode type {kind} hardware omitted; connection retained"
                ));
            }
            beam.beam_nodes.push((id, t, kind));
        }
        self.beams.push(beam);
        Ok(())
    }
    /// Existing support solid generator and existing PBR components; no renderer.
    /// Build transactionally so a failed import cannot partially replace a scene.
    pub fn append_mesh(&self, mesh: &mut TrackMesh) -> Result<()> {
        let mut additions = vec![];
        let mut vertices = mesh.stats.vertices;
        for (i, b) in self.beams.iter().enumerate() {
            let Some(d) = b.diameter_m else { continue };
            let (a, z) = b.endpoints(&self.nodes)?;
            let mut solid = crate::construction::member_solid(
                a,
                z,
                &Profile::Circle {
                    radius_m: d / 2.,
                    sides: 16,
                },
                b.rotation_rad,
            )?;
            vertices += solid.vertices.len();
            ensure(
                vertices <= 2_000_000,
                "combined track/support vertex budget exceeded",
            )?;
            for v in &mut solid.vertices {
                v.source_station_m = -(i as f64 + 1.);
            }
            let bounds = solid.bounds;
            additions.push(Component {
                id: format!("support/1/{}", i + 1),
                role: Role::Structure,
                material: "nl2-support".into(),
                visible: true,
                mesh: solid,
                placements: vec![],
                bounds,
            });
        }
        mesh.materials.push(Material {
            id: "nl2-support".into(),
            library: None,
            base_color: [0.66, 0.69, 0.72],
            metallic: 0.45,
            roughness: 0.5,
        });
        for c in additions {
            mesh.stats.vertices += c.mesh.vertices.len();
            mesh.stats.triangles += c.mesh.indices.len() / 3;
            mesh.stats.rendered_triangles += c.mesh.indices.len() / 3;
            mesh.stats.bytes += c.mesh.vertices.len()
                * std::mem::size_of::<crate::construction::MeshVertex>()
                + c.mesh.indices.len() * 4;
            mesh.bounds.union(c.bounds);
            mesh.components.push(c);
        }
        // Include isolated nodes and exact foundation elevations in camera fitting.
        if let Some(b) = self.bounds {
            mesh.bounds.union(b);
        }
        Ok(())
    }
}
pub fn parse_support_xml(text: &str) -> Result<ImportedSupports> {
    ensure(text.len() <= MAX_INPUT_BYTES, "XML exceeds 32 MiB")?;
    ensure(
        !text.contains("<!DOCTYPE") && !text.contains("<!ENTITY"),
        "XML DTD/entities are forbidden",
    )?;
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 150_000,
        },
    )
    .map_err(|e| GeometryError(format!("support XML: {e}")))?;
    ensure(
        doc.root_element().has_tag_name("root"),
        "expected XML root element",
    )?;
    for n in doc.descendants().filter(Node::is_element) {
        ensure(
            n.ancestors().take(34).count() <= 32,
            "XML nesting exceeds 32 levels",
        )?;
        ensure(
            n.tag_name().namespace().is_none(),
            "namespaced support XML is unsupported",
        )?;
        ensure(
            n.attributes().len() <= 32,
            "XML element attribute budget exceeded",
        )?;
    }
    let root = doc.root_element();
    let mut containers = root.children().filter(|n| n.has_tag_name("supports"));
    let supports = containers
        .next()
        .ok_or_else(|| GeometryError("missing supports container".into()))?;
    ensure(containers.next().is_none(), "duplicate supports container")?;
    let mut out = ImportedSupports {
        nodes: BTreeMap::new(),
        beams: vec![],
        connectors: vec![],
        footers: vec![],
        prefabs: vec![],
        bounds: None,
        unsupported: BTreeMap::new(),
        source_xml: text.into(),
    };
    out.properties(root, &[], &["supports"]);
    for a in supports.attributes() {
        out.warn(format!("supports/@{}", a.name()));
    }
    out.parse_children(supports)?;
    let mut b = bounds(out.nodes.values().map(|n| n.position_m));
    for beam in &out.beams {
        let (a, z) = beam.endpoints(&out.nodes)?;
        if let Some(b) = &mut b {
            b.include(a);
            b.include(z);
        }
    }
    out.bounds = b;
    Ok(out)
}
