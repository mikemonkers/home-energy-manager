# GivEnergy Local

Desktop app for monitoring and controlling GivEnergy solar inverters over local Modbus TCP.

## Stack

- **Frontend**: React 19 + TypeScript + Vite 8 + Tailwind CSS 4 + Zustand + Recharts + React Router 7
- **Backend**: Tauri 2 desktop shell; embedded Axum HTTP/WS server on port **7337**
- **Modbus**: Custom Rust TCP client to GivEnergy data adapter (port **8899**)
- **Testing**: Rust unit tests only (no frontend tests, no integration tests)

## Prerequisites

- **Node.js** + npm
- **Rust** toolchain (`rustup`)
- **Tauri CLI**: `cargo install tauri-cli`

## Commands

| Command | Action |
|---|---|
| `npm run dev` | Vite dev server on port 5173 |
| `npm run build` | `tsc -b && vite build` (full typecheck + bundle) |
| `npm run lint` | `eslint .` |
| `npm run preview` | `vite preview` |
| `cargo test` (in `src-tauri/`) | Run all Rust unit tests (98 tests) |
| `cargo tauri dev` | Dev mode with Tauri window + Vite + hot-reload |
| `cargo tauri build` | Production build of the desktop app |

Order for full verification: `npm run lint` → `npm run build` (typechecks) → `cargo test` in `src-tauri/`.

## Architecture

### Frontend (`src/`)

React app. Entrypoint: `src/main.tsx`.

- **Pages**: `StatusPage` (dashboard + energy flow), `BatteryPage` (cell-level detail), `HistoryPage` (charts), `ControlPage` (schedules, modes, limits), `SettingsPage` (connection config, connected clients, developer mode, about), `LogsPage` (developer console — only visible when developer mode is enabled)
- **Components**: `EnergyFlowDiagram` (radial SVG power flow), `BatteryPanel` (per-module cell data), `SummaryTiles` (power stats)
- **Hooks**: `useWebSocket` — connects to `/ws`, reconnects on drop, fetches initial snapshot via REST
- **Lib**: `api.ts` (fetch helpers), `format.ts` (power/voltage/temp formatters), `types.ts` (InverterSnapshot etc.)
- **State**: Zustand store (`useInverterStore`) holds `snapshot`, `connectionState`, `connectedHost`, `developerMode` (persisted to localStorage)
- **Version**: Injected at build time via `__APP_VERSION__` (defined in `vite.config.ts`, declared in `src/env.d.ts`)

Frontend talks exclusively to the local Axum server — never directly to the inverter.

### Backend (`src-tauri/src/`)

- **`lib.rs`** — Tauri app setup + headless CLI entry; spawns Axum server (port 7337) + Modbus polling loop
- **`history/`** — SQLite-backed history storage (`~/.givenergy-local/history.db`)
  - `mod.rs` — `HistoryDb` wrapper, schema migration, `insert_reading()`, aggregated `query_history()` with time-bucket AVG
- **`inverter/`** — data model, register decode/encode, discovery, poll loop
  - `model.rs` — `InverterSnapshot`, `ScheduleSlot`, `BatteryMode`, `BatteryState`
  - `decoder.rs` — converts raw register blocks into `InverterSnapshot`; applies global `enable_charge`/`enable_discharge` flags to slot states
  - `encoder.rs` — translates `ControlCommand` enum into `RegisterWrite` lists (whitelist-validated)
  - `poll.rs` — main polling loop: drain pending writes → read registers → broadcast snapshot; uses `Notify` for immediate write execution
  - `discovery.rs` — network scanning with GivEnergy Modbus protocol verification (sends a read request and validates the 0x5959 magic header in the response)
- **`modbus/`** — GivEnergy Modbus TCP protocol
  - `client.rs` — `ModbusClient`: connect, read registers, write single register (FC6), stale frame drain
  - `framer.rs` — proprietary frame encode/decode (MBAP header + transparent sub-frame + CRC); response CRC validation is lenient (logged, not rejected)
  - `registers.rs` — register addresses, poll block definitions, safe-write whitelist, HHMM encode/decode
- **`server/`** — Axum HTTP layer
  - `api.rs` — REST endpoints for control commands; queues writes to `AppState::pending_writes` and notifies poll loop
  - `ws.rs` — WebSocket endpoint streaming `PollMessage` (snapshot or connection state)
  - `logs.rs` — Log ring buffer (`LogRing`) + tracing capture layer + `GET /api/logs` endpoint for developer console
  - `mod.rs` — router setup, server startup (graceful bind failure, no panic)
- **`settings/`** — persisted JSON config (`~/.givenergy-local/settings.json`)

### Shared state (`AppState`)

Central `Arc<Mutex<…>>`-based state shared between poll loop, API handlers, and WebSocket:

- `latest_snapshot` — most recent `InverterSnapshot`
- `connection_state` — `Connected` / `Disconnected`
- `pending_writes` — queue of `Vec<RegisterWrite>` batches from the API
- `write_notify` — `Notify` that wakes the poll loop immediately when writes are queued
- `settings` — live `PollSettings` (host, port, serial, interval)
- `history` — `HistoryDb` for time-series storage
- `log_ring` — `LogRing` (2000-entry ring buffer) of captured log lines for the developer console

## Modbus write protocol

