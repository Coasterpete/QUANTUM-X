//! Compact controls share the global workbench preferences. Explicit numeric
//! text entry remains exact; snapping applies to placement drags only.
use super::{
    session::Session,
    snap::{Settings, Space},
};
use qx_geometry::math::Vec3;

pub(super) fn master(s: &mut Session, ui: &mut egui::Ui) {
    let enabled = &mut s.workbench.snap.enabled;
    if ui.selectable_label(*enabled, if *enabled { "Snap: On" } else { "Snap: Off" })
        .on_hover_text("Global snapping · Alt while dragging temporarily bypasses snapping. Preferences → Snap Settings.")
        .clicked() { *enabled = !*enabled; }
}
pub(super) fn toolbar(s: &mut Session, ui: &mut egui::Ui, graph: bool) {
    master(s, ui);
    if graph {
        ui.checkbox(&mut s.workbench.snap.graph.enabled, "Graph snap");
        ui.checkbox(&mut s.workbench.snap.graph.horizontal, "X")
            .on_hover_text("Snap station / time / normalized boundaries");
        ui.checkbox(&mut s.workbench.snap.graph.vertical, "Y")
            .on_hover_text("Snap values in the active curve's units");
    } else {
        ui.checkbox(&mut s.workbench.snap.spatial.enabled, "Spatial snap");
    }
    if ui.button("Snap Settings…").clicked() {
        s.snap_dialog = true;
    }
    if ui.input(|i| i.modifiers.alt) && s.workbench.snap.enabled {
        ui.small("Alt: bypass");
    }
}
pub(super) fn quick(s: &mut Session, ui: &mut egui::Ui) {
    master(s, ui);
    if super::icons::button(
        ui,
        super::icons::Icon::Preferences,
        "",
        true,
        "Snap Settings · independent graph X/Y and spatial controls",
    )
    .clicked()
    {
        s.snap_dialog = true;
    }
}
pub(super) fn dialog(s: &mut Session, ctx: &egui::Context) {
    let mut open = s.snap_dialog;
    egui::Window::new("Snap Settings").id(egui::Id::unique("snap preferences"))
        .open(&mut open).resizable(true).default_width(410.).show(ctx, |ui| {
            ui.small("Global user preferences · saved across restarts · projects stay unchanged");
            ui.checkbox(&mut s.workbench.snap.enabled, "Enable snapping");
            ui.small("Alt: temporary bypass · Shift: fine drag / fine distance increment. Typed numeric values remain exact.");
            egui::ScrollArea::vertical().max_height(520.).show(ui, |ui| {
                let g = &mut s.workbench.snap.graph;
                egui::CollapsingHeader::new("Graph snapping").default_open(true).show(ui, |ui| {
                    ui.checkbox(&mut g.enabled, "Enable graph snapping");
                    ui.horizontal(|ui| { ui.checkbox(&mut g.horizontal, "Horizontal"); ui.checkbox(&mut g.vertical, "Vertical"); });
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut g.points, "Control points");
                        ui.checkbox(&mut g.handles, "Shape handles");
                        ui.checkbox(&mut g.boundaries, "Transition boundaries");
                    });
                    ui.checkbox(&mut g.nearby_keys, "Attract to nearby existing keys");
                    ui.small("Boundary targets include other curves in this Region. Handles retain their existing constrained axes.");
                    egui::Grid::new("graph snap increments").show(ui, |ui| {
                        for (label, value) in [
                            ("Station m", &mut g.station_m), ("Fine station m", &mut g.fine_station_m),
                            ("Time s", &mut g.time_s), ("Normalized domain", &mut g.normalized),
                            ("Force g", &mut g.force_g), ("Rate rad/m", &mut g.rate_rad_per_m),
                            ("Rate rad/s", &mut g.rate_rad_per_s), ("Rate rad/u", &mut g.rate_rad_per_u),
                            ("Acceleration m/s²", &mut g.acceleration_mps2),
                        ] { increment(ui, label, value, 1e9); }
                        increment(ui, "Key attraction px", &mut g.key_radius_points, 64.);
                    });
                });
                let p = &mut s.workbench.snap.spatial;
                egui::CollapsingHeader::new("Spatial snapping").default_open(true).show(ui, |ui| {
                    ui.checkbox(&mut p.enabled, "Enable spatial snapping");
                    ui.checkbox(&mut p.world_grid, "World grid (world XYZ / foundation elevation)");
                    ui.checkbox(&mut p.distance, "Distance increments (Region-local / track-local)");
                    ui.checkbox(&mut p.angles, "Angle increments");
                    ui.checkbox(&mut p.track_station, "Track station attachments");
                    ui.checkbox(&mut p.nearby_points, "Align edited axis to nearby geometric points / nodes");
                    egui::Grid::new("spatial snap increments").show(ui, |ui| {
                        for (label, value, max) in [
                            ("World grid m", &mut p.grid_m, 1e9), ("Distance m", &mut p.distance_m, 1e9),
                            ("Fine distance m", &mut p.fine_distance_m, 1e9), ("Angle degrees", &mut p.angle_degrees, 360.),
                            ("Track station m", &mut p.station_m, 1e9), ("Nearby distance m", &mut p.nearby_radius_m, 1e9),
                        ] { increment(ui, label, value, max); }
                    });
                    ui.small("Uses the existing numeric placement controls; no new viewport transform tool. Only the axis being dragged is snapped.");
                });
            });
        });
    s.snap_dialog = open;
}
fn increment(ui: &mut egui::Ui, label: &str, value: &mut f64, max: f64) {
    ui.label(label);
    ui.add(
        egui::DragValue::new(value)
            .speed(0.01)
            .range(if label.ends_with("px") { 1. } else { 1e-10 }..=max)
            .max_decimals(10),
    );
    ui.end_row();
}

