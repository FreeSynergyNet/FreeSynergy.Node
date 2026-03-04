#!/usr/bin/env bash
# FreeSynergy.Node – Bootstrap Installer
#
# This script can be downloaded standalone and run on any fresh server.
# It will:
#   1. Check and install required tools (git, python3, ansible)
#   2. Clone the FreeSynergy.Node repo (default: official FSN repo)
#   3. Interactive setup wizard OR import an existing config file
#   4. Run the platform setup and deployment playbooks
#
# Usage (quick – uses official FSN repo):
#   bash <(curl -fsSL https://raw.githubusercontent.com/Lord-KalEl/FreeSynergy.Node/main/fsn-install.sh)
#
# Usage (verified – recommended for production):
#   curl -fsSL https://raw.githubusercontent.com/Lord-KalEl/FreeSynergy.Node/main/fsn-install.sh        -o fsn-install.sh
#   curl -fsSL https://raw.githubusercontent.com/Lord-KalEl/FreeSynergy.Node/main/fsn-install.sh.sha256 -o fsn-install.sh.sha256
#   sha256sum -c fsn-install.sh.sha256 && bash fsn-install.sh
#
# Usage (with an existing project config):
#   ./fsn-install.sh --config /path/to/myproject.project.yml
#
# Usage (advanced / non-interactive):
#   ./fsn-install.sh \
#     --repo    https://github.com/yourfork/FreeSynergy.Node \
#     --target  /opt/fsn \
#     --project projects/MyProject/my.project.yml \
#     --skip-deploy
#
# Available flags:
#   --repo URL        GitHub URL of the FreeSynergy.Node repository (default: official FSN repo)
#   --target DIR      Local directory to clone into (default: ~/FreeSynergy.Node)
#   --project FILE    Path to an already-placed project.yml (skips wizard)
#   --config FILE     Import an external project.yml – copies it to projects/, skips wizard
#   --skip-setup      Skip setup-server.yml (e.g. server already prepared)
#   --skip-deploy     Fetch + install project only, skip final deploy
#   --help            Show this help
#
# NOTE: API tokens, DNS credentials, and passwords are NEVER accepted as
#       command-line arguments – they would appear in shell history.
#       They are always collected interactively via read -s.

set -euo pipefail

# --- Canonical repository (update when forking) ---
FSN_DEFAULT_REPO="https://github.com/Lord-KalEl/FreeSynergy.Node"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${GREEN}[FSN]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }
info() { echo -e "${CYAN}[INFO]${NC} $*"; }
step() { echo -e "\n${BOLD}━━ $* ${NC}"; }
ask()  { printf "${CYAN}[?]${NC} $* "; }

# --- Print checksum info for integrity verification ---
# When run as a file:  shows the actual SHA256 of this script.
# When piped (bash <(curl ...)): shows instructions for verified install.
print_checksum_info() {
    echo -e "${BOLD}━━ Script Integrity ${NC}"
    if [ -f "$0" ]; then
        local checksum
        checksum=$(sha256sum "$0" | cut -d' ' -f1)
        info "SHA256: ${checksum}"
        info "Compare against: ${FSN_DEFAULT_REPO}/releases"
    else
        info "Quick install (unverified). For production, verify first:"
        info "  curl -fsSL ${FSN_DEFAULT_REPO}/raw/main/fsn-install.sh        -o fsn-install.sh"
        info "  curl -fsSL ${FSN_DEFAULT_REPO}/raw/main/fsn-install.sh.sha256 -o fsn-install.sh.sha256"
        info "  sha256sum -c fsn-install.sh.sha256 && bash fsn-install.sh"
    fi
    echo
}

