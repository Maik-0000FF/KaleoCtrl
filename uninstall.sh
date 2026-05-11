#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# KaleoCtrl — Uninstallation Script
# Mirror of install.sh: removes what was installed.
#
# Supports: Arch/EndeavourOS/Manjaro, Ubuntu/Debian, Fedora, openSUSE
#
# Usage:
#   ./uninstall.sh
# ─────────────────────────────────────────────────────────────

set -euo pipefail

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()    { echo -e "${CYAN}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
error()   { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
step()    { echo -e "\n${BOLD}── $1${NC}"; }

# --- Detect Distribution ---
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        case "$ID" in
            arch|endeavouros|manjaro|garuda|cachyos)
                DISTRO="arch"
                PKG_MGR="pacman"
                ;;
            ubuntu|debian|linuxmint|pop|elementary|zorin)
                DISTRO="debian"
                PKG_MGR="apt"
                ;;
            fedora|nobara|ultramarine)
                DISTRO="fedora"
                PKG_MGR="dnf"
                ;;
            opensuse*|sles)
                DISTRO="suse"
                PKG_MGR="zypper"
                ;;
            *)
                DISTRO="unknown"
                PKG_MGR="unknown"
                ;;
        esac
    else
        DISTRO="unknown"
        PKG_MGR="unknown"
    fi
}

# --- Check if command exists ---
has() { command -v "$1" &>/dev/null; }

# --- Ask yes/no (default No) ---
ask_yn() {
    local prompt="$1"
    local answer
    read -rp "$prompt [y/N] " answer
    [[ "$answer" =~ ^[Yy] ]]
}

# --- Banner ---
print_banner() {
    echo -e "${CYAN}"
    echo "  ╔═══════════════════════════════════════╗"
    echo "  ║        KaleoCtrl Uninstaller           ║"
    echo "  ║   Removes binary, icons, desktop entry ║"
    echo "  ╚═══════════════════════════════════════╝"
    echo -e "${NC}"
}

# --- Remove desktop integration (mirror of install_desktop) ---
remove_desktop() {
    step "Removing desktop integration"

    local removed=0

    # Binary
    if [ -f "$HOME/.local/bin/kaleoctrl" ]; then
        rm -f "$HOME/.local/bin/kaleoctrl"
        success "Removed $HOME/.local/bin/kaleoctrl"
        removed=1
    else
        info "No binary at $HOME/.local/bin/kaleoctrl"
    fi

    # Icons
    local icon_paths=(
        "$HOME/.local/share/icons/hicolor/256x256/apps/kaleoctrl.png"
        "$HOME/.local/share/icons/hicolor/128x128/apps/kaleoctrl.png"
        "$HOME/.local/share/icons/hicolor/32x32/apps/kaleoctrl.png"
    )
    for icon in "${icon_paths[@]}"; do
        if [ -f "$icon" ]; then
            rm -f "$icon"
            success "Removed $icon"
            removed=1
        fi
    done

    # Desktop entry
    if [ -f "$HOME/.local/share/applications/kaleoctrl.desktop" ]; then
        rm -f "$HOME/.local/share/applications/kaleoctrl.desktop"
        success "Removed $HOME/.local/share/applications/kaleoctrl.desktop"
        removed=1
    else
        info "No desktop entry at $HOME/.local/share/applications/kaleoctrl.desktop"
    fi

    # Refresh GTK icon cache (best-effort)
    if [ "$removed" = "1" ] && has gtk-update-icon-cache; then
        gtk-update-icon-cache "$HOME/.local/share/icons/hicolor/" 2>/dev/null \
            || warn "GTK icon cache update failed (icons may linger until next login)"
    fi

    # Refresh desktop database (best-effort)
    if [ "$removed" = "1" ] && has update-desktop-database; then
        update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    fi

    if [ "$removed" = "0" ]; then
        info "Nothing to remove — desktop integration was not installed"
    else
        success "Desktop integration removed"
    fi
}

