# CLAUDE.md – Instruktionen für Claude Code

## Was ist das?

FreeSynergy.Node – ein modulares, dezentrales Deployment-System auf Basis von
Podman Quadlets, gesteuert durch Ansible.

## Regeln

- Sprache in Dateien: **Englisch** (Kommentare, YAML-Keys, Variablennamen)
- Sprache im Chat: **Deutsch**
- YAML-Stil: max 160 Zeichen pro Zeile, ein Leerzeichen nach Doppelpunkt
- Jede Änderung wird in `CHANGELOG.md` dokumentiert

## Projektstruktur

```
modules/       -> Modul-Definitionen (YAML + Templates + Hooks)
hosts/         -> Host-Dateien (eine pro Server)
projects/      -> Projekt-Dateien + Branding + Sites
playbooks/     -> Ansible Playbooks + Tasks + Templates
```

## Wie Änderungen dokumentiert werden

Bei JEDER Änderung an einer Datei:

1. Öffne `CHANGELOG.md`
2. Füge einen neuen Eintrag hinzu mit:
   - Datum und wer geändert hat (z.B. "Claude Code", "Manuell", "Claude Chat")
   - Welche Dateien geändert wurden
   - Was genau geändert wurde
   - Ob es offene Probleme gibt
3. Speichere die Datei

Format:
```markdown
## [YYYY-MM-DD] – [Wer] – [Kurzbeschreibung]
### Geänderte Dateien
- `pfad/datei` – Was geändert
### Offene Probleme
- Was nicht funktioniert
### Nächster Schritt
- Was als nächstes kommt
```

## Wichtige Konventionen

### Modul-Dateien
- Pfad: `modules/{type}/{name}/{name}.yml`
- Reihenfolge der Blöcke: `module` → `vars` → `load` → `container` → `environment`
- `container.healthcheck` ist Pflicht für jedes Modul
- `container.published_ports: []` für alle außer Zentinel
- `container.networks: []` wird vom Deployer automatisch gesetzt

### Projekt-Dateien
- `{name}.project.yml` = lokales Deployment
- `{name}.{hostname}.yml` = Remote-Deployment
- `vault_` Prefix NUR für echte Secrets

### Proxy (Zentinel)
- Lebt in der Host-Datei, NICHT in der Projekt-Datei
- Statische Sites werden direkt von Zentinel ausgeliefert
- Branding-Assets unter `/branding/` erreichbar
- Landing Page unter Root-Domain erreichbar

### Healthchecks
Jedes Modul hat zwei Healthcheck-Ebenen:
1. **Quadlet** (container.healthcheck): Podman-Level, startet Container bei Failure neu
2. **Zentinel** (container.health_path): Proxy-Level, nimmt Upstream aus Rotation

## Debugging-Befehle

```bash
# Alle Container anzeigen
podman ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Quadlet-Dateien anzeigen
ls ~/.config/containers/systemd/

# Container-Logs
podman logs -f kanidm
journalctl --user -u kanidm.service

# Systemd-Status
systemctl --user status kanidm.service

# Netzwerke anzeigen
podman network ls

# Quadlet neu generieren
systemctl --user daemon-reload
systemctl --user restart kanidm.service
```

## Workflow: Änderung auf Server → zurück an Claude Chat

1. Problem auf Server gefunden
2. Claude Code: Datei fixen + CHANGELOG.md updaten
3. `CHANGELOG.md` an Claude Chat schicken
4. Claude Chat: Review + weitere Fixes + CHANGELOG updaten
5. Geändertes Archiv zurück auf Server
6. Repeat
