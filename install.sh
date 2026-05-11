#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
# KaleoCtrl — Installation Script
# Voice control and speech-to-text for the Linux desktop
#
# Supports: Arch/EndeavourOS/Manjaro, Ubuntu/Debian, Fedora
#
# Usage:
#   chmod +x install.sh
#   ./install.sh
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

# --- Summary ---
print_banner() {
    echo -e "${CYAN}"
    echo "  ╔═══════════════════════════════════════╗"
    echo "  ║         KaleoCtrl Installer            ║"
    echo "  ║   Voice Control for Linux Desktop      ║"
    echo "  ╚═══════════════════════════════════════╝"
    echo -e "${NC}"
}

# --- Install system packages ---
install_system_deps() {
    step "Installing system dependencies"

    case "$DISTRO" in
        arch)
            info "Detected: Arch-based distribution ($PRETTY_NAME)"
            info "The following packages will be installed via pacman:"
            echo ""
            echo "  Build tools:     base-devel cmake pkg-config"
            echo "  Tauri/GTK:       gtk3 webkit2gtk-4.1 libayatana-appindicator"
            echo "  SSL:             openssl"
            echo "  Vulkan:          vulkan-headers vulkan-icd-loader"
            echo "  Audio:           alsa-lib libpulse"
            echo "  Text injection:  xdotool wtype ydotool wl-clipboard"
            echo ""
            echo -e "${YELLOW}This requires sudo privileges.${NC}"
            read -rp "Continue? [Y/n] " answer
            [[ "$answer" =~ ^[Nn] ]] && error "Installation cancelled."

            # Refresh package database — on a system with stale DB,
            # plain `pacman -S` fails with "target not found".
            info "Refreshing package database (pacman -Sy)..."
            sudo pacman -Sy --noconfirm

            sudo pacman -S --needed --noconfirm \
                base-devel cmake pkg-config \
                gtk3 webkit2gtk-4.1 libayatana-appindicator openssl \
                vulkan-headers vulkan-icd-loader \
                alsa-lib libpulse \
                xdotool wtype ydotool wl-clipboard
            ;;

        debian)
            info "Detected: Debian-based distribution ($PRETTY_NAME)"
            info "The following packages will be installed via apt:"
            echo ""
            echo "  Build tools:     build-essential cmake pkg-config"
            echo "  Tauri/GTK:       libgtk-3-dev libwebkit2gtk-4.1-dev"
            echo "                   libayatana-appindicator3-dev"
            echo "  SSL:             libssl-dev"
            echo "  Vulkan:          libvulkan-dev"
            echo "  Audio:           libasound2-dev libpulse-dev"
            echo "  Text injection:  xdotool wtype ydotool wl-clipboard"
            echo ""
            echo -e "${YELLOW}This requires sudo privileges.${NC}"
            read -rp "Continue? [Y/n] " answer
            [[ "$answer" =~ ^[Nn] ]] && error "Installation cancelled."

            sudo apt-get update
            sudo apt-get install -y \
                build-essential cmake pkg-config \
                libgtk-3-dev libwebkit2gtk-4.1-dev \
                libayatana-appindicator3-dev \
                libssl-dev libvulkan-dev \
                libasound2-dev libpulse-dev \
                xdotool wtype ydotool wl-clipboard
            ;;

        fedora)
            info "Detected: Fedora-based distribution ($PRETTY_NAME)"
            info "The following packages will be installed via dnf:"
            echo ""
            echo "  Build tools:     @development-tools cmake pkg-config"
            echo "  Tauri/GTK:       gtk3-devel webkit2gtk4.1-devel"
            echo "                   libayatana-appindicator-devel"
            echo "  SSL:             openssl-devel"
            echo "  Vulkan:          vulkan-headers vulkan-loader-devel"
            echo "  Audio:           alsa-lib-devel pulseaudio-libs-devel"
            echo "  Text injection:  xdotool wtype ydotool wl-clipboard"
            echo ""
            echo -e "${YELLOW}This requires sudo privileges.${NC}"
            read -rp "Continue? [Y/n] " answer
            [[ "$answer" =~ ^[Nn] ]] && error "Installation cancelled."

            sudo dnf install -y \
                @development-tools cmake pkg-config \
                gtk3-devel webkit2gtk4.1-devel \
                libayatana-appindicator-devel \
                openssl-devel vulkan-headers vulkan-loader-devel \
                alsa-lib-devel pulseaudio-libs-devel \
                xdotool wtype ydotool wl-clipboard
            ;;

        suse)
            info "Detected: openSUSE ($PRETTY_NAME)"
            info "The following packages will be installed via zypper:"
            echo ""
            echo "  Build tools:     -t pattern devel_basis, cmake, pkg-config"
            echo "  Tauri/GTK:       gtk3-devel webkit2gtk3-devel"
            echo "                   libayatana-appindicator3-devel"
            echo "  SSL:             libopenssl-devel"
            echo "  Vulkan:          vulkan-devel"
            echo "  Audio:           alsa-devel libpulse-devel"
            echo "  Text injection:  xdotool wtype ydotool wl-clipboard"
            echo ""
            echo -e "${YELLOW}This requires sudo privileges.${NC}"
            read -rp "Continue? [Y/n] " answer
            [[ "$answer" =~ ^[Nn] ]] && error "Installation cancelled."

            sudo zypper install -y \
                -t pattern devel_basis \
                cmake pkg-config \
                gtk3-devel webkit2gtk3-devel \
                libayatana-appindicator3-devel \
                libopenssl-devel vulkan-devel \
                alsa-devel libpulse-devel \
                xdotool wtype ydotool wl-clipboard
            ;;

        *)
            warn "Unknown distribution. Please install the following manually:"
            echo ""
            echo "  - GTK3 + dev headers"
            echo "  - WebKitGTK 4.1 + dev headers"
            echo "  - libayatana-appindicator + dev headers"
            echo "  - OpenSSL + dev headers"
            echo "  - Vulkan SDK headers + loader"
            echo "  - ALSA + PulseAudio dev headers"
            echo "  - cmake, pkg-config, build tools"
            echo "  - xdotool, wtype, ydotool, wl-clipboard"
            echo ""
            read -rp "Continue anyway? [y/N] " answer
            [[ ! "$answer" =~ ^[Yy] ]] && error "Installation cancelled."
            ;;
    esac

    success "System dependencies installed"
}