pub(super) fn number(
    ui: &mut egui::Ui,
    value: &mut f64,
    label: &str,
    speed: f64,
    snap: Settings,
    space: Space,
    keys: &[f64],
) -> egui::Response {
    let before = *value;
    let mut widget = egui::DragValue::new(value).speed(speed).max_decimals(12);
    if !label.is_empty() {
        widget = widget.prefix(label);
    }
    let mut response = ui.add(widget);
    if response.dragged() || response.drag_stopped() {
        // egui keeps its full-precision accumulator under the DragValue ID.
        // Reading it also lets Alt restore the raw value without another move.
        let raw = ui
            .data(|d| d.get_temp::<f64>(response.id))
            .unwrap_or(*value);
        *value = snap.spatial_value(
            raw,
            space,
            keys,
            ui.input(|i| i.modifiers.shift),
            ui.input(|i| i.modifiers.alt),
        );
        if *value != before {
            response.mark_changed();
        }
        response = response.on_hover_text(format!(
            "{label}{:.12} · {} · Alt bypasses snap",
            *value,
            if snap.spatial_active(ui.input(|i| i.modifiers.alt)) {
                "Snapping active"
            } else {
                "Snap off / bypassed"
            }
        ));
    }
    response
}
pub(super) fn vector(
    ui: &mut egui::Ui,
    v: &mut Vec3,
    snap: Settings,
    space: Space,
    points: &[Vec3],
) {
    for (axis, value, label) in [
        (0, &mut v.x, "X m "),
        (1, &mut v.y, "Y m "),
        (2, &mut v.z, "Z m "),
    ] {
        let keys: Vec<_> = points.iter().map(|p| [p.x, p.y, p.z][axis]).collect();
        number(ui, value, label, 0.05, snap, space, &keys);
    }
}

pub(super) fn graph_number(
    ui: &mut egui::Ui,
    value: &mut f64,
    label: &str,
    speed: f64,
    snap: Settings,
    axis: super::snap::GraphAxis,
) -> egui::Response {
    let before = *value;
    let mut widget = egui::DragValue::new(value).speed(speed).max_decimals(12);
    if !label.is_empty() {
        widget = widget.prefix(label);
    }
    let mut response = ui.add(widget);
    if response.dragged() || response.drag_stopped() {
        let raw = ui
            .data(|d| d.get_temp::<f64>(response.id))
            .unwrap_or(*value);
        let step = snap.graph_step(
            axis,
            ui.input(|i| i.modifiers.shift),
            ui.input(|i| i.modifiers.alt),
        );
        *value = super::snap::scalar(raw, step, &[], 0.);
        if *value != before {
            response.mark_changed();
        }
        response = response.on_hover_text(format!(
            "{label}{:.12} · {} · Alt bypasses snap",
            *value,
            if step.is_some() {
                "Snapping active"
            } else {
                "Snap off / bypassed"
            }
        ));
    }
    response
}