# --- Detect OS and package manager ---
detect_os() {
    if [ ! -f /etc/os-release ]; then
        err "Cannot detect OS – /etc/os-release not found"
        exit 1
    fi
    . /etc/os-release
    OS_ID="${ID}"
    OS_FAMILY="${ID_LIKE:-${ID}}"

    case "${OS_ID}" in
        debian|ubuntu)          PKG_MGR="apt";;
        fedora)                 PKG_MGR="dnf";;
        centos|rhel|rocky|alma) PKG_MGR="dnf";;
        arch|manjaro)           PKG_MGR="pacman";;
        *)
            case "${OS_FAMILY}" in
                *debian*|*ubuntu*)  PKG_MGR="apt";;
                *fedora*|*rhel*)    PKG_MGR="dnf";;
                *arch*)             PKG_MGR="pacman";;
                *)
                    warn "Unknown OS: ${OS_ID} – defaulting to apt"
                    PKG_MGR="apt"
                    ;;
            esac
            ;;
    esac
    log "OS: ${OS_ID} (package manager: ${PKG_MGR})"
}

install_pkg() {
    local pkg="$1"
    log "Installing ${pkg}..."
    case "${PKG_MGR}" in
        apt)    sudo apt-get update -qq && sudo apt-get install -y -qq "${pkg}";;
        dnf)    sudo dnf install -y -q "${pkg}";;
        pacman) sudo pacman -S --noconfirm "${pkg}";;
        *)      err "Unsupported package manager: ${PKG_MGR}"; exit 1;;
    esac
}

# --- Dependency checks ---
check_python() {
    step "Checking Python 3"
    if command -v python3 &>/dev/null; then
        log "Python3 found: $(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
    else
        warn "Python3 not found – installing..."
        install_pkg python3
    fi
}

check_git() {
    step "Checking Git"
    if command -v git &>/dev/null; then
        log "Git found: $(git --version)"
    else
        warn "Git not found – installing..."
        install_pkg git
    fi
}

check_ansible() {
    step "Checking Ansible"
    if command -v ansible-playbook &>/dev/null; then
        log "Ansible found: $(ansible --version | head -1)"
    else
        warn "Ansible not found – installing..."
        if command -v pip3 &>/dev/null; then
            log "Installing via pip3..."
            pip3 install --user ansible
            export PATH="${HOME}/.local/bin:${PATH}"
        else
            install_pkg ansible
        fi
    fi
}

# --- Clone or update the platform repo ---
fetch_platform() {
    step "Fetching FreeSynergy.Node"

    if [ "${FSN_REPO}" = "${FSN_DEFAULT_REPO}" ]; then
        info "Using official FSN repo: ${FSN_REPO}"
        info "  (use --repo YOUR_FORK_URL to install a custom fork)"
    else
        info "Using custom repo: ${FSN_REPO}"
    fi

    if [ -z "${FSN_TARGET:-}" ]; then
        FSN_TARGET="${HOME}/FreeSynergy.Node"
        info "Install target: ${FSN_TARGET}  (override with --target)"
    fi

    if [ -d "${FSN_TARGET}/.git" ]; then
        log "Repo found at ${FSN_TARGET} – pulling latest..."
        git -C "${FSN_TARGET}" pull --ff-only
    else
        log "Cloning ${FSN_REPO} → ${FSN_TARGET} ..."
        git clone "${FSN_REPO}" "${FSN_TARGET}"
    fi

    FSN_ROOT="${FSN_TARGET}"
    log "Platform ready at: ${FSN_ROOT}"
}

# ── Project Setup Wizard ───────────────────────────────────────────────────────

# List all available module classes from the modules/ directory tree.
# Output: one "type/name" per line, sorted.
list_available_modules() {
    local mod_dir="${FSN_ROOT}/modules"
    [ -d "${mod_dir}" ] || return
    find "${mod_dir}" -mindepth 2 -maxdepth 2 -type d \
        | sed "s|${mod_dir}/||" \
        | sort
}