# --- Install Rust ---
install_rust() {
    step "Checking Rust toolchain"

    if has rustc && has cargo; then
        local rust_ver
        rust_ver=$(rustc --version | awk '{print $2}')
        success "Rust $rust_ver already installed"
    else
        info "Rust is not installed. Installing via rustup..."
        info "rustup is the official Rust installer (https://rustup.rs)"
        echo ""
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        success "Rust $(rustc --version | awk '{print $2}') installed"
    fi

    # Install Tauri CLI
    if has cargo-tauri; then
        success "Tauri CLI already installed"
    else
        info "Installing Tauri CLI..."
        cargo install tauri-cli
        success "Tauri CLI installed"
    fi
}

# --- Install Node.js ---
install_node() {
    step "Checking Node.js"

    if has node && has npm; then
        local node_ver
        node_ver=$(node --version)
        success "Node.js $node_ver already installed"
    else
        info "Node.js is not installed."

        case "$DISTRO" in
            arch)
                info "Installing via pacman..."
                sudo pacman -S --needed --noconfirm nodejs npm
                ;;
            debian)
                info "Installing via apt..."
                sudo apt-get install -y nodejs npm
                ;;
            fedora)
                info "Installing via dnf..."
                sudo dnf install -y nodejs npm
                ;;
            suse)
                info "Installing via zypper..."
                sudo zypper install -y nodejs npm
                ;;
            *)
                warn "Please install Node.js (v16+) and npm manually."
                warn "Visit: https://nodejs.org"
                read -rp "Continue anyway? [y/N] " answer
                [[ ! "$answer" =~ ^[Yy] ]] && error "Installation cancelled."
                ;;
        esac

        if has node; then
            success "Node.js $(node --version) installed"
        fi
    fi
}

# --- Detect GPU ---
detect_gpu() {
    step "Detecting GPU"

    if has vulkaninfo; then
        local gpu_name
        gpu_name=$(vulkaninfo --summary 2>/dev/null | grep "deviceName" | head -1 | sed 's/.*= //' || echo "")
        if [ -n "$gpu_name" ]; then
            success "Vulkan GPU detected: $gpu_name"
            info "whisper.cpp will use GPU acceleration automatically"
            return
        fi
    fi

    warn "No Vulkan GPU detected — whisper.cpp will use CPU (slower)"
    info "For GPU acceleration, install your GPU's Vulkan driver:"
    echo ""
    echo "  Intel:  vulkan-intel (Arch) / mesa-vulkan-drivers (Debian/Fedora)"
    echo "  AMD:    vulkan-radeon (Arch) / mesa-vulkan-drivers (Debian/Fedora)"
    echo "  NVIDIA: nvidia-utils (Arch) / nvidia-driver (Debian/Fedora)"
}

# --- Install npm dependencies ---
install_frontend() {
    step "Installing frontend dependencies"

    if [ ! -f "package.json" ]; then
        error "package.json not found. Are you in the KaleoCtrl project directory?"
    fi

    npm install
    success "Frontend dependencies installed"
}