Per the [givenergy-modbus](https://github.com/dewet22/givenergy-modbus) reference library:

- **Function code 6** (Write Single Holding Register) — one register per request
- **Device address 0x11** (inverter setup address) — NOT 0x32 (BMS/poll address)
- **CRC/check**: `CrcModbus(function_code + register + value)` — computed per the reference library
- **Slot clearing**: write `0` (not sentinel 60) — `00:00–00:00` is treated as disabled
- **Retry policy**: 6 attempts with 2s delay for exception code 67 (dongle busy); fail fast and continue

Known limitation: register 32 (charge slot 2 end time) consistently returns exception 67 on some inverters despite being in the reference library's safe-write list. The system handles this gracefully — `enable_charge` flag still updates correctly.

## TypeScript quirks

- `verbatimModuleSyntax: true` — use `import type` for type-only imports
- `erasableSyntaxOnly: true` — no `enum`, no `namespace`, no `constructor parameter properties`
- `noUnusedLocals` / `noUnusedParameters` — both on; declarations must be used
- ESLint rule `react-hooks/set-state-in-effect` — do not call `setState` directly inside `useEffect`; use key-based remounting or derived values instead

## Rust testing

All tests are `#[cfg(test)]` unit tests inside each module. Run with:
```
cd src-tauri && cargo test
```
No integration tests or test fixtures exist. The Modbus client tests use a mock TCP server.

## Build artifacts

- `dist/` — Vite output (frontend)
- `src-tauri/target/` — Rust build output
- `node_modules/.tmp/tsconfig.*.tsbuildinfo` — TypeScript incremental build info

## Headless server mode (Linux)

Run without a Tauri window — just the Axum HTTP/WS server and Modbus poll loop. Ideal for Raspberry Pi or always-on servers.

```bash
# Build the frontend first
npm run build

# Build the binary
cd src-tauri && cargo build --release

# Run headless
./target/release/givenergy-local --headless
./target/release/givenergy-local --headless --port 8080
./target/release/givenergy-local --headless --dist /path/to/dist
```

The `--dist` flag specifies the frontend static files directory. Search order: `--dist` arg > `./dist/` (cwd) > `<exe_dir>/dist/` > `/usr/share/givenergy-local/dist/`. If no dist is found, runs API-only (REST + WebSocket still work).

## Known issues

### Cost graphs inflated by ~1000× (HistoryPage Cost tab)

**Symptom**: Import cost and export income charts on the History page show values ~1000× too high. E.g. 39.0 kWh import at 0.305 £/kRate displays £198 instead of ~£11.90.

**Investigation findings**:

1. **Current approach**: The cost preprocess function in `getCharts()` (`src/pages/HistoryPage.tsx`) computes cost by taking deltas of `today_import_kwh` (or `today_export_kwh`) between consecutive AVG'd bucket values from the history API.

2. **Root cause**: `today_import_kwh` is a cumulative daily counter (monotonically increasing, resets at midnight). The history API (`GET /api/history`) returns `AVG(today_import_kwh)` per time bucket (30s for 1h, 60s for 6h, 300s for 24h, etc.). Taking deltas of AVG'd cumulative counters is fragile:
   - Corrupted/zero register readings inside a bucket bias the AVG downward, creating an artificially large delta to the next bucket
   - Data gaps (disconnections, inverter restarts) cause the delta to span the gap period, importing the accumulated energy as a single large cost spike
   - Midnight rollover resets the counter from ~max to ~0 — the fallback `raw >= prev ? raw - prev : raw` logic then uses the raw value as the delta, which is the ~0 after reset, masking the issue but losing data
   - The very first bucket's AVG represents the midpoint of the first interval, not the starting value, so the first delta is from midpoint of bucket 1 to midpoint of bucket 2 — approximately correct but amplified by any data irregularities

3. **Decoder verification**: The decoder (`src-tauri/src/inverter/decoder.rs`) correctly applies `* 0.1` to register values: `snap.today_import_kwh = get_reg(data, 26) as f32 * 0.1`. This converts from 0.1 kWh units (register IR(26): e_grid_in_day) to kWh. Unit tests confirm this: register 26 = 52 → `today_import_kwh = 5.2`.

4. **Storage verification**: `InverterSnapshot` serialises via Serde (f32 → JSON number). History DB stores as `REAL`. History query returns `AVG("today_import_kwh")`. Data pipeline is clean — no unit conversion errors found in code.

5. **Likely explanation**: The AVG of a cumulative counter over buckets, combined with the delta computation, amplifies small data irregularities. If a single zero-readout (register corruption) falls in a bucket, it significantly drags down the AVG, and the delta to the next clean bucket adds a massive "import" spike. This compounds across many buckets to produce the ~1000× error.

6. **Mitigation applied**: Backend `sanitize_snapshot()` now checks all six `today_*_kwh`
   fields for jumps >50 kWh or values outside 0–1000 kWh, falling back to the previous
   reading. Frontend spike-removal thresholds added for these fields (50 kWh). A
   transparent overlay banner on cost charts warns users the data may be inaccurate.

7. **Recommended fix**: Switch the cost computation from `today_import_kwh` deltas to `grid_power` instantaneous readings. Power values (in W) compose correctly under AVG: `AVG(grid_power) * bucket_duration_hours / 1000 = net import/export energy in kWh`. The history API already tracks `grid_power` as INTEGER (W, signed: +exporting, -importing).

   Proposed approach for both import cost and export income:
   ```typescript
   // For each bucket row:
   //   import_power_kw = max(-avg_grid_power, 0) / 1000
   //   duration_h = (current_t_ms - prev_t_ms) / 3600000
   //   import_energy_kwh = import_power_kw * duration_h
   //   cost += import_energy_kwh * rate(t)
   ```

## Release process

1. Bump version in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`
2. Update `CHANGELOG.md`
3. Commit, tag (`vX.Y.Z`), push tag
4. GitHub Actions workflow (`.github/workflows/build.yml`) builds for macOS (ARM + x64), Linux, Windows and creates a GitHub Release with binaries attached
