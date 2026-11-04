# CP Architecture Specification

## Overview
**CP (Copy/Paste Engine)** is a lightweight, ultra-high-performance Windows copy/paste replacement driver built with Rust and native Win32 APIs.

```
       [Windows Explorer]
               │ (Ctrl+C / Copy)
               ▼
   [Win32 Clipboard (CF_HDROP)]
               │
               ▼
       [CP DRIVER Engine]
         ├── Hardware Topology Cache (NVMe / SATA / ReFS)
         ├── 4MB Sector-Aligned IOCP Buffer
         ├── SHA-256 / CRC32 Integrity Verifier
         └── SHChangeNotify (Instant Shell Refresh)
               │ (Streaming Transfer)
               ▼
       [Physical Storage (D:\)]
```

## Modular Directory Layout

```
cp/
├── docs/                       # Architectural specifications & pipeline manuals
│   ├── ARCHITECTURE.md
│   └── PIPELINE.md
├── public/                     # Static assets & brand identity
├── scripts/                    # Dev environment & release pipeline
│   ├── dev.mjs                 # Zero-warning dev server supervisor
│   ├── predev.mjs              # Clean process & port sweeper
│   └── ship.mjs                # GPHost OTA release packager
├── src/                        # Enterprise Frontend (React 19 + TypeScript + Tailwind 4)
│   ├── assets/                 # Vector & raster brand icons
│   ├── components/             # Reusable UI components
│   │   ├── ClipboardBanner.tsx # Dynamic clipboard detection card
│   │   ├── Header.tsx          # Brand emblem lockup
│   │   ├── StatusGrid.tsx      # Symmetrical hardware telemetry grid
│   │   └── TransferView.tsx    # High-throughput streaming progress dialog
│   ├── services/               # Typed Tauri IPC client layer
│   │   └── tauriApi.ts
│   ├── types/                  # Centralized TypeScript contracts
│   │   └── index.ts
│   ├── App.css
│   ├── App.tsx
│   ├── main.tsx
│   └── vite-env.d.ts
├── src-tauri/                  # Native Rust Backend (Tauri 2 + Win32 API)
│   ├── icons/                  # Multi-resolution Windows ICO & PNG suites
│   ├── src/
│   │   ├── engine.rs           # Streaming file copy & clipboard inspector
│   │   ├── hardware.rs         # Zero-process CPUID & topology probing
│   │   ├── ota.rs              # GPHost Cloud Engine updater
│   │   ├── lib.rs              # Tauri initialization, window & tray hooks
│   │   └── main.rs             # Windows GUI subsystem entry point
│   ├── Cargo.toml
│   └── tauri.conf.json
├── index.html                  # Frontend entry point
├── package.json
├── tsconfig.json
└── vite.config.ts
```