# --- Remove project-local artifacts (optional) ---
remove_project_artifacts() {
    step "Project-local artifacts"

    if [ -d "models" ]; then
        local size
        size=$(du -sh models 2>/dev/null | awk '{print $1}')
        echo "  models/ exists (${size:-unknown size}) — contains downloaded Whisper models"
        if ask_yn "  Delete models/ ?"; then
            rm -rf models
            success "Removed models/"
        else
            info "Kept models/"
        fi
    fi

    if [ -d "src-tauri/target" ]; then
        local size
        size=$(du -sh src-tauri/target 2>/dev/null | awk '{print $1}')
        echo "  src-tauri/target/ exists (${size:-unknown size}) — Rust build artifacts"
        if ask_yn "  Delete src-tauri/target/ ?"; then
            rm -rf src-tauri/target
            success "Removed src-tauri/target/"
        else
            info "Kept src-tauri/target/"
        fi
    fi

    if [ -d "node_modules" ]; then
        local size
        size=$(du -sh node_modules 2>/dev/null | awk '{print $1}')
        echo "  node_modules/ exists (${size:-unknown size}) — npm dependencies"
        if ask_yn "  Delete node_modules/ ?"; then
            rm -rf node_modules
            success "Removed node_modules/"
        else
            info "Kept node_modules/"
        fi
    fi

    if [ -d "dist" ]; then
        echo "  dist/ exists — Vite frontend build output"
        if ask_yn "  Delete dist/ ?"; then
            rm -rf dist
            success "Removed dist/"
        else
            info "Kept dist/"
        fi
    fi
}

# --- Remove configuration (optional, destructive) ---
remove_config() {
    step "Configuration files"

    if [ ! -d "config" ]; then
        info "No config/ directory found"
        return
    fi

    warn "config/ contains your settings.json and keywords_*.json"
    warn "This will delete your custom assistant name, language, keyword tweaks, etc."
    if ask_yn "  Delete config/ ?"; then
        rm -rf config
        success "Removed config/"
    else
        info "Kept config/"
    fi
}

# --- Remove system packages (optional, dangerous) ---
remove_system_deps() {
    step "System packages (optional)"

    warn "System packages installed by install.sh are general-purpose libraries"
    warn "(GTK, WebKit, ALSA, Vulkan, …). Other applications likely depend on them."
    warn "Removing them can break unrelated software. Skip this unless you know what you're doing."
    echo ""

    if ! ask_yn "  Attempt to remove system packages anyway?"; then
        info "Skipped — system packages kept"
        return
    fi

    case "$DISTRO" in
        arch)
            info "Removing via pacman -Rns (skips packages still required by others)"
            sudo pacman -Rns --noconfirm \
                webkit2gtk-4.1 libayatana-appindicator \
                vulkan-headers \
                xdotool wtype ydotool wl-clipboard \
                2>&1 | grep -v "target not found" || true
            ;;
        debian)
            info "Removing via apt-get remove --purge"
            sudo apt-get remove --purge -y \
                libwebkit2gtk-4.1-dev \
                libayatana-appindicator3-dev \
                libvulkan-dev \
                xdotool wtype ydotool wl-clipboard \
                2>&1 || true
            sudo apt-get autoremove -y || true
            ;;
        fedora)
            info "Removing via dnf remove"
            sudo dnf remove -y \
                webkit2gtk4.1-devel \
                libayatana-appindicator-devel \
                vulkan-headers vulkan-loader-devel \
                xdotool wtype ydotool wl-clipboard \
                2>&1 || true
            ;;
        suse)
            info "Removing via zypper remove"
            sudo zypper remove -y \
                webkit2gtk3-devel \
                libayatana-appindicator3-devel \
                vulkan-devel \
                xdotool wtype ydotool wl-clipboard \
                2>&1 || true
            ;;
        *)
            warn "Unknown distribution — please remove packages manually if desired"
            ;;
    esac

    info "Note: build tools (cmake, base-devel, …) and audio libs (alsa, pulse)"
    info "were not removed — they are common dependencies for many programs."
}

# --- Summary ---
print_summary() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════${NC}"
    echo -e "${GREEN}  KaleoCtrl uninstallation complete${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════${NC}"
    echo ""
    echo "  Notes:"
    echo "    - Rust toolchain (rustup) was not touched"
    echo "    - Node.js was not touched"
    echo "    - The project directory itself was not removed"
    echo ""
}

# --- Main ---
main() {
    print_banner
    detect_distro

    info "Detected: ${PRETTY_NAME:-Unknown Linux} ($DISTRO)"
    echo ""
    echo "  Choose uninstall mode:"
    echo ""
    echo "    1) Standard       — remove binary, icons, desktop entry"
    echo "    2) Deep clean     — standard + models, build artifacts, node_modules"
    echo "    3) Full wipe      — deep clean + config + offer to remove system packages"
    echo ""
    read -rp "  Select [1/2/3]: " mode

    case "$mode" in
        1)
            remove_desktop
            print_summary
            ;;
        2)
            remove_desktop
            remove_project_artifacts
            print_summary
            ;;
        3)
            remove_desktop
            remove_project_artifacts
            remove_config
            remove_system_deps
            print_summary
            ;;
        *)
            error "Invalid selection."
            ;;
    esac
}

main "$@"
