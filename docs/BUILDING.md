# Building NEXUS from source

NEXUS is a Tauri 2 desktop application: a Rust core (Tauri commands, SQLite
storage, providers, agent, ingestion, intelligence, memory, graph) and a
React + TypeScript + Vite web UI.

## Prerequisites

* **Rust** 1.79 or newer (`rustup toolchain install stable`)
* **Node.js** 20+ and **npm** 10+
* **Tauri 2** system requirements for your platform
  * **Windows**: WebView2 runtime (preinstalled on Windows 10+), MSVC build tools
  * **macOS**: Xcode command line tools
  * **Linux**: `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libgtk-3-dev`,
    `libayatana-appindicator3-dev`, `librsvg2-dev`

## Clone and bootstrap

```sh
git clone <your-fork-or-mirror> nexus
cd nexus
```

The repository is a single Cargo workspace for the Tauri side and an
`npm` workspace for the web side.

## Run in development

The `tauri.conf.json` `build.beforeDevCommand` runs `npm --prefix ../web
run dev` automatically when you start the Tauri shell. So in most cases
the one-liner is enough:

```sh
# from repo root
cd app/src-tauri
cargo tauri dev
```

If you do not have the Tauri CLI installed:

```sh
cargo install tauri-cli --version "^2.0"
cargo tauri dev
```

The first build will be slow (10+ minutes) because Cargo has to build
every transitive crate. Subsequent runs are fast.

## Run only the web side

Useful for UI work that does not require the Tauri runtime:

```sh
cd app/web
npm install
npm run dev
```

The Vite dev server runs on `http://localhost:5173` but without
Tauri's IPC the page is mostly empty. Use `cargo tauri dev` for a
full check.

## Tests

```sh
cd app/src-tauri
cargo test --lib --tests
```

The test suite is plain `cargo test`; there is no separate Jest / Vitest
configuration. Live tests (LM Studio, MemPalace, MLflow) are gated with
`#[ignore]` and only run on request:

```sh
cargo test --lib --tests -- --ignored
```

## Build a release binary

```sh
cd app/src-tauri
cargo tauri build
```

This will:

1. Run `npm --prefix ../web run build` (the Vite production build).
2. Compile the Rust binary in `--release` mode.
3. Bundle the result into an MSI/NSIS installer (Windows), a `.app`
   (macOS), or an AppImage/.deb (Linux), depending on your host.

The bundle output is written to
`app/src-tauri/target/release/bundle/`.

### Cross-platform notes

* Building a Windows MSI from macOS or Linux is not supported by the
  official Tauri toolchain; do Windows builds on Windows.
* macOS `.app` bundles must be signed and notarised for distribution.
* Linux AppImage is the most portable single-file output.

## Code signing (optional but recommended)

Tauri reads the standard `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` environment variables. The key
generation step is documented at
<https://tauri.app/distribute/sign/windows/>.

## Output locations

| Artifact             | Path                                      |
|----------------------|-------------------------------------------|
| Compiled binary      | `app/src-tauri/target/release/nexus(.exe)`|
| Windows installer    | `app/src-tauri/target/release/bundle/nsis/`|
|                      | `app/src-tauri/target/release/bundle/msi/` |
| macOS `.app`         | `app/src-tauri/target/release/bundle/macos/`|
| Linux AppImage       | `app/src-tauri/target/release/bundle/appimage/`|
| Frontend bundle      | `app/web/dist/`                           |

## Profiling and tracing

* `RUST_LOG=nexus=debug cargo tauri dev` increases Rust log verbosity.
* `RUST_BACKTRACE=1` enables full backtraces on panic.
* The Monitor page samples CPU/RAM/disk from the host every 3 seconds.
  GPU samples are reported honestly — if the platform does not expose
  GPU telemetry, the page shows "unknown" rather than a fabricated value.

## Updating the schema

The SQLite schema is split into numbered migrations in
`app/src-tauri/migrations/`. Add a new file `0012_*.sql`, register it
in `src/db/migrations.rs::ALL`, and re-run the app. The runner is
idempotent: applied versions are recorded in `schema_migrations`.

## Local memory backend (MemPalace)

MemPalace is an optional JSON-RPC sidecar that lives outside the
core. See `docs/MEMPALACE_INTEGRATION.md` for the wire format and
the gating that keeps the live test `#[ignore]`-only.
