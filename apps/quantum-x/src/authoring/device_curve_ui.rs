//! Device controls around the existing Transition Editor canvas and precision UI.
use super::{graph_canvas, graph_regions, graph_view, session::Session};

pub(super) fn show(s: &mut Session, ui: &mut egui::Ui) {
    super::graph_gesture::update(s, ui.ctx());
    graph_canvas::route_scroll_tail(s, ui);
    graph_view::sync(s);
    let mut section = s.graph_section();
    let key = s.graph_key();
    let c = section.channel_mut(key).unwrap().clone();
    s.set_graph_transition(s.graph_transition().min(c.transitions.len() - 1));
    let index = s.graph_transition();
    ui.spacing_mut().item_spacing.y = 3.;
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{} · Custom Acceleration Profile", section.name));
        ui.label("Device-local distance m · commanded magnitude m/s²");
        let dirty = s.expansion.operations != s.editor.project().operations;
        ui.add_enabled_ui(s.graph_drag.is_none(), |ui| {
            if ui
                .add_enabled(dirty, egui::Button::new("Apply"))
                .on_hover_text("Commit operations · Ctrl+Enter")
                .clicked()
            {
                s.apply_graph();
            }
            if ui
                .add_enabled(dirty, egui::Button::new("Cancel"))
                .on_hover_text("Discard operations draft · Esc")
                .clicked()
            {
                s.cancel_graph();
            }
            if ui.button("Track transitions").clicked() {
                s.expansion.curve_device = None;
                s.graph_feedback = None;
            }
        });
    });
    if s.graph_device().is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(s.graph_drag.is_none(),|ui| {
            egui::ComboBox::from_id_salt("device selected region").selected_text(format!("Transition {} / {}",index+1,c.transitions.len())).show_ui(ui,|ui| {
                for i in 0..c.transitions.len() {if ui.selectable_label(index==i,format!("Transition {}",i+1)).clicked() {s.set_graph_transition(i);}}
            });
            if ui.button("Fit Curve").clicked() {graph_canvas::fit(s);}
            if ui.button("Fit Region").clicked() {graph_canvas::fit_region(s);}
            if ui.button("Reset View").clicked() {graph_canvas::reset(s);}
            super::panels::input_menu(ui,"Edit values",|ui| { super::graph_precision::selected(s,ui); super::graph::transition_values(s,ui,"device values"); });
            super::panels::input_menu(ui,"Transitions",|ui| {
                super::graph_precision::tools(s,ui);
                let before:f64=c.transitions[..index].iter().map(|t|t.length).sum();
                let mut boundary=before+c.transitions[index].length;
                ui.label("End boundary · device-local m");
                if ui.add_enabled_ui(index+1<c.transitions.len(),|ui| super::snap_ui::graph_number(ui,&mut boundary,"",0.01,s.workbench.snap,super::snap::GraphAxis::Boundary(qx_geometry::authoring::Domain::Distance))).inner.changed() {
                    match graph_canvas::boundary(&s.graph_section(),key,index,boundary-before) {Ok(next)=>s.accept_graph_section(next),Err(e)=>s.status=e}
                }
                ui.add(egui::DragValue::new(&mut s.split_fraction).range(0.001..=0.999).speed(0.01).prefix("Insert fraction "));
                if ui.button("Insert key (preserve shape)").clicked() {graph_regions::split(s,before+c.transitions[index].length*s.split_fraction);}
                if ui.add_enabled(index+1<c.transitions.len(),egui::Button::new("Merge with next (preserve shape)")).clicked() {
                    match qx_document::region_edit::merge(&s.graph_section(),key,index) {Ok(next)=>s.accept_graph_section(next),Err(e)=>s.status=e.to_string()}
                }
                if ui.add_enabled(c.transitions.len()>1,egui::Button::new("Remove and extend neighbor (reshape)")).clicked() {
                    match qx_document::editing::remove_transition(&s.graph_section(),key,c.transitions[index].id) {Ok(next)=>{s.accept_graph_section(next);s.set_graph_transition(index.saturating_sub(1));},Err(e)=>s.status=e.to_string()}
                }
                for (label, other) in [("Transition earlier",index.checked_sub(1)), ("Transition later",(index+1<c.transitions.len()).then_some(index+1))] {
                    if ui.add_enabled(other.is_some(),egui::Button::new(label)).clicked() {
                        match qx_document::region_edit::reorder(&s.graph_section(),key,index,other.unwrap()) { Ok(next)=>{s.accept_graph_section(next);s.set_graph_transition(other.unwrap());},Err(e)=>s.status=e.to_string() }
                    }
                }
            });
            super::panels::input_menu(ui,"View range",|ui| {
                for (i,label) in ["X from m","X to m","Y from m/s²","Y to m/s²"].iter().enumerate() {
                    ui.horizontal(|ui| {ui.label(*label);ui.add(egui::DragValue::new(&mut s.graph_view.range_edit[i]).speed(0.001).max_decimals(12));});
                }
                if ui.button("Apply range").clicked() {s.graph_view.apply_range(c.length());}
                if let Some(error)=s.graph_view.error {ui.label(error);}
            });
        });
        ui.label("?").on_hover_text("Square: value / station · circle: shape / easing · bar: plateau · diamond: boundary\nRight-click: insert, merge, remove\nShift drag: fine · Ctrl+wheel: zoom · +Shift: X only · +Alt: Y only · Middle drag: pan · F: fit · Esc: cancel");
    });
    ui.horizontal_wrapped(|ui| super::snap_ui::quick(s, ui));
    // One always-visible precision row follows the exact selected endpoint pair.
    let t = &c.transitions[s.graph_transition().min(c.transitions.len() - 1)];
    let mut start = t.start;
    let mut end = t.end;
    ui.horizontal_wrapped(|ui| {
        ui.label("Start m/s²");
        if super::snap_ui::graph_number(
            ui,
            &mut start,
            "",
            0.01,
            s.workbench.snap,
            super::snap::GraphAxis::Value(qx_geometry::transition::Unit::MetresPerSecondSquared),
        )
        .changed()
        {
            endpoint(s, false, start);
        }
        ui.label("End m/s²");
        if super::snap_ui::graph_number(
            ui,
            &mut end,
            "",
            0.01,
            s.workbench.snap,
            super::snap::GraphAxis::Value(qx_geometry::transition::Unit::MetresPerSecondSquared),
        )
        .changed()
        {
            endpoint(s, true, end);
        }
        ui.small("Draft preview · 0…100 m/s² · Apply commits");
    });
    graph_canvas::show(s, ui);
}
fn endpoint(s: &mut Session, end: bool, value: f64) {
    match qx_document::region_edit::endpoint(
        &s.graph_section(),
        s.graph_key(),
        s.graph_transition(),
        end,
        value,
    ) {
        Ok(next) => s.accept_graph_section(next),
        Err(e) => s.status = e.to_string(),
    }
}