# --- Build ---
build_app() {
    step "Building KaleoCtrl"

    info "This may take several minutes on first build (compiling whisper.cpp)..."
    echo ""

    if ! cargo tauri build; then
        error "Build failed. See output above."
    fi

    success "Build complete!"
}

# --- Install desktop integration ---
install_desktop() {
    step "Installing desktop integration"

    local bin_dir="src-tauri/target/release"
    local bin_name="kaleoctrl"

    if [ ! -f "$bin_dir/$bin_name" ]; then
        error "Binary not found at $bin_dir/$bin_name — build may have failed"
    fi

    # Install binary
    info "Installing binary to ~/.local/bin/"
    mkdir -p "$HOME/.local/bin"
    cp "$bin_dir/$bin_name" "$HOME/.local/bin/"
    chmod +x "$HOME/.local/bin/$bin_name"

    # Install icon
    info "Installing icon"
    mkdir -p "$HOME/.local/share/icons/hicolor/256x256/apps"
    mkdir -p "$HOME/.local/share/icons/hicolor/128x128/apps"
    mkdir -p "$HOME/.local/share/icons/hicolor/32x32/apps"
    cp "src-tauri/icons/icon.png" "$HOME/.local/share/icons/hicolor/256x256/apps/kaleoctrl.png"
    cp "src-tauri/icons/icon-128.png" "$HOME/.local/share/icons/hicolor/128x128/apps/kaleoctrl.png"
    cp "src-tauri/icons/icon-32.png" "$HOME/.local/share/icons/hicolor/32x32/apps/kaleoctrl.png"

    # Install desktop file
    info "Installing desktop entry"
    mkdir -p "$HOME/.local/share/applications"
    cat > "$HOME/.local/share/applications/kaleoctrl.desktop" << EOF
[Desktop Entry]
Categories=Utility;AudioVideo;
Exec=$HOME/.local/bin/kaleoctrl
StartupWMClass=kaleoctrl
Icon=kaleoctrl
Name=KaleoCtrl
Comment=Voice control and speech-to-text for Linux
Terminal=false
Type=Application
EOF

    # Update icon cache
    if has gtk-update-icon-cache; then
        gtk-update-icon-cache "$HOME/.local/share/icons/hicolor/" 2>/dev/null \
            || warn "GTK icon cache update failed (icons may appear after next login)"
    fi

    success "Desktop integration installed"
    info "KaleoCtrl is now available in your application menu"
}

# --- Config setup ---
setup_config() {
    step "Setting up configuration"

    if [ -d "config" ] && [ -f "config/settings.json" ]; then
        success "Configuration already exists"
    else
        error "config/ directory not found. Are you in the KaleoCtrl project directory?"
    fi

    # Ensure models directory exists
    mkdir -p models
    success "Models directory ready (models/)"
}

# --- Summary ---
print_summary() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════${NC}"
    echo -e "${GREEN}  KaleoCtrl installation complete!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════${NC}"
    echo ""
    echo "  Next steps:"
    echo ""
    echo "  1. Launch KaleoCtrl from your application menu"
    echo "     or run:  kaleoctrl"
    echo ""
    echo "  2. Go to Settings > Model Manager"
    echo "     Download a whisper model (recommended: large-v3-turbo)"
    echo ""
    echo "  3. Go to Status, load the model, and start listening"
    echo ""
    echo -e "  ${CYAN}Documentation:${NC} https://github.com/Maik-0000FF/KaleoCtrl"
    echo ""
}

# --- Development mode (skip build + install) ---
dev_mode() {
    install_frontend
    setup_config
    detect_gpu

    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Development setup complete!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════${NC}"
    echo ""
    echo "  Run the app in dev mode:"
    echo ""
    echo "    cargo tauri dev"
    echo ""
    echo "  Then go to Settings > Model Manager to download a model."
    echo ""
}

# --- Main ---
main() {
    print_banner
    detect_distro

    info "Detected: ${PRETTY_NAME:-Unknown Linux} ($DISTRO)"
    echo ""
    echo "  Choose installation mode:"
    echo ""
    echo "    1) Full install    — install deps, build, and install to system"
    echo "    2) Dev setup       — install deps only (for development with cargo tauri dev)"
    echo "    3) Build only      — skip dep install, just build"
    echo ""
    read -rp "  Select [1/2/3]: " mode

    case "$mode" in
        1)
            install_system_deps
            install_rust
            install_node
            install_frontend
            setup_config
            detect_gpu
            build_app
            install_desktop
            print_summary
            ;;
        2)
            install_system_deps
            install_rust
            install_node
            dev_mode
            ;;
        3)
            install_frontend
            setup_config
            detect_gpu
            build_app
            install_desktop
            print_summary
            ;;
        *)
            error "Invalid selection."
            ;;
    esac
}

main "$@"
