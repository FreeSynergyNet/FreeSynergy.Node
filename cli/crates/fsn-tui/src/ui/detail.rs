// Detail panel renderers for Dashboard center area.
//
// Design Pattern: Composite — each resource type (Project, Host, Service)
// renders its own detail view. dashboard.rs calls these via SidebarItem::render_center()
// without knowing which variant is selected.
//
// Shared helpers (run_state_color, run_state_char) live in widgets.rs — no
// duplicated match blocks here.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::ui::render_ctx::RenderCtx;

use fsn_core::health::{self, HealthCheck, HealthLevel};
use crate::app::{AppState, RunState};
use crate::ui::widgets;

// ── Project detail panel ──────────────────────────────────────────────────────

pub fn render_project_detail(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, slug: &str) {
    let Some(proj) = state.projects.iter().find(|p| p.slug == slug) else {
        f.render_stateful_widget(Paragraph::new("—"), area, &mut ParagraphState::new());
        return;
    };

    let name       = proj.config.project.meta.name.as_str();
    let domain     = proj.config.project.domain.as_str();
    let email      = proj.email();
    let install    = proj.install_dir();
    let svc_count  = proj.config.load.services.len();
    let host_count = state.hosts.len();
    let langs      = proj.config.project.languages.join(", ");

    let svc_ok  = state.services.iter().filter(|s| s.status == RunState::Running).count();
    let svc_err = state.services.iter().filter(|s| s.status == RunState::Failed).count();

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", name),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Domain:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(domain.to_string(), Style::default().fg(Color::Blue)),
        ]),
    ];
    if !email.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("E-Mail:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(email.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    if !langs.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Languages: ", Style::default().fg(Color::DarkGray)),
            Span::styled(langs, Style::default().fg(Color::White)),
        ]));
    }
    if !install.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Install:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(install.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Services:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(svc_count.to_string(), Style::default().fg(Color::White)),
        Span::styled("  (", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("● {}", svc_ok), Style::default().fg(Color::Green)),
        if svc_err > 0 {
            Span::styled(format!("  ✕ {}", svc_err), Style::default().fg(Color::Red))
        } else {
            Span::styled("", Style::default())
        },
        Span::styled(")", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Hosts:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(host_count.to_string(), Style::default().fg(Color::White)),
    ]));

    if let Some(desc) = proj.config.project.meta.description.as_deref() {
        if !desc.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray))));
        }
    }

    // ── Health status ────────────────────────────────────────────────────────
    let host_projects: Vec<&str> = state.hosts.iter()
        .filter_map(|h| h.config.host.project.as_deref())
        .collect();
    let h_status = health::check_project_with_hosts(&proj.config, &host_projects);
    push_health_lines(&mut lines, &h_status);

    f.render_stateful_widget(Paragraph::new(lines), inner, &mut ParagraphState::new());
}

// ── Host detail panel ─────────────────────────────────────────────────────────

pub fn render_host_detail(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, slug: &str) {
    let Some(host) = state.hosts.iter().find(|h| h.slug == slug) else {
        f.render_stateful_widget(Paragraph::new("—"), area, &mut ParagraphState::new());
        return;
    };

    let display = host.config.host.meta.display_name();
    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", display),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let addr     = host.config.host.addr();
    let ssh_user = &host.config.host.ssh_user;
    let ssh_port = host.config.host.ssh_port;
    let external = if host.config.host.external { "external" } else { "local" };
    let alias    = host.config.host.meta.alias.as_deref().unwrap_or("—");

    // ── Health status ────────────────────────────────────────────────────────
    let h_status = &host.config.health();
    let mut health_lines: Vec<Line> = Vec::new();
    push_health_lines(&mut health_lines, &h_status);
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Address:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(addr.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("SSH:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}@{}:{}", ssh_user, addr, ssh_port), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Alias:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(alias.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Type:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(external.to_string(), Style::default().fg(Color::White)),
        ]),
    ];
    lines.extend(health_lines);
    f.render_stateful_widget(Paragraph::new(lines), inner, &mut ParagraphState::new());
}

// ── Service detail panel ──────────────────────────────────────────────────────

pub fn render_service_detail(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, svc_name: &str) {
    let Some(proj) = state.projects.get(state.selected_project) else {
        f.render_stateful_widget(Paragraph::new("—"), area, &mut ParagraphState::new());
        return;
    };
    let Some(entry) = proj.config.load.services.get(svc_name) else {
        f.render_stateful_widget(Paragraph::new("—"), area, &mut ParagraphState::new());
        return;
    };

    let status       = state.last_podman_statuses.get(svc_name).copied().unwrap_or(RunState::Missing);
    // Single source of truth: widgets + i18n. No local RunState match needed.
    let status_color = widgets::run_state_color(status);
    let status_label = format!("{} {}", widgets::run_state_char(status),
        state.t(crate::app::run_state_i18n(status)));

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", svc_name),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let subdomain = entry.subdomain.as_deref().unwrap_or("—");
    let port      = entry.port.map(|p| p.to_string()).unwrap_or_else(|| "—".to_string());
    let env_count = entry.env.len();
    let domain    = format!("{}.{}", svc_name, proj.domain());

    let lines = vec![
        Line::from(vec![
            Span::styled("Class:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.service_class.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Project:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(proj.slug.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Domain:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(domain, Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::styled("Subdomain: ", Style::default().fg(Color::DarkGray)),
            Span::styled(subdomain.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Port:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(port, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Status:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(status_label, Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("Env-Vars:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(env_count.to_string(), Style::default().fg(Color::White)),
        ]),
    ];
    f.render_stateful_widget(Paragraph::new(lines), inner, &mut ParagraphState::new());
}

// ── Shared health helpers ──────────────────────────────────────────────────────

/// Append health issue lines to an existing lines vec.
/// Shows a separator + indicator + one line per issue.
fn push_health_lines(lines: &mut Vec<Line<'static>>, status: &health::HealthStatus) {
    if status.is_ok() { return; }
    lines.push(Line::from(""));
    let (indicator, header_color) = match status.overall {
        HealthLevel::Ok      => ("✓", Color::Green),
        HealthLevel::Warning => ("⚠", Color::Yellow),
        HealthLevel::Error   => ("✗", Color::Red),
    };
    lines.push(Line::from(vec![
        Span::styled("Health:    ", Style::default().fg(Color::DarkGray)),
        Span::styled(indicator, Style::default().fg(header_color).add_modifier(Modifier::BOLD)),
    ]));
    for issue in &status.issues {
        let (icon, color) = match issue.level {
            HealthLevel::Ok      => ("  ✓", Color::Green),
            HealthLevel::Warning => ("  ⚠", Color::Yellow),
            HealthLevel::Error   => ("  ✗", Color::Red),
        };
        // msg_key used directly as fallback; proper i18n keys can be added later
        lines.push(Line::from(vec![
            Span::styled(icon, Style::default().fg(color)),
            Span::styled(format!(" {}", issue.msg_key), Style::default().fg(color)),
        ]));
    }
}
