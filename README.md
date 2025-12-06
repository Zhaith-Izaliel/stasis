<p align="center">
  <img src="assets/stasis.png" alt="Stasis Logo" width="200"/>
</p>

<h1 align="center">Stasis</h1>

<p align="center">
  <strong>A modern Wayland idle manager that knows when to step back.</strong>
</p>

<p align="center">
  Keep your session in perfect balance—automatically preventing idle when it matters, allowing it when it doesn't.
</p>

<p align="center">
  <img src="https://img.shields.io/github/last-commit/saltnpepper97/stasis?style=for-the-badge&color=%2328A745" alt="GitHub last commit"/>
  <img src="https://img.shields.io/aur/version/stasis?style=for-the-badge" alt="AUR version">
  <img src="https://img.shields.io/badge/License-MIT-E5534B?style=for-the-badge" alt="MIT License"/>
  <img src="https://img.shields.io/badge/Wayland-00BFFF?style=for-the-badge&logo=wayland&logoColor=white" alt="Wayland"/>
  <img src="https://img.shields.io/badge/Rust-1.89+-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#compositor-support">Compositor Support</a> •
  <a href="#media-bridge-browser-plugin">Media Bridge Plugin</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## Features

Stasis doesn't just lock your screen after a timer—it understands context. Watching a video? Reading a document? Playing music? Stasis detects these scenarios and intelligently manages idle behavior, so you never have to jiggle your mouse to prevent an unwanted screen lock.

