//! Both section navigators read the authoritative document and share selection/commands.
use super::{
    icons::{self, Icon},
    session::{Session, method},
};
use qx_document::command::Command;
use qx_geometry::authoring::{Domain, Section};

pub(super) fn extent(section: &Section) -> String {
    match section.domain {
        Domain::Time => format!("{:.3} s", section.extent),
        Domain::Distance => format!("{:.3} m", section.extent),
        _ => "normalized curve".into(),
    }
}
pub(super) fn card(
    ui: &mut egui::Ui,
    section: &Section,
    index: usize,
    active: bool,
    timeline: bool,
) -> egui::Response {
    card_sized(ui, section, index, active, timeline, false)
}
fn card_sized(
    ui: &mut egui::Ui,
    section: &Section,
    index: usize,
    active: bool,
    timeline: bool,
    compact: bool,
) -> egui::Response {
    let width = if timeline {
        186.
    } else {
        ui.available_width().max(120.)
    };
    let (_, rect) = ui.allocate_space(egui::vec2(
        width,
        if compact {
            24.
        } else if timeline {
            36.
        } else {
            48.
        },
    ));
    let id = egui::Id::unique(("section navigator", timeline, section.id.0));
    let response = ui.interact(rect, id, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            ui.is_enabled(),
            active,
            format!(
                "Section {}: {}. {}. {}",
                index + 1,
                section.name,
                method(section),
                extent(section)
            ),
        )
    });
    let visuals = ui.style().interact_selectable(&response, active);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect(
        rect,
        3.,
        visuals.bg_fill,
        if active || response.has_focus() {
            ui.visuals().selection.stroke
        } else {
            visuals.bg_stroke
        },
        egui::StrokeKind::Inside,
    );
    let text: egui::WidgetText =
        egui::RichText::new(format!("{:02}  {}", index + 1, section.name)).into();
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        width - 16.,
        egui::TextStyle::Body,
    );
    painter.galley(
        rect.left_top() + egui::vec2(8., if timeline { 1. } else { 6. }),
        galley,
        visuals.text_color(),
    );
    if !compact {
        painter.text(
            rect.left_top() + egui::vec2(8., if timeline { 20. } else { 28. }),
            egui::Align2::LEFT_TOP,
            format!("{} · {}", method(section), extent(section)),
            egui::FontId::monospace(11.),
            visuals.text_color(),
        );
    }
    response
}
pub(super) fn sequence(session: &mut Session, ui: &mut egui::Ui) {
    let compact = ui.available_height() < 260.;
    egui::ScrollArea::horizontal().id_salt("section timeline")
    .auto_shrink([false, true])
    .scroll_source(egui::scroll_area::ScrollSource { mouse_wheel: !ui.input(|i| i.modifiers.ctrl), ..Default::default() })
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            for (index, section) in session.editor.project().sections.clone().iter().enumerate() {
                let active = session.selected == section.id;
                let response = card_sized(ui, section, index, active, true, compact);
                if response.clicked() { session.select(section.id); }
                if response.hovered() { session.hovered = Some(section.id); }
                response.on_hover_text(format!("Section ID {}. Click to select in both the timeline and Track Workspace. Apply or Cancel any pending section draft first.", section.id.0));
            }
        });
    });
}
pub(super) fn move_selected(session: &mut Session, later: bool) {
    if session.defer_section_action(super::workflow::PendingAction::Move { later }) {
        return;
    }
    let sections = &session.editor.project().sections;
    let Some(index) = sections.iter().position(|s| s.id == session.selected) else {
        return;
    };
    let after = if later {
        let Some(next) = sections.get(index + 1) else {
            return;
        };
        Some(next.id)
    } else {
        if index == 0 {
            return;
        }
        index.checked_sub(2).map(|i| sections[i].id)
    };
    session.execute(vec![Command::MoveSection {
        id: session.selected,
        after,
    }]);
}
pub(super) fn ordering(session: &mut Session, ui: &mut egui::Ui) {
    let sections = &session.editor.project().sections;
    let index = sections
        .iter()
        .position(|s| s.id == session.selected)
        .unwrap_or(0);
    let count = sections.len();
    let ready = session.graph_drag.is_none();
    ui.horizontal_wrapped(|ui| {
        if icons::button(
            ui,
            Icon::Earlier,
            "Earlier",
            ready && index > 0,
            "Move selected section one position earlier. One undo step.",
        )
        .clicked()
        {
            move_selected(session, false);
        }
        if icons::button(
            ui,
            Icon::Later,
            "Later",
            ready && index + 1 < count,
            "Move selected section one position later. One undo step.",
        )
        .clicked()
        {
            move_selected(session, true);
        }
    });
}
