//! Provisional precision controls; accepted changes use the existing atomic train transaction.
use super::session::Session;
use qx_document::{command::Command, train::mechanism::*};
use qx_geometry::math::{Quaternion, Vec3};

pub(super) fn controls(s: &mut Session, ui: &mut egui::Ui) {
    let mut trains = s.editor.project().trains.clone();
    let mut changed = false;
    ui.collapsing("Vehicle mechanics · development controls",|ui| {
        ui.small("Numeric changes are undoable document transactions. Mounts set initial poses; joints govern motion. Inertia is about the mass center.");
        for train in &mut trains {
            let ids:Vec<_>=train.components.iter().map(|c|c.id.clone()).collect();
            ui.push_id(&train.id,|ui| {
                ui.label(&train.name);
                if let Some(style)=s.catalog.styles.iter().find(|v|v.id==s.editor.project().style.style_id) {
                    ui.small(format!("Compatibility declaration: {:?} · no dynamics certification",train.compatibility.assess(style)));
                }
                for c in &mut train.components {
                    ui.collapsing(format!("{} · {:?}",c.id,c.kind),|ui| {
                        ui.label("Mass (kg)"); changed|=ui.add(egui::DragValue::new(&mut c.mass_kg).speed(1.).range(0.001..=1e9)).changed();
                        changed|=vector(ui,"Principal inertia (kg m²)",&mut c.inertia_kg_m2);
                        changed|=vector(ui,"Mass center (component-local m)",&mut c.center_of_mass_m);
                        changed|=vector(ui,"Dimensions (m; visual reference only)",&mut c.dimensions_m);
                        if let Some(p)=&mut c.placement {
                            match p {
                                Placement::Track {offset_m,frame}=> {
                                    ui.label("Initial track station offset (m)");
                                    changed|=ui.add(egui::DragValue::new(offset_m).speed(0.01)).changed();
                                    changed|=local_frame(ui,frame);
                                }
                                Placement::Mounted {parent,frame}=> {
                                    egui::ComboBox::from_id_salt(("mount",&c.id)).selected_text(parent.as_str()).show_ui(ui,|ui| {
                                        for id in &ids { if id!=&c.id { changed|=ui.selectable_value(parent,id.clone(),id).changed(); } }
                                    });
                                    changed|=local_frame(ui,frame);
                                }
                            }
                        } else { ui.small("Legacy Schema 5 inspection placement"); }
                        ui.collapsing("Captive centerline guide points (not rail contact)",|ui| {
                            let mut remove=None;
                            for (i,p) in c.guide_references_m.iter_mut().enumerate() {
                                ui.push_id(i,|ui| {
                                    changed|=vector(ui,&format!("Local point {} (m)",i+1),p);
                                    if ui.button("Remove guide").clicked() {remove=Some(i);}
                                });
                            }
                            if let Some(i)=remove {c.guide_references_m.remove(i);changed=true;}
                            if ui.add_enabled(c.guide_references_m.len()<1024,egui::Button::new("Add guide at component origin")).clicked() {c.guide_references_m.push(Vec3::ZERO);changed=true;}
                        });
                    });
                }
                for j in &mut train.connections {
                    ui.collapsing(format!("{} · angular mechanism",j.id),|ui| {
                        let mut choice=match j.model {JointModel::Ball=>0,JointModel::Fixed {..}=>1,JointModel::Hinge {..}=>2};
                        let old=choice;
                        egui::ComboBox::from_id_salt(("joint type",&j.id)).selected_text(["Ball","Fixed","Hinge"][choice]).show_ui(ui,|ui| {
                            for (i,label) in ["Ball","Fixed","Hinge"].into_iter().enumerate() {ui.selectable_value(&mut choice,i,label);}
                        });
                        if old!=choice {
                            j.model=match choice {0=>JointModel::Ball,1=>JointModel::Fixed {frame_a:Quaternion::IDENTITY,frame_b:Quaternion::IDENTITY},_=>JointModel::Hinge {frame_a:Quaternion::IDENTITY,frame_b:Quaternion::IDENTITY,limits:None,drive:RotationDrive::Free}};
                            changed=true;
                        }
                        match &mut j.model {
                            JointModel::Ball=>{ui.small("Three free rotations; coincident anchors");}
                            JointModel::Fixed {frame_a,frame_b} | JointModel::Hinge {frame_a,frame_b,..}=> {
                                changed|=rotation(ui,"Joint frame A quaternion",frame_a);
                                changed|=rotation(ui,"Joint frame B quaternion",frame_b);
                            }
                        }
                        if let JointModel::Hinge {limits,drive,..}=&mut j.model {
                            ui.small("Joint-local Z is the axis; X is the zero-angle reference.");
                            let mut limited=limits.is_some();
                            if ui.checkbox(&mut limited,"Angular limits").changed() { *limits=limited.then_some(AngleLimits {min_rad:-1.2,max_rad:1.2});changed=true; }
                            if let Some(l)=limits {
                                ui.label("Lower / upper angle (rad)");
                                for v in [&mut l.min_rad,&mut l.max_rad] {changed|=ui.add(egui::DragValue::new(v).speed(0.01)).changed();}
                            }
                            if matches!(drive,RotationDrive::MechanicalProfile {..}) {
                                ui.small("Mechanical rotation profile: stored and inspectable; transmission solver requires future implementation. Playback rejects this mechanism.");
                            } else {
                                let mut motor=matches!(drive,RotationDrive::TorqueMotor {..});
                                if ui.checkbox(&mut motor,"Finite-torque position motor").changed() {
                                    *drive=if motor {RotationDrive::TorqueMotor {target_rad:0.,stiffness_nm_per_rad:200.,damping_nms_per_rad:30.,max_torque_nm:100.}} else {RotationDrive::Free};changed=true;
                                }
                                if let RotationDrive::TorqueMotor {target_rad,stiffness_nm_per_rad,damping_nms_per_rad,max_torque_nm}=drive {
                                    for (label,v) in [("Target (rad)",target_rad),("Stiffness (Nm/rad)",stiffness_nm_per_rad),("Damping (Nms/rad)",damping_nms_per_rad),("Torque limit (Nm)",max_torque_nm)] {
                                        ui.label(label);changed|=ui.add(egui::DragValue::new(v).speed(0.01)).changed();
                                    }
                                }
                            }
                        }
                    });
                }
            });
        }
    });
    if changed {
        s.execute(vec![Command::SetTrains(trains)]);
    }
}
fn vector(ui: &mut egui::Ui, label: &str, v: &mut Vec3) -> bool {
    ui.label(label);
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (axis, n) in [("X", &mut v.x), ("Y", &mut v.y), ("Z", &mut v.z)] {
            changed |= ui
                .add(
                    egui::DragValue::new(n)
                        .speed(0.01)
                        .max_decimals(9)
                        .prefix(format!("{axis} ")),
                )
                .changed();
        }
    });
    changed
}
fn rotation(ui: &mut egui::Ui, label: &str, q: &mut Quaternion) -> bool {
    ui.label(label);
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for n in [&mut q.w, &mut q.x, &mut q.y, &mut q.z] {
            changed |= ui.add(egui::DragValue::new(n).speed(0.001)).changed();
        }
    });
    if changed && let Ok(normalized) = q.normalized() {
        *q = normalized;
    }
    changed
}
fn local_frame(ui: &mut egui::Ui, f: &mut LocalFrame) -> bool {
    let a = vector(ui, "Initial mount translation (m)", &mut f.translation_m);
    rotation(ui, "Initial mount rotation W / X / Y / Z", &mut f.rotation) || a
}