- **🧠 Smart idle detection** with configurable timeouts
- **🎵 Media-aware idle handling** – automatically detects media playback
- **🌐 Per-tab browser detection** – optional [Media Bridge](#-media-bridge-browser-plugin) plugin for granular media tracking
- **🚫 Application-specific inhibitors** – prevent idle when specific apps are running
- **⏸️ Idle inhibitor respect** – honors Wayland idle inhibitor protocols
- **🛌 Lid events via DBus** – detect laptop lid open/close events to manage idle
- **⚙️ Flexible action system** – supports named action blocks and custom commands
- **🔍 Regex pattern matching** – powerful app filtering with regular expressions
- **📝 Clean configuration** – uses the intuitive [RUNE](https://github.com/saltnpepper97/rune-cfg) configuration language
- **⚡ Live reload** – update configuration without restarting the daemon

## Roadmap

### Complete

- [x] **Sequential action blocks** – Action blocks run in the exact order defined in your config. Stasis maintains an internal index to track progress, making execution smarter and more reliable.  
- [x] **Lock-centric design** – When a `lock-screen` action block is present and properly configured, Stasis tracks the PID of the command it executes to ensure the sequence stays intact.  
- [x] **Resume-command support** – Each action block can run an optional follow-up command after completion.  
- [x] **Event-driven, minimal polling** – Stasis now relies primarily on two internal loops that notify others to wake from deep sleep, significantly reducing CPU and memory usage at idle.  
- [x] **CLI per-state triggers** – Trigger a **specific state**, the **current state**, or **all states**, all while respecting previously completed actions.  
- [x] **Custom notifications** - Run notifications before any command block as well as notify on unpause!  requires `libnotify`

### In Progress
- [ ] Quickshell support via `busctl` using a config option
- [ ] **User profiles / presets** – save and load different workflows for various scenarios (work, gaming, etc.).
### Planned
- [ ] **Logging & analytics** – historical idle data for power/performance insights.
- [ ] **Power-saving optimizations** – CPU/GPU-aware idle handling.


## Installation

### Arch Linux (AUR)

Install the stable release or latest development version:

```bash
# Stable release
yay -S stasis

# Or latest git version
yay -S stasis-git
```

Works with `paru` too:
```bash
paru -S stasis
```

### NixOS

**please note the note at the bottom about flakes and nix.**

If you use Nix flakes, `stasis` provides a `flake.nix` so you can build or
consume the package directly from flakes.

Quick ways to use `stasis` from flakes:

- Build directly from the remote flake (no local checkout required):

```bash
# build the stasis package from GitHub
nix build 'github:saltnpepper97/stasis#stasis'
```

- Add `stasis` as an input in your own `flake.nix` and reference the package in
  your outputs or NixOS configuration. Example (snippet):

```nix
inputs = {
  nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  stasis.url = "github:saltnpepper97/stasis";
};

outputs = { self, nixpkgs, ... }:
let
  system = "x86_64-linux"; # adjust for your host
in {
  # reference the stasis package from the stasis flake
  packages.${system}.my-stasis = self.inputs.stasis.packages.${system}.stasis;

  # Or add it to a NixOS configuration
  nixosConfigurations.<host> = nixpkgs.lib.nixosSystem {
    inherit system;
    modules = [ ./configuration.nix ];
    configuration = {
      environment.systemPackages = [ self.inputs.stasis.packages.${system}.stasis ];
    };
  };
}
```

Notes:
- please know this i am a complete noob in nix and flakes, so updates and fixes will be appreciated! @CamRed25

### From Source

#### Dependencies

<p>
<code>git</code> <br />
<code>cargo</code> <br />
<code>rust</code> <br />
<code>dbus</code> <br />
<code>libinput</code> <br />
<code>libnotify</code> - <b>(optional)</b> for desktop notifications <br />
<code>playerctl</code> - <b>(optional)</b> for enhanced media player detection <br />
<code>pulseaudio</code> or <code>pipewire-pulse</code> - <b>(optional)</b> for audio detection <br />
<p>

Build and install manually for maximum control:

```bash
# Clone and build
git clone https://github.com/saltnpepper97/stasis
cd stasis
cargo build --release --locked

# Install system-wide
sudo install -Dm755 target/release/stasis /usr/local/bin/stasis

# Or install to user directory
install -Dm755 target/release/stasis ~/.local/bin/stasis
```

## Quick Start

Get up and running in just a few minutes!  
See the [webpage](https://saltnpepper97.github.io/stasis/) for quick start instructions, including how to ensure your user is in the `input` group and much more!

## Compositor Support

Stasis integrates with each compositor's native IPC protocol for optimal app detection and inhibition.

| Compositor | Support Status | Notes |
|------------|---------------|-------|
| **Niri** | ✅ Full Support | Tested and working perfectly |
| **Hyprland** | ✅ Full Support | Native IPC integration |
| **labwc** | ⚠️ Limited | Process-based fallback (details below) |
| **River** | ⚠️ Limited | Process-based fallback (details below) |
| **Your Favorite** | 🤝 PRs Welcome | Help us expand support! |

### River & labwc Compatibility Notes

Both River and labwc have IPC protocol limitations that affect Stasis functionality:

- **Limited window enumeration:** These compositors don't provide complete window lists via IPC
- **Fallback mode:** Stasis uses process-based detection (sysinfo) for app inhibition
- **Pattern adjustments:** Executable names may differ from app IDs—check logs and adjust regex patterns accordingly

> **💡 Tip:** When using River or labwc, include both exact executable names and flexible regex patterns in your `inhibit_apps` configuration. Enable verbose logging to see which apps are detected.

### Want to Add Compositor Support?

We welcome contributions! Adding support typically involves:

1. Implementing the compositor's native IPC protocol
2. Adding window/app detection functionality  
3. Testing with common applications

Check existing implementations in the codebase for reference, and don't hesitate to open an issue if you need guidance.

## Media Bridge Browser Plugin

**[Media Bridge](https://github.com/saltnpepper97/soundtabs)** is an optional browser extension that provides accurate per-tab media detection. While SoundTabs is a standalone project not exclusive to Stasis, it solves critical browser media detection issues that benefit idle management.

### The Browser Media Problem

Standard browser MPRIS implementations have significant limitations:

- **Browser-wide signals only** – No way to know which specific tab is playing
- **Sticky inhibitors** – Once media starts, MPRIS can inhibit until the tab closes
- **Poor muted tab handling** – Can't distinguish between muted and paused tabs
- **Inaccurate state reporting** – Doesn't always reflect actual playback status

Media Bridge fixes these issues by providing real-time, per-tab audio state directly from the browser.

### Why Use Media Bridge with Stasis?

- **Per-tab accuracy** – Know exactly which tab is playing media
- **Muted tab detection** – Correctly detect when tabs are muted vs. paused
- **Real-time updates** – Instant state changes without audio sink polling
- **Seamless fallback** – Stasis automatically uses standard MPRIS if Media Bridge isn't installed
- **Works alongside other players** – Doesn't interfere with Spotify, VLC, or other media apps

### Browser Support

| Browser | Status | Installation |
|---------|--------|--------------|
| **Firefox** | ✅ Available | [Install Media Bridge](https://github.com/saltnpepper97/media-bridge) |
| **Chrome/Chromium** | 🚧 Coming Soon | Extension in development |
| **Brave/Edge/Vivaldi** | 🚧 Coming Soon | Will use Chrome extension |

### How It Works with Stasis

1. **With Media Bridge:** Stasis receives precise per-tab media state via Unix socket communication
2. **Without Media Bridge:** Stasis falls back to standard MPRIS + audio sink detection
3. **No configuration needed:** Stasis automatically detects and uses Media Bridge when available
4. **Other media respected:** Non-browser media players continue to work through MPRIS

> **📝 Note:** Media Bridge is completely optional and not Stasis-specific. Stasis works great without it using improved MPRIS detection with audio sink verification.

## About RUNE Configuration

Stasis uses **[RUNE](https://github.com/saltnpepper97/rune-cfg)**—a purpose-built configuration language that's both powerful and approachable.

**Why RUNE?**
- **Human-readable:** Clean syntax that makes sense at a glance
- **Variables:** Define once, reference anywhere
- **Type-safe:** Catch configuration errors before runtime
- **Nested blocks:** Organize complex configurations naturally
- **Raw strings:** Use `r"regex.*"` for patterns without escaping hell
- **Comments:** Document your config with `#`
- **Metadata:** Add context with `@` annotations

RUNE makes configuration feel less like programming and more like describing what you want—because that's what a config should be.

## Contributing

Thank you for making Stasis better for everyone! To keep contributions organized and efficient, please follow these guidelines.

### Contribution Flow

1. **Bug Reports & Feature Requests**  
   - Before opening an issue, check the [FAQ](https://saltnpepper97.github.io/stasis/faq).  
   - Any **new feature or enhancement** must first go through a bug report or issue to discuss use cases and design.  
   - Use the appropriate label: `bug` for errors, `feature request` for new ideas, `enhancement` for improvements.  

2. **Packaging & Compositor Contributions**  
   - Direct contributions for **distribution packaging** or **Wayland/compositor integration** are welcome without prior issue discussion.  
   - Use the labels: `packaging` or `compositor`.

3. **Other Contributions**  
   - Anything else not listed above **must start as an issue** so maintainers can review and provide feedback.  
   - Labels like `discussion`, `help wanted`, or `needs review` can help guide attention.

## License

Released under the [MIT License](LICENSE) – free to use, modify, and distribute.

---

<p align="center">
  <sub>Built with ❤️ for the Wayland community</sub><br>
  <sub><i>Keeping your session in perfect balance between active and idle</i></sub>
</p>
