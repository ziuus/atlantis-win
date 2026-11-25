# 🔱 Atlantis for Windows (`atlantis-win`)

[![Windows CI](https://img.shields.io/github/actions/workflow/status/ziuus/atlantis-win/ci-release.yml?style=flat-square&label=Windows%20CI)](https://github.com/ziuus/atlantis-win/actions)
[![Engine](https://img.shields.io/badge/Engine-Liquid--Glass-blue?style=flat-square)](https://github.com/ziuus/liquid-glass)
[![License](https://img.shields.io/badge/License-MIT-black?style=flat-square)](LICENSE)

Physical optical **Liquid Glass** desktop shell for **Windows 10 / 11**, powered by [`liquid-glass`](https://github.com/ziuus/liquid-glass) (`glass-core` + `glass-platform`) and `winit` (DX12 / Vulkan via WGPU).

## Features (6-Widget Shell)
- **Top-Center Dynamic Island**: Notifications, Control Center (`Eco` Power Save, `App BG: Solid`, `Theme`, Volume), System Overview.
- **Top-Left Control Hub**: 4 pinned modular boxes + smooth **`More (4)`** expansion + **`Customize`** reorder/visibility (`Audio`, `Display`, `Connectivity`, `System Tools`).
- **Top-Right Tasks**: Compact pill + interactive task checklist (`%LOCALAPPDATA%\atlantis\todos.json`).
- **Bottom Dock**: Glass dock with hover highlights and instant app/launcher triggers.
- **2-Card Spatial Launcher**: Start Menu `.lnk` search, live inspector, math evaluator, and `>` system actions (`F2` or Dock button).

## Ecosystem

| OS | Repository |
|---|---|
| **Linux / Wayland** | [`ziuus/atlantis`](https://github.com/ziuus/atlantis) |
| **Windows 10 / 11** | [`ziuus/atlantis-win`](https://github.com/ziuus/atlantis-win) |
| **Core Engine** | [`ziuus/liquid-glass`](https://github.com/ziuus/liquid-glass) |

## Quick Start (Windows)

```powershell
cargo build --release
cargo run --release
```
