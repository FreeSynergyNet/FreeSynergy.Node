// rat-salsa event loop wiring.
//
// Design Pattern: Facade — thin wrappers that delegate to the
// existing events/render modules.  Separated from app/mod.rs
// so that AppState (pure data + helpers) and the event-loop
// plumbing (timers, channels, rat-salsa callbacks) live in
// different compilation units.
//
// Public items re-exported by app/mod.rs:
//   AppEvent, AppGlobal, run_salsa

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rat_salsa::{Control, RunConfig, SalsaAppContext, SalsaContext, run_tui};
use rat_salsa::poll::{PollCrossterm, PollTimers};
use rat_salsa::timer::{TimeOut, TimerDef};

use fsn_core::store::StoreEntry;

use super::{AppState, DeployMsg, NotifKind, RunState};

// ── AppEvent ──────────────────────────────────────────────────────────────────

/// Events dispatched by rat-salsa to the application.
#[derive(Debug)]
pub enum AppEvent {
    /// Raw crossterm event (keyboard, resize, etc.).
    Crossterm(crossterm::event::Event),
    /// Periodic tick — polls background channels and refreshes sysinfo.
    Tick(TimeOut),
}

impl From<crossterm::event::Event> for AppEvent {
    fn from(e: crossterm::event::Event) -> Self { AppEvent::Crossterm(e) }
}

impl From<TimeOut> for AppEvent {
    fn from(t: TimeOut) -> Self { AppEvent::Tick(t) }
}

// ── AppGlobal ─────────────────────────────────────────────────────────────────

/// Global state accessible from all rat-salsa callbacks (init/render/event/error).
pub struct AppGlobal {
    ctx:  SalsaAppContext<AppEvent, anyhow::Error>,
    /// Root path of the FSN workspace — forwarded to events::handle.
    pub root: PathBuf,
}

impl SalsaContext<AppEvent, anyhow::Error> for AppGlobal {
    fn set_salsa_ctx(&mut self, app_ctx: SalsaAppContext<AppEvent, anyhow::Error>) {
        self.ctx = app_ctx;
    }
    fn salsa_ctx(&self) -> &SalsaAppContext<AppEvent, anyhow::Error> { &self.ctx }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Entry point for the rat-salsa event loop. Called from `lib.rs::run()`.
/// Terminal setup (raw mode, alternate screen, mouse capture) is handled by rat-salsa.
pub fn run_salsa(root: PathBuf, state: &mut AppState) -> anyhow::Result<()> {
    let mut global = AppGlobal { ctx: Default::default(), root };
    run_tui(
        fsn_init,
        fsn_render,
        fsn_event,
        fsn_error,
        &mut global,
        state,
        RunConfig::default()?.poll(PollCrossterm).poll(PollTimers::new()),
    )?;
    Ok(())
}

// ── rat-salsa callbacks ───────────────────────────────────────────────────────

fn fsn_init(state: &mut AppState, ctx: &mut AppGlobal) -> anyhow::Result<()> {
    // Repeating 250 ms tick — used to drain background mpsc channels.
    ctx.add_timer(TimerDef::new().repeat_forever().timer(Duration::from_millis(250)));
    // Force an immediate render so the screen isn't blank before the first event.
    let _ = state;
    Ok(())
}

fn fsn_render(
    area:  ratatui::layout::Rect,
    buf:   &mut ratatui::buffer::Buffer,
    state: &mut AppState,
    _ctx:  &mut AppGlobal,
) -> anyhow::Result<()> {
    let mut rctx = crate::ui::render_ctx::RenderCtx::new(area, buf);
    crate::ui::render(&mut rctx, state);
    Ok(())
}

fn fsn_event(
    event: &AppEvent,
    state: &mut AppState,
    ctx:   &mut AppGlobal,
) -> anyhow::Result<Control<AppEvent>> {
    match event {
        AppEvent::Crossterm(e) => {
            match e {
                crossterm::event::Event::Key(key) => {
                    crate::events::handle(*key, state, ctx.root.as_path())?;
                }
                crossterm::event::Event::Mouse(mouse) => {
                    crate::mouse::handle_mouse(*mouse, state, ctx.root.as_path())?;
                }
                _ => {}
            }
            if state.should_quit { return Ok(Control::Quit); }
            Ok(Control::Changed)
        }

        AppEvent::Tick(_) => {
            // Drain reconciler channel — collect first to avoid simultaneous borrow.
            let reconcile_msgs: Vec<HashMap<String, RunState>> = state.reconcile_rx
                .as_ref()
                .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
                .unwrap_or_default();
            for statuses in reconcile_msgs { state.apply_podman_status(statuses); }

            // Drain deploy channel.
            let deploy_msgs: Vec<DeployMsg> = state.deploy_rx
                .as_ref()
                .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
                .unwrap_or_default();
            for msg in deploy_msgs { state.apply_deploy_msg(msg); }

            // Drain store fetcher channel (one-shot).
            let store_result: Option<Vec<StoreEntry>> = state.store_rx
                .as_ref()
                .and_then(|rx| rx.try_recv().ok());
            if let Some(entries) = store_result {
                let count = entries.len();
                state.store_entries = entries;
                state.store_rx = None;
                if count > 0 {
                    state.push_notif(NotifKind::Info, format!("Store: {count} modules loaded"));
                }
            }

            // Drain language downloader channel (one-shot).
            let lang_result: Option<Result<String, String>> = state.lang_download_rx
                .as_ref()
                .and_then(|rx| rx.try_recv().ok());
            if let Some(result) = lang_result {
                state.lang_download_rx = None;
                match result {
                    Ok(code) => {
                        state.reload_langs();
                        state.push_notif(NotifKind::Success,
                            format!("Language '{}' downloaded", code.to_uppercase()));
                    }
                    Err(msg) => {
                        state.push_notif(NotifKind::Info,
                            format!("Language download failed: {msg}"));
                    }
                }
            }

            // Drain language index fetcher (one-shot).
            let lang_index: Option<Result<Vec<crate::StoreLangEntry>, String>> = state.store_langs_rx
                .as_ref()
                .and_then(|rx| rx.try_recv().ok());
            if let Some(result) = lang_index {
                state.store_langs_rx = None;
                match result {
                    Ok(entries) => { state.store_langs = entries; }
                    Err(msg)    => {
                        state.push_notif(NotifKind::Info,
                            format!("Store index: {msg} — configure a local path in Settings"));
                    }
                }
            }

            // Refresh sysinfo every 5 s.
            if state.last_refresh.elapsed() >= Duration::from_secs(5) {
                state.sysinfo = crate::sysinfo::SysInfo::collect();
                state.last_refresh = Instant::now();
            }

            state.expire_notifications(Duration::from_secs(4));
            state.anim.advance();
            Ok(Control::Changed)
        }
    }
}

fn fsn_error(
    err:    anyhow::Error,
    _state: &mut AppState,
    _ctx:   &mut AppGlobal,
) -> anyhow::Result<Control<AppEvent>> {
    tracing::error!("{:#}", err);
    Ok(Control::Changed)
}
