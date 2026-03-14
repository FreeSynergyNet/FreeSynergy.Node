# FreeSynergy Vision

> "Not a classic OS — dynamically rendered, federated service views.
> Every interaction is an Intent, routed to the best available provider
> (local, federated, or remote)."

---

## Core Idea

FreeSynergy is not a panel of icons that launch applications.
It is a **live graph of services** — each service exposes capabilities,
each capability answers intents.

The user does not open an app.
The user expresses a need:

```
fsn intent "show mails"
fsn intent "create task"
fsn intent "deploy project my-project"
```

FreeSynergy finds the best available provider for that intent —
local container, federated peer, or remote API — and renders the result
directly in the Desktop shell.

---

## Architecture Pillars

### 1. Intent-Routing

Every user action is an **Intent** — a structured request with a verb and
optional context.  The Intent Router consults the ServiceRole Registry to
find all services that declare a matching capability, then picks the best
one based on availability, latency, and user preference.

```
Intent("show mails")
  → ServiceRole Registry
  → [mail/stalwart@local, mail/proton@federated]
  → pick local (available, lowest latency)
  → render MailView with local data
```

Groundwork already in place: `ServiceRole` registry in `fsn-core`.
Next step: extend it with an Intent-to-Capability mapping table.

### 2. Federated Services

A FreeSynergy node does not have to own every service it uses.
Through federation (OIDC + ActivityPub + SCIM), a node can delegate
capabilities to trusted peers — a shared mail server, a team wiki,
a collaborative task board.

The user sees a unified interface regardless of where data lives.

### 3. Spatial / Card-based Desktop

The Desktop shell is not a window manager.
It is a **canvas of cards** — each card is a live service view.

Inspiration: Obsidian Canvas, Raycast command palette, Linear's
spatial layout.  Cards can be pinned, linked, stacked, and navigated
with keyboard shortcuts alone.

*Evaluation gate:* Implement after the standard window-manager layout
is stable and Intent-Routing is functional.

### 4. Plugin-Extended Rendering

Service modules (plugins) ship their own card renderers as WASM bundles.
The Desktop shell loads them at runtime — no recompilation needed.

The Core executes; the plugin returns structured data.
The renderer turns data into a card.

---

## Guiding Constraints

| Constraint | Reason |
|---|---|
| **Local-first** | Works without internet; federation is an enhancement, not a requirement |
| **No lock-in** | All data in open formats (TOML, SQLite, ActivityPub); export at any time |
| **Privacy by default** | Secrets never leave the node unless the user explicitly federates |
| **Keyboard-complete** | Every action reachable without a mouse |
| **Auditable** | Every deployment action written to `AuditLog`; surfaced in the Desktop |

---

## Milestones toward this Vision

| Milestone | Status |
|---|---|
| Podman Quadlet deployment (CLI) | done — v0.1.0 |
| ServiceHost / Supervisor | done — `fsn-container/src/supervisor.rs` |
| Intent-Routing (ServiceRole → Intent mapping) | next |
| Desktop shell (FreeSynergy.Desktop) | in progress |
| Federation (OIDC + ActivityPub) | planned — `fsn-federation` stub |
| Spatial canvas Desktop | future |
