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
- Configure query params, headers, and request bodies (JSON, form, raw, multipart).
- Organize requests into collections and folders.
- Environments and variables (`{{base_url}}`, `{{token}}`) with per-environment overrides.
- Response viewer: pretty-printed JSON, headers, status, timing, and size.
- Request history.
- Import/export compatible formats (planned: Postman, Bruno, OpenAPI).

## Status

🚧 Early development. Core request/response flow is the current focus. Expect breaking changes.

## Tech Stack

- **Language:** [Rust](https://www.rust-lang.org/)
- **UI:** [GPUI](https://www.gpui.rs/) — the GPU-accelerated UI framework from the Zed editor.
- **HTTP:** [`reqwest`](https://crates.io/crates/reqwest) (planned)
- **Serialization:** [`serde`](https://serde.rs/) (planned)

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

### Release build

```sh
cargo build --release
```

## Roadmap

- [ ] Send a request and render the response
- [ ] Collections and folders
- [ ] Environments and variables
- [ ] Request history
- [ ] Auth helpers (Bearer, Basic, API key)
- [ ] Scripting / pre-request hooks
- [ ] Import from Postman / Bruno / OpenAPI

## License

MIT (see [LICENSE](LICENSE)).
