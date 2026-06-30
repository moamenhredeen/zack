# Zack

A fast, native HTTP testing tool built with Rust and [GPUI](https://www.gpui.rs/). Think Postman or Bruno — but lightweight, offline-first, and powered by a GPU-accelerated UI.

## Why Zack?

Most HTTP clients ship a full browser engine (Electron) just to render a form and a JSON viewer. Zack is a native desktop app instead: instant startup, low memory, smooth scrolling on large responses, and your collections stored as plain files you can commit to git.

- **Native & fast** — Rust core, GPUI rendering. No Electron, no Chromium.
- **Offline-first** — no account, no cloud, no telemetry.
- **Git-friendly** — requests and collections live as readable files in your repo.
- **Keyboard-driven** — built for people who'd rather not reach for the mouse.

## Features

- Send requests with all common methods (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`).
- Read and write OpenCollection YAML folders.
- Configure query params, headers, and request bodies (JSON and raw text in the current UI).
- Organize requests in collection files.
- Environments and variables (`{{base_url}}`, `{{token}}`) with per-environment overrides (planned).
- Response viewer: pretty-printed JSON, headers, status, timing, and size.
- Request history (planned).
- Import/export compatible formats (planned: Postman, Bruno, OpenAPI).

## Status

🚧 Early development. Zack currently opens an OpenCollection YAML folder, edits request files, sends HTTP requests, and renders responses. Expect breaking changes.

## Tech Stack

- **Language:** [Rust](https://www.rust-lang.org/)
- **UI:** [GPUI](https://www.gpui.rs/) — the GPU-accelerated UI framework from the Zed editor.
- **HTTP:** [`reqwest`](https://crates.io/crates/reqwest)
- **Serialization:** [`serde`](https://serde.rs/), `serde_yaml`, and `serde_json`

## Getting Started

### Prerequisites

- A recent Rust toolchain (install via [rustup](https://rustup.rs/)).
- GPUI platform dependencies — see the [GPUI docs](https://www.gpui.rs/) for your OS.

### Build & Run

```sh
git clone https://github.com/moamenhredeen/zack.git
cd zack
cargo run
```

By default Zack opens the bundled `sample-collection/`. To open another OpenCollection folder, set `ZACK_COLLECTION` to a directory containing `opencollection.yml`:

```sh
ZACK_COLLECTION=/path/to/collection cargo run
```

### Release build

```sh
cargo build --release
```

## Roadmap

- [x] Send a request and render the response
- [x] Read/write OpenCollection request files
- [ ] Collection and folder creation UI
- [ ] Environments and variables
- [ ] Request history
- [ ] Auth helpers (Bearer, Basic, API key)
- [ ] Scripting / pre-request hooks
- [ ] Import from Postman / Bruno / OpenAPI

## License

MIT (see [LICENSE](LICENSE)).
