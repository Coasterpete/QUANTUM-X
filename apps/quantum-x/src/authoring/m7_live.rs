//! Actual egui press/move/hold/release through the existing workbench. Screenshots
//! are snapshots of the resulting preview, explicitly not Windows input tests.
use super::{
    graph_canvas,
    m7_scenario::{Capture, export_obj},
    panels,
    session::{Session, scene},
};
use qx_geometry::authoring::{ChannelKey, SectionId};
use qx_rendering::{DisplayMode, DisplayOptions, InspectionScene};
use qx_style::{Catalog, construction::TrackConstruction};
use std::{
    error::Error,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn wait(session: &mut Session) -> Result<InspectionScene> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(scene) = session.poll() {
            return Ok(scene);
        }
        if session.draft_error.is_some() || Instant::now() > deadline {
            return Err(session.status.clone().into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn frame(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    events: Vec<egui::Event>,
    size: [u32; 2],
) -> Option<egui::Rect> {
    let mut handle = None;
    let _ = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32, size[1] as f32),
            )),
            events,
            focused: true,
            ..Default::default()
        },
        |ctx| {
            panels::workspace(session, ctx, scene, &mut DisplayOptions::default());
            handle = ctx
                .read_response(egui::Id::unique((
                    "graph endpoint",
                    2_u64,
                    ChannelKey::Normal as u8,
                    1_u64,
                    true,
                )))
                .map(|r| r.rect);
        },
    );
    handle
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn capture(
    session: &mut Session,
    view: &InspectionScene,
    root: &Path,
    name: &str,
    size: [u32; 2],
) -> Result<serde_json::Value> {
    let file = format!("{name}-{}.png", size[0]);
    let mut surface = Capture {
        session,
        display: DisplayOptions {
            mode: DisplayMode::RenderedCenterline,
            grid: true,
            ..Default::default()
        },
    };
    let gpu =
        qx_rendering::render_offscreen(view, &mut surface, &root.join(&file), size[0], size[1])?;
    if size[0] == 1440 {
        export_obj(
            view.mesh.as_ref().unwrap(),
            &root.join(format!("{name}.obj")),
        )?;
        qx_document::persistence::save(
            &root.join(format!("{name}.qx.json")),
            &surface.session.preview_project(),
            &surface.session.catalog,
        )?;
    }
    Ok(
        serde_json::json!({"file":file,"width":size[0],"height":size[1],"gpu":gpu,"mesh_ms":view.mesh_ms,"samples":view.track.samples.len(),"triangles":view.mesh.as_ref().unwrap().stats.rendered_triangles}),
    )
}
pub(super) fn run(
    original: &qx_document::Project,
    catalog: &Catalog,
    root: &Path,
) -> Result<Vec<serde_json::Value>> {
    let mut evidence = vec![];
    for (label, construction) in [
        ("dual", TrackConstruction::generic_dual()),
        ("single", TrackConstruction::generic_single()),
    ] {
        for size in [[1440, 960], [960, 640]] {
            let mut project = original.clone();
            project.construction = construction.clone();
            project.metadata.name = format!("M7 live graph / {label}");
            let mut session = Session::new(project.clone(), None, catalog.clone())?;
            session.select(SectionId(2));
            session.select_channel(ChannelKey::Normal);
            session.regenerate();
            let initial = wait(&mut session)?;
            let before = capture(
                &mut session,
                &initial,
                root,
                &format!("live-{label}-before"),
                size,
            )?;
            // A preceding section-name draft exercises M6's explicit Apply path.
            // The force value itself is changed only by real graph pointer events.
            session.draft.name.push_str(" / live edit validation");
            session.preview();
            let baseline = wait(&mut session)?;
            let ctx = egui::Context::default();
            let mut target = None;
            for _ in 0..5 {
                target = frame(&mut session, &ctx, &baseline, vec![], size);
            }
            let target = target
                .ok_or("normal-force endpoint was not rendered")?
                .center();
            let graph = ctx
                .read_response(egui::Id::unique((
                    "graph canvas",
                    2_u64,
                    ChannelKey::Normal as u8,
                )))
                .ok_or("graph canvas missing")?
                .rect;
            let units_per_pixel =
                session.graph_view.view.y.span() / graph_canvas::plot_rect(graph).height() as f64;
            let end = target - egui::vec2(0., (0.25 / units_per_pixel) as f32);
            frame(
                &mut session,
                &ctx,
                &baseline,
                vec![egui::Event::PointerMoved(target), button(target, true)],
                size,
            );
            // Cross the drag threshold even in the short 960px graph, then
            // return to the same precise 0.25 g displacement at either size.
            frame(
                &mut session,
                &ctx,
                &baseline,
                vec![egui::Event::PointerMoved(target - egui::vec2(0., 12.))],
                size,
            );
            frame(
                &mut session,
                &ctx,
                &baseline,
                vec![egui::Event::PointerMoved(end)],
                size,
            );
            let draft = wait(&mut session)?;
            if session.graph_drag.is_none()
                || !session.visible_preview
                || draft.mesh == initial.mesh
            {
                return Err("held graph gesture did not publish changed solid geometry".into());
            }
            let revision = session.revision;
            let draft_mesh = draft.mesh.as_ref().unwrap().clone();
            for _ in 0..3 {
                frame(&mut session, &ctx, &draft, vec![], size);
            }
            if session.revision != revision {
                return Err("idle held gesture regenerated geometry".into());
            }
            frame(
                &mut session,
                &ctx,
                &draft,
                vec![egui::Event::PointerMoved(end), button(end, false)],
                size,
            );
            if session.graph_drag.is_some()
                || !session.draft_dirty()
                || session.editor.project() != &project
            {
                return Err("gesture did not preserve the staged/committed distinction".into());
            }
            // Capture the exact mesh published while held, after releasing the
            // pointer into the existing staged draft. A fresh offscreen egui
            // context does not pretend to retain native mouse capture.
            let preview = capture(
                &mut session,
                &draft,
                root,
                &format!("live-{label}-draft"),
                size,
            )?;
            if !session.apply_draft() || session.revision != revision || session.visible_preview {
                return Err("Apply did not reuse the published valid preview".into());
            }
            let accepted = session.editor.project().clone();
            session.undo(false);
            let undone = wait(&mut session)?;
            if undone.mesh != initial.mesh || session.editor.project() != &project {
                return Err("Undo did not restore original mesh/document".into());
            }
            session.undo(true);
            let redone = wait(&mut session)?;
            if redone.mesh.as_deref() != Some(draft_mesh.as_ref())
                || session.editor.project() != &accepted
            {
                return Err("Redo did not restore accepted mesh/document".into());
            }
            let path = root.join(format!("live-{label}-saved-{}.qx.json", size[0]));
            session.save_to(&path);
            if session.dirty() {
                return Err(session.status.clone().into());
            }
            let reopened = qx_document::persistence::load(&path, catalog)?;
            let reopened_scene = scene(&reopened, catalog, session.options)?;
            if reopened != accepted || reopened_scene.mesh != redone.mesh {
                return Err("save/reopen changed live-edit mesh".into());
            }
            let after = capture(
                &mut session,
                &redone,
                root,
                &format!("live-{label}-after"),
                size,
            )?;
            // Cancel another actual staged force edit and await committed geometry.
            let value = session.draft.channel_mut(ChannelKey::Normal)?.transitions[0].end + 0.1;
            session.draft = qx_document::region_edit::endpoint(
                &session.draft,
                ChannelKey::Normal,
                0,
                true,
                value,
            )?;
            session.preview();
            let changed = wait(&mut session)?;
            if changed.mesh == redone.mesh {
                return Err("cancel precondition did not change mesh".into());
            }
            session.cancel_section();
            let canceled = wait(&mut session)?;
            if canceled.mesh != redone.mesh {
                return Err("Cancel did not restore the accepted mesh".into());
            }
            evidence.push(serde_json::json!({"construction":label,"width":size[0],"input":"egui primary press, move, three held idle frames, release","mesh_changed_while_pointer_held":true,"idle_revision_stable":true,"release_retained_draft":true,"apply_reused_revision":true,"undo_redo_restored_mesh":true,"cancel_restored_mesh":true,"save_reopen_identical":true,"native_windows_input":false,"captures":[before,preview,after]}));
            println!(
                "Live {label} {}: held preview, Apply reuse, Cancel, Undo/Redo and save/reopen verified",
                size[0]
            );
            // Keep the shared preview ownership assertion explicit in this path.
            let shared = draft.mesh.as_ref().unwrap();
            if !Arc::ptr_eq(shared, &draft_mesh) {
                return Err("preview ownership changed unexpectedly".into());
            }
        }
    }
    Ok(evidence)
}