# Ask user to choose a DNS provider and collect the API token.
# Sets: DNS_PROVIDER, DNS_TOKEN
select_dns_provider() {
    step "DNS Provider"
    info "Which DNS provider manages your domain?"
    info "  1) Hetzner DNS"
    info "  2) Cloudflare"
    ask "Choose [1/2, default: 1]:"
    read -r _dns_choice

    case "${_dns_choice}" in
        2) DNS_PROVIDER="cloudflare" ;;
        *) DNS_PROVIDER="hetzner" ;;
    esac
    log "DNS provider: ${DNS_PROVIDER}"

    local token_label
    case "${DNS_PROVIDER}" in
        hetzner)    token_label="Hetzner DNS API Token" ;;
        cloudflare) token_label="Cloudflare API Token"  ;;
    esac

    ask "${token_label} (Enter to skip):"
    read -rs DNS_TOKEN; echo
    [ -z "${DNS_TOKEN}" ] && warn "No token entered – DNS automation will be disabled."
}

# Ask user to choose an ACME / SSL certificate provider.
# Sets: ACME_PROVIDER
select_acme_provider() {
    step "SSL Certificates (ACME)"
    info "Which provider should issue SSL certificates?"
    info "  1) Let's Encrypt – free, public CA  [default]"
    info "  2) Smallstep CA  – self-hosted CA"
    ask "Choose [1/2, default: 1]:"
    read -r _acme_choice

    case "${_acme_choice}" in
        2) ACME_PROVIDER="smallstep-ca" ;;
        *) ACME_PROVIDER="letsencrypt" ;;
    esac
    log "ACME provider: ${ACME_PROVIDER}"
}

