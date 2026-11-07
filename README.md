# CP

<p align="center">
  <img src="src/assets/cp-icon.png" width="96" height="96" alt="CP Logo" style="border-radius: 20px;" />
</p>

<h3 align="center">Copy. Paste. Faster.</h3>

<p align="center">
  An ultra-high-performance Windows copy/paste replacement engine built with Rust, Tauri v2, and native Win32 APIs. Engineered to bypass Windows Explorer's copy bottlenecks through pipelined double-buffering, sequential DMA prefetching, and NTFS contiguous cluster pre-allocation.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.0.1-emerald.svg?style=flat-square" alt="Version" />
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-blue.svg?style=flat-square" alt="Platform" />
  <img src="https://img.shields.io/badge/subsystem-Native%20Win32%20GUI-indigo.svg?style=flat-square" alt="Subsystem" />
  <img src="https://img.shields.io/badge/license-MIT-zinc.svg?style=flat-square" alt="License" />
</p>

---

## ⚡ Why CP?

Default Windows File Explorer copies files through standard buffered synchronous I/O, leading to file cache lockups, mechanical head thrashing on hard drives, and severe UI thread pauses.

**CP** acts as a native high-speed copy pipeline for Windows:

- **🚀 Pipelined Double-Buffering (8MB Chunks, 64MB Queue):** Dedicated reader and writer threads stream data in parallel through a ring buffer, saturating disk controllers with zero idle waits.
- **⚡ Hardware DMA Prefetching (`FILE_FLAG_SEQUENTIAL_SCAN`):** Opens handles with `0x08000000` to command the Windows NT storage miniport driver to initiate lookahead DMA reads directly.
- **💽 Contiguous Sector Pre-Allocation:** Pre-allocates destination files on NTFS upfront (`set_len`), eliminating head-seeking between the Master File Table (MFT) and data sectors on mechanical drives.
- **📋 Native Win32 Low-Level Interceptor:** Uses a low-level keyboard hook (`WH_KEYBOARD_LL`) to detect `Ctrl+V` within File Explorer (`CabinetWClass`), suppresses the slow default copy dialog, and triggers CP instantly.
- **🛡️ Robust Pause, Resume & Instant Cancel:** Pause suspends both threads immediately; cancel safely closes handles, deletes the incomplete file with retry backoff, and flushes Explorer with `SHChangeNotify`.
- **✨ Apple/Linear-Grade Interface:** Clean $640 \times 350\,\text{px}$ utility console, animated shimmer progress bar, full source/dest paths without truncation, and live telemetry (Speed, Processed, ETA).
- **🔕 Zero Terminal Flashing:** Configured with `#![windows_subsystem = "windows"]` and in-process CPUID probing.
- **🔒 Encrypted Workspace Vault (`cp-vault.enc`):** AES-256-GCM + scrypt container ensuring 100% instant offline brain and configuration recovery on freshly formatted machines.

---

## 🏛️ Architecture

```
cp/
├── public/                 # Static web assets & icons
├── scripts/                # Development supervisor, vault & release pipeline
│   ├── dev.mjs             # Clean dev environment runner with port & process hygiene
│   └── vault.mjs           # AES-256-GCM encrypted workspace vault packer
├── src/                    # Frontend (React 19 + TypeScript + Tailwind CSS v4)
│   ├── assets/             # Brand logos & iconography
│   ├── components/         # Header, StatusGrid, TransferView
│   ├── services/           # Type-safe Tauri IPC bridge
│   └── types/              # Data models and transfer state contracts
└── src-tauri/              # Backend (Rust 2021 + Tauri v2)
    ├── src/
    │   ├── engine.rs       # Pipelined streaming engine, pre-allocation & clipboard reader
    │   ├── hardware.rs     # In-process CPUID topology & disk probe
    │   ├── interceptor.rs  # Win32 WH_KEYBOARD_LL Explorer Ctrl+V interceptor
    │   ├── ota.rs          # Remote OTA pipeline client
    │   └── lib.rs          # Tauri runtime, system tray & window management
    ├── tauri.conf.json     # Window dimensions, bundle metadata & security config
    └── Cargo.toml          # Rust dependencies & Windows API bindings
```

---

## 🚀 Getting Started

### Prerequisites
- **OS:** Windows 10 or 11 (64-bit)
- **Node.js:** v20+
- **Rust:** 1.77+ with `msvc` toolchain

### 1-Click Automated Setup (Fresh Machine Setup)

```bash
# 1. Clone repository
git clone https://github.com/AspiringWebGaurav/cp.git
cd cp

# 2. Run automated setup (installs deps, restores vault, and verifies toolchain)
Setup.cmd
```
*(Or manually via: `npm install && npm run vault:unpack && npm run doctor`)*

### Development & Diagnostics

```bash
# Launch local dev environment (supervised native Win32 window)
npm run dev

# Run comprehensive system diagnostics
npm run doctor

# Check GPHost cloud storage readiness
npm run gphost:audit
```

---

## 📜 License

Distributed under the **MIT License**. See [LICENSE](LICENSE) for full details.

Copyright (c) 2026 Gaurav Patil.
