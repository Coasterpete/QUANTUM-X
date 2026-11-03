use super::{expansion, session::Session};
use qx_geometry::{math::Vec3, supports::*};
use qx_rendering::InspectionScene;

pub(super) fn show(s: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    let track = match s.expansion.attachment_track(scene, s.visible_revision) {
        Ok(t) => t,
        Err(e) => {
            ui.label(e);
            return;
        }
    };
    super::theme::heading(ui, "SUPPORT WORKSPACE", super::theme::RED);
    ui.small("Editable construction · provisional control placement · no structural analysis.");
    ui.horizontal_wrapped(|ui| super::snap_ui::toolbar(s, ui, false));
    let snap = s.workbench.snap;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Create structure").clicked()
            && let Some(id) = s
                .expansion
                .supports
                .iter()
                .map(|v| v.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
        {
            s.expansion.supports.push(Structure::empty(id));
            s.expansion.structure = Some(id);
            s.expansion.node = None;
            s.expansion.member = None;
        }
        if ui.button("Inspect / frame track").clicked() {
            s.frame_requested = true;
        }
    });
    for structure in &s.expansion.supports {
        if ui
            .selectable_label(
                s.expansion.structure == Some(structure.id),
                format!("{} · {} members", structure.name, structure.members.len()),
            )
            .clicked()
        {
            s.expansion.structure = Some(structure.id);
            s.expansion.node = None;
            s.expansion.member = None;
        }
    }
    if let Some(index) = s
        .expansion
        .supports
        .iter()
        .position(|v| Some(v.id) == s.expansion.structure)
    {
        ui.separator();
        let selected_structure = s.expansion.supports[index].id;
        let points: Vec<_> = s
            .expansion
            .supports
            .iter()
            .flat_map(|structure| {
                structure
                    .nodes
                    .iter()
                    .filter(|n| {
                        structure.id != selected_structure || Some(n.id) != s.expansion.node
                    })
                    .filter_map(|n| n.position.resolve(&track).ok())
            })
            .collect();
        let mut stations: Vec<_> = track
            .sections
            .iter()
            .flat_map(|r| [r.start_distance_m, r.end_distance_m])
            .collect();
        stations.extend(s.expansion.supports.iter().flat_map(|structure| {
            structure
                .nodes
                .iter()
                .filter(|n| structure.id != selected_structure || Some(n.id) != s.expansion.node)
                .filter_map(|n| match &n.position {
                    NodePosition::Track { attachment }
                    | NodePosition::Foundation { attachment, .. } => {
                        attachment.station(&track).ok()
                    }
                    _ => None,
                })
        }));
        let structure = &mut s.expansion.supports[index];
        ui.push_id(("support", structure.id), |ui| {
            ui.text_edit_singleline(&mut structure.name);
            ui.checkbox(&mut structure.visible, "Visible");
            ui.collapsing("PBR appearance", |ui| {
                ui.color_edit_button_rgb(&mut structure.material.base_color);
                ui.add(
                    egui::Slider::new(&mut structure.material.metallic, 0. ..=1.).text("Metallic"),
                );
                ui.add(
                    egui::Slider::new(&mut structure.material.roughness, 0. ..=1.)
                        .text("Roughness"),
                );
            });
            ui.horizontal_wrapped(|ui| {
                for (kind, label) in [(0, "Add node"), (1, "Track mount"), (2, "Foundation")] {
                    if ui.button(label).clicked() {
                        let station = s
                            .expansion
                            .selected_station_m
                            .unwrap_or(0.)
                            .clamp(0., track.length_m());
                        let station = snap
                            .spatial_value(
                                station,
                                super::snap::Space::Station,
                                &stations,
                                ui.input(|i| i.modifiers.shift),
                                ui.input(|i| i.modifiers.alt),
                            )
                            .clamp(0., track.length_m());
                        let attachment =
                            Attachment::at_station(&track, station, Vec3::new(0., -0.5, 0.));
                        if let (Some(id), Ok(attachment)) = (
                            structure
                                .nodes
                                .iter()
                                .map(|n| n.id)
                                .max()
                                .unwrap_or(0)
                                .checked_add(1),
                            attachment,
                        ) {
                            let position = match kind {
                                1 => NodePosition::Track { attachment },
                                2 => NodePosition::Foundation {
                                    attachment,
                                    elevation_m: s.expansion.timber.foundation_elevation_m,
                                },
                                _ => NodePosition::World {
                                    position_m: scene
                                        .track
                                        .sample_at_distance(station)
                                        .unwrap()
                                        .position_m
                                        + Vec3::new(0., -2., 0.),
                                    foundation: false,
                                },
                            };
                            structure.nodes.push(Node {
                                id,
                                name: format!("{label} {id}"),
                                position,
                            });
                            s.expansion.node = Some(id);
                            s.expansion.member = None;
                        }
                    }
                }
            });
            egui::CollapsingHeader::new("Nodes / foundations")
                .default_open(true)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("support nodes")
                        .max_height(110.)
                        .show(ui, |ui| {
                            for node in &structure.nodes {
                                if ui
                                    .selectable_label(s.expansion.node == Some(node.id), &node.name)
                                    .clicked()
                                {
                                    s.expansion.node = Some(node.id);
                                    s.expansion.member = None;
                                }
                            }
                        });
                    if let Some(i) = structure
                        .nodes
                        .iter()
                        .position(|n| Some(n.id) == s.expansion.node)
                    {
                        let node = &mut structure.nodes[i];
                        ui.text_edit_singleline(&mut node.name);
                        match &mut node.position {
                            NodePosition::World {
                                position_m,
                                foundation,
                            } => {
                                super::snap_ui::vector(
                                    ui,
                                    position_m,
                                    snap,
                                    super::snap::Space::World,
                                    &points,
                                );
                                ui.checkbox(foundation, "Foundation node");
                            }
                            NodePosition::Track { attachment } => {
                                attachment_controls(ui, attachment, &track, snap, &stations)
                            }
                            NodePosition::Foundation {
                                attachment,
                                elevation_m,
                            } => {
                                attachment_controls(ui, attachment, &track, snap, &stations);
                                let elevations: Vec<_> = points.iter().map(|p| p.y).collect();
                                super::snap_ui::number(
                                    ui,
                                    elevation_m,
                                    "Foundation elevation Y m ",
                                    0.1,
                                    snap,
                                    super::snap::Space::World,
                                    &elevations,
                                );
                            }
                        }
                        if ui.button("Delete node and incident members").clicked() {
                            let id = node.id;
                            structure
                                .members
                                .retain(|m| m.start_node != id && m.end_node != id);
                            structure.nodes.remove(i);
                            s.expansion.node = None;
                        }
                    }
                });
            ui.collapsing("Members / connections", |ui| {
                if ui
                    .add_enabled(
                        structure.nodes.len() >= 2,
                        egui::Button::new("Connect two nodes"),
                    )
                    .clicked()
                    && let Some(id) = structure
                        .members
                        .iter()
                        .map(|m| m.id)
                        .max()
                        .unwrap_or(0)
                        .checked_add(1)
                {
                    structure.members.push(Member {
                        id,
                        start_node: structure.nodes[0].id,
                        end_node: structure.nodes[1].id,
                        profile: MemberProfile::Circular { diameter_m: 0.2 },
                        roll_rad: 0.,
                    });
                    s.expansion.member = Some(id);
                    s.expansion.node = None;
                }
                egui::ScrollArea::vertical()
                    .id_salt("support members")
                    .max_height(110.)
                    .show(ui, |ui| {
                        for m in &structure.members {
                            if ui
                                .selectable_label(
                                    s.expansion.member == Some(m.id),
                                    format!("Member {} · {} → {}", m.id, m.start_node, m.end_node),
                                )
                                .clicked()
                            {
                                s.expansion.member = Some(m.id);
                                s.expansion.node = None;
                            }
                        }
                    });
                if let Some(i) = structure
                    .members
                    .iter()
                    .position(|m| Some(m.id) == s.expansion.member)
                {
                    let m = &mut structure.members[i];
                    for (salt, label, id) in [
                        ("start", "From node", &mut m.start_node),
                        ("end", "To node", &mut m.end_node),
                    ] {
                        egui::ComboBox::from_id_salt(salt)
                            .selected_text(format!("{label} {}", id))
                            .show_ui(ui, |ui| {
                                for n in &structure.nodes {
                                    ui.selectable_value(id, n.id, &n.name);
                                }
                            });
                    }
                    let mut rectangular = matches!(m.profile, MemberProfile::Rectangular { .. });
                    if ui
                        .checkbox(&mut rectangular, "Rectangular member")
                        .changed()
                    {
                        m.profile = if rectangular {
                            MemberProfile::Rectangular {
                                width_m: 0.18,
                                height_m: 0.22,
                            }
                        } else {
                            MemberProfile::Circular { diameter_m: 0.2 }
                        };
                    }
                    match &mut m.profile {
                        MemberProfile::Circular { diameter_m } => {
                            ui.add(
                                egui::DragValue::new(diameter_m)
                                    .speed(0.005)
                                    .range(0.001..=100.)
                                    .prefix("Diameter m "),
                            );
                        }
                        MemberProfile::Rectangular { width_m, height_m } => {
                            ui.add(
                                egui::DragValue::new(width_m)
                                    .speed(0.005)
                                    .range(0.001..=100.)
                                    .prefix("Width m "),
                            );
                            ui.add(
                                egui::DragValue::new(height_m)
                                    .speed(0.005)
                                    .range(0.001..=100.)
                                    .prefix("Height m "),
                            );
                        }
                    }
                    super::snap_ui::number(
                        ui,
                        &mut m.roll_rad,
                        "Axial roll rad ",
                        0.01,
                        snap,
                        super::snap::Space::Angle,
                        &[],
                    );
                    if ui.button("Delete member").clicked() {
                        structure.members.remove(i);
                        s.expansion.member = None;
                    }
                }
            });
        });
        if ui.button("Delete structure").clicked() {
            s.expansion.supports.remove(index);
            s.expansion.structure = None;
        }
    }
    ui.separator();
    ui.collapsing("Generate timber supports",|ui| {
        let r=&mut s.expansion.timber;
        ui.selectable_value(&mut r.family,TimberFamily::TraditionalBent,"Traditional timber bents");
        ui.selectable_value(&mut r.family,TimberFamily::HybridLattice,"Hybrid timber lattice");
        for (v,label) in [(&mut r.start_m,"Start m "),(&mut r.end_m,"End m "),(&mut r.spacing_m,"Maximum bent spacing m "),
            (&mut r.width_m,"Bent width m "),(&mut r.foundation_elevation_m,"Foundation Y m "),
            (&mut r.member_width_m,"Timber width m "),(&mut r.member_height_m,"Timber height m ")] {
            ui.add(egui::DragValue::new(v).speed(0.1).prefix(label));
        }
        if ui.button("Use selected track region").clicked()
            && let Some(r)=track.sections.iter().find(|v|v.id==s.selected) {s.expansion.timber.start_m=r.start_distance_m;s.expansion.timber.end_m=r.end_distance_m;}
        if ui.button("Generate editable timber structure").clicked()
            && let Some(id)=s.expansion.supports.iter().map(|v|v.id).max().unwrap_or(0).checked_add(1) {
                match timber(id,s.expansion.timber.clone(),&track) {
                    Ok(structure)=>{s.expansion.supports.push(structure);s.expansion.structure=Some(id);s.expansion.node=None;s.expansion.member=None;}
                    Err(e)=>s.expansion.message=Some(e.to_string()),
                }
            }
        if let Some(i)=s.expansion.supports.iter().position(|v|Some(v.id)==s.expansion.structure && v.recipe.is_some())
            && ui.button("Replace selected timber structure from recipe").on_hover_text("Explicit replacement discards edits to this structure only when Apply is pressed; Cancel restores them.").clicked() {
                let id=s.expansion.supports[i].id;
                match timber(id,s.expansion.timber.clone(),&track) {
                    Ok(structure)=>s.expansion.supports[i]=structure,Err(e)=>s.expansion.message=Some(e.to_string()),
                }
            }
        ui.small("Generated nodes and members remain individually editable. Track mounts and foundation X/Z regenerate with their region. No load-bearing validation.");
    });
    expansion::actions(s, ui);
}
fn attachment_controls(
    ui: &mut egui::Ui,
    a: &mut Attachment,
    t: &qx_geometry::solver::GeneratedTrack,
    snap: super::snap::Settings,
    stations: &[f64],
) {
    if let Ok(mut station) = a.station(t)
        && super::snap_ui::number(
            ui,
            &mut station,
            "Track station m ",
            0.1,
            snap,
            super::snap::Space::Station,
            stations,
        )
        .changed()
        && let Ok(new) = Attachment::at_station(t, station.clamp(0., t.length_m()), a.offset_m)
    {
        *a = new;
    }
    ui.small("Offsets in track forward / up / lateral axes");
    super::snap_ui::vector(ui, &mut a.offset_m, snap, super::snap::Space::Local, &[]);
}