# Display all modules and let the user pick which ones to install.
# Sets: SELECTED_MODULES (array)
select_modules() {
    step "Module Selection"

    local modules=()
    while IFS= read -r m; do
        modules+=("${m}")
    done < <(list_available_modules)

    if [ ${#modules[@]} -eq 0 ]; then
        warn "No modules found in ${FSN_ROOT}/modules/ – skipping selection."
        return
    fi

    info "Available modules (enter numbers or 'all'):"
    echo ""
    for i in "${!modules[@]}"; do
        printf "  ${CYAN}%2d)${NC} %s\n" "$((i+1))" "${modules[$i]}"
    done
    echo ""
    info "Examples:  '1 3 5'  or  'all'"
    ask "Select modules:"
    read -r _selection

    SELECTED_MODULES=()
    if [[ "${_selection}" == "all" ]]; then
        SELECTED_MODULES=("${modules[@]}")
    else
        for num in ${_selection}; do
            local idx=$((num - 1))
            if [[ ${idx} -ge 0 && ${idx} -lt ${#modules[@]} ]]; then
                SELECTED_MODULES+=("${modules[idx]}")
            else
                warn "Ignoring invalid number: ${num}"
            fi
        done
    fi

    if [ ${#SELECTED_MODULES[@]} -eq 0 ]; then
        warn "No modules selected. You can add them manually to the project.yml later."
    else
        log "Selected ${#SELECTED_MODULES[@]} module(s): ${SELECTED_MODULES[*]}"
    fi
}

# Try to auto-detect the server's primary IP address.
detect_server_ip() {
    ip route get 1.1.1.1 2>/dev/null \
        | awk '{for(i=1;i<=NF;i++) if($i=="src") {print $(i+1); exit}}'
}

# Write the project.yml to projects/PROJECT_NAME/PROJECT_NAME.project.yml
# Sets: FSN_PROJECT
generate_project_yml() {
    local project_dir="${FSN_ROOT}/projects/${PROJECT_NAME}"
    local project_file="${project_dir}/${PROJECT_NAME}.project.yml"
    mkdir -p "${project_dir}"

    {
        echo "---"
        echo "# FreeSynergy.Node project file – generated by fsn-install.sh"
        echo "# Edit this file to add/remove modules or change settings."
        echo ""
        echo "project:"
        echo "  name: \"${PROJECT_NAME}\""
        echo "  domain: \"${PROJECT_DOMAIN}\""
        echo ""
        echo "load:"
        echo "  modules:"
        if [ ${#SELECTED_MODULES[@]} -gt 0 ]; then
            for module_class in "${SELECTED_MODULES[@]}"; do
                local instance="${module_class##*/}"   # auth/kanidm → kanidm
                echo "    ${instance}:"
                echo "      module_class: \"${module_class}\""
            done
        else
            echo "    # No modules selected. Add entries like:"
            echo "    # kanidm:"
            echo "    #   module_class: \"auth/kanidm\""
        fi
    } > "${project_file}"

    FSN_PROJECT="${project_file}"
    log "Project file: ${project_file}"
}

# Write a host skeleton to hosts/HOSTNAME.host.yml (skips if already exists).
generate_host_yml() {
    local hostname
    hostname=$(hostname -s 2>/dev/null || echo "server1")
    local host_file="${FSN_ROOT}/hosts/${hostname}.host.yml"

    if [ -f "${host_file}" ]; then
        log "Host file already exists: ${host_file} – skipping generation."
        return
    fi

    {
        echo "---"
        echo "# FreeSynergy.Node host file – generated by fsn-install.sh"
        echo "host:"
        echo "  name: \"${hostname}\""
        echo "  ip: \"${SERVER_IP:-}\""
        echo "  ipv6: \"\"                  # optional, leave empty if not available"
        echo "  external: false"
        echo ""
        echo "  proxy:"
        echo "    zentinel:"
        echo "      module_class: \"proxy/zentinel\""
        echo "      load:"
        echo "        plugins:"
        echo "          dns: \"${DNS_PROVIDER}\""
        echo "          acme: \"${ACME_PROVIDER}\""
    } > "${host_file}"

    log "Host file: ${host_file}"
}

# Show a summary of the wizard choices and ask for confirmation.
show_setup_summary() {
    echo ""
    echo -e "${BOLD}━━ Setup Summary ${NC}"
    info "Project:  ${PROJECT_NAME}"
    info "Domain:   ${PROJECT_DOMAIN}"
    info "Server:   ${SERVER_IP:-(not detected)}"
    info "DNS:      ${DNS_PROVIDER}"
    info "ACME:     ${ACME_PROVIDER}"
    if [ ${#SELECTED_MODULES[@]} -gt 0 ]; then
        info "Modules (${#SELECTED_MODULES[@]}):"
        for m in "${SELECTED_MODULES[@]}"; do
            info "  · ${m}"
        done
    else
        info "Modules:  (none selected)"
    fi
    echo ""
    ask "Proceed with these settings? [Y/n]:"
    read -r _confirm
    [[ "${_confirm,,}" == "n" ]] && { info "Aborted by user."; exit 0; }
}

# Full interactive setup wizard.
# Called when neither --project nor --config is given.
setup_project_interactive() {
    step "Project Setup Wizard"
    info "Answer a few questions to configure your deployment."
    info "Press Ctrl+C at any time to abort."
    echo ""

    # Project name
    ask "Project name (e.g. MyProject):"
    read -r PROJECT_NAME
    [ -z "${PROJECT_NAME}" ] && { err "Project name is required."; exit 1; }

    # Domain
    ask "Domain (e.g. example.com):"
    read -r PROJECT_DOMAIN
    [ -z "${PROJECT_DOMAIN}" ] && { err "Domain is required."; exit 1; }

    # Server IP
    local detected_ip
    detected_ip=$(detect_server_ip || true)
    if [ -n "${detected_ip}" ]; then
        info "Detected server IP: ${detected_ip}"
        ask "Server IP [${detected_ip}]:"
        read -r _ip_input
        SERVER_IP="${_ip_input:-${detected_ip}}"
    else
        ask "Server IP address:"
        read -r SERVER_IP
    fi

    select_dns_provider
    select_acme_provider
    select_modules
    show_setup_summary
    generate_project_yml
    generate_host_yml
}

# Import an external project.yml via --config FILE.
# Copies the file to projects/<name>/<name>.project.yml, sets FSN_PROJECT.
import_config() {
    local src_file="$1"
    step "Importing Project Config"

    if [ ! -f "${src_file}" ]; then
        err "Config file not found: ${src_file}"
        exit 1
    fi

    # Extract project name (simple grep – no yq dependency needed)
    local project_name
    project_name=$(grep -A3 '^project:' "${src_file}" \
        | grep 'name:' | head -1 \
        | awk '{print $2}' | tr -d '"'"'")

    if [ -z "${project_name}" ]; then
        err "Could not read 'project.name' from: ${src_file}"
        err "The file must contain:"
        err "  project:"
        err "    name: \"YourProjectName\""
        exit 1
    fi

    local dest_dir="${FSN_ROOT}/projects/${project_name}"
    local dest_file="${dest_dir}/${project_name}.project.yml"
    mkdir -p "${dest_dir}"
    cp "${src_file}" "${dest_file}"

    FSN_PROJECT="${dest_file}"
    log "Config imported to: ${dest_file}"
    log "Project: ${project_name}"
}

# ── Secrets ────────────────────────────────────────────────────────────────────

# Collect secrets interactively and write to hosts/secrets.yml.
# Tokens and passwords are NEVER accepted as CLI args (shell history risk).
# If DNS_TOKEN was already collected in the wizard, it is reused here.
collect_secrets() {
    step "Collecting Secrets"

    local secrets_file="${FSN_ROOT}/hosts/secrets.yml"

    if [ -f "${secrets_file}" ]; then
        log "Secrets file already exists: ${secrets_file}"
        ask "Re-enter secrets? [y/N]"
        read -r _reenter
        [[ "${_reenter,,}" != "y" ]] && return
    fi

    info "Secrets are stored in: ${secrets_file}"
    info "  – git-ignored, chmod 600, never shown in shell history"
    echo ""

    local tmp_secrets
    tmp_secrets=$(mktemp)
    {
        echo "---"
        echo "# FreeSynergy.Node Secrets – generated by fsn-install.sh"
        echo "# Do NOT commit this file. It is git-ignored."
    } > "${tmp_secrets}"

    # DNS token: use from wizard if already collected, otherwise ask
    if [ -n "${DNS_TOKEN:-}" ]; then
        case "${DNS_PROVIDER:-hetzner}" in
            hetzner)    echo "vault_hetzner_dns_token: \"${DNS_TOKEN}\"" >> "${tmp_secrets}" ;;
            cloudflare) echo "vault_cloudflare_api_token: \"${DNS_TOKEN}\"" >> "${tmp_secrets}" ;;
        esac
        log "DNS token saved (${DNS_PROVIDER:-hetzner})."
    else
        ask "Hetzner DNS API Token (Enter to skip):"
        read -rs _hz_token; echo
        [ -n "${_hz_token}" ] && \
            echo "vault_hetzner_dns_token: \"${_hz_token}\"" >> "${tmp_secrets}"

        ask "Cloudflare API Token (Enter to skip):"
        read -rs _cf_token; echo
        [ -n "${_cf_token}" ] && \
            echo "vault_cloudflare_api_token: \"${_cf_token}\"" >> "${tmp_secrets}"
    fi

    mv "${tmp_secrets}" "${secrets_file}"
    chmod 600 "${secrets_file}"
    log "Secrets written to ${secrets_file}"
}

# ── Playbooks ──────────────────────────────────────────────────────────────────

run_playbooks() {
    step "Running Ansible Playbooks"

    local pb="${FSN_ROOT}/playbooks"
    local secrets_file="${FSN_ROOT}/hosts/secrets.yml"
    local secrets_args=()
    local project_args=()

    [ -f "${secrets_file}" ] && secrets_args=(-e "@${secrets_file}")
    [ -n "${FSN_PROJECT:-}" ] && project_args=(-e "project_config=${FSN_PROJECT}")

    if [ "${SKIP_SETUP:-false}" != "true" ]; then
        log "Step 1/4 – setup-server.yml"
        ansible-playbook "${pb}/setup-server.yml" "${secrets_args[@]}"
    else
        info "Skipping setup-server.yml (--skip-setup)"
    fi

    if [ -n "${FSN_PROJECT:-}" ]; then
        log "Step 2/4 – fetch-modules.yml"
        ansible-playbook "${pb}/fetch-modules.yml" \
            "${project_args[@]}" "${secrets_args[@]}"

        log "Step 3/4 – install-project.yml"
        ansible-playbook "${pb}/install-project.yml" \
            "${project_args[@]}" "${secrets_args[@]}"

        if [ "${SKIP_DEPLOY:-false}" != "true" ]; then
            log "Step 4/4 – deploy-stack.yml"
            ansible-playbook "${pb}/deploy-stack.yml" \
                "${project_args[@]}" "${secrets_args[@]}"
        else
            info "Skipping deploy-stack.yml (--skip-deploy)"
            info "Run manually when ready:"
            info "  ansible-playbook ${pb}/deploy-stack.yml -e project_config=${FSN_PROJECT}"
        fi
    else
        info "No project configured. Run setup wizard or use --project / --config."
        info "  ansible-playbook ${pb}/fetch-modules.yml -e project_config=<file>"
        info "  ansible-playbook ${pb}/deploy-stack.yml  -e project_config=<file>"
    fi
}

# ── Entry Point ────────────────────────────────────────────────────────────────

main() {
    echo -e "\n${BOLD}FreeSynergy.Node Installer${NC}"
    echo -e "${BOLD}══════════════════════════${NC}\n"

    print_checksum_info

    # Defaults
    FSN_REPO="${FSN_REPO:-${FSN_DEFAULT_REPO}}"
    FSN_TARGET="${FSN_TARGET:-}"
    FSN_PROJECT="${FSN_PROJECT:-}"
    FSN_CONFIG="${FSN_CONFIG:-}"
    SKIP_SETUP="false"
    SKIP_DEPLOY="false"
    # Wizard state (set by setup_project_interactive or kept empty)
    PROJECT_NAME=""
    PROJECT_DOMAIN=""
    SERVER_IP=""
    DNS_PROVIDER="hetzner"
    DNS_TOKEN=""
    ACME_PROVIDER="letsencrypt"
    SELECTED_MODULES=()

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --repo)         FSN_REPO="$2";    shift 2;;
            --target)       FSN_TARGET="$2";  shift 2;;
            --project)      FSN_PROJECT="$2"; shift 2;;
            --config)       FSN_CONFIG="$2";  shift 2;;
            --skip-setup)   SKIP_SETUP="true"; shift;;
            --skip-deploy)  SKIP_DEPLOY="true"; shift;;
            --help|-h)
                sed -n '/^# Usage/,/^[^#]/p' "$0" | grep '^#' | sed 's/^# \?//'
                exit 0
                ;;
            *)
                err "Unknown option: $1"
                err ""
                err "NOTE: API tokens and passwords cannot be passed as arguments."
                err "      They are collected interactively to avoid shell history exposure."
                err "      Run '$0 --help' for usage."
                exit 1
                ;;
        esac
    done

    detect_os
    check_python
    check_git
    check_ansible
    fetch_platform

    # Project configuration – three modes:
    #   1. --config FILE  → import external project.yml, skip wizard
    #   2. --project FILE → use an already-placed project.yml, skip wizard
    #   3. (neither)      → run interactive setup wizard
    if [ -n "${FSN_CONFIG}" ]; then
        import_config "${FSN_CONFIG}"
    elif [ -z "${FSN_PROJECT}" ]; then
        setup_project_interactive
    fi

    collect_secrets
    run_playbooks

    echo -e "\n${GREEN}${BOLD}[FSN] Installation complete.${NC}\n"
}

main "$@"
