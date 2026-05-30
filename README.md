# Thrust

A Spring-Boot-like build-time code generator for Rust/Axum backends.

Developers write annotated Rust structs. A build step scans the source, extracts metadata, and generates ordinary Rust code — wiring, containers, routers — before `cargo` compiles anything. The full Rust toolchain (rust-analyzer, clippy, cargo check) works on the source unchanged.

---

## Why

Rust backend development requires substantial boilerplate: manual `Arc` wiring, `AppState` construction, router registration, constructor chaining. Existing DI frameworks often fight the borrow checker directly. Thrust sidesteps this by treating ownership as an implementation detail hidden behind generated code.

---

## Architecture

Three approaches were considered:

**New language** (`*.rux` files) — rejected. Would require a parser, type checker, formatter, LSP, and cargo integration. Multi-year effort, zero ecosystem compatibility.

**Proc-macros only** — viable for single-struct transforms, but cross-module dependency graph analysis is hard to implement cleanly as a proc-macro. Validation errors are difficult to surface.

**Build-time code generator (chosen)** — developers write normal Rust with framework attributes. A `build.rs` script runs before compilation, parses the source with `syn`, builds a project-wide metadata model, and writes generated Rust into `OUT_DIR`. `cargo build` then compiles everything together.

```
Annotated Rust source
        ↓
   build.rs (syn + quote)
        ↓
  OUT_DIR/generated.rs
        ↓
   cargo build
        ↓
  Final binary
```

Source files remain valid Rust at all times. rust-analyzer sees the real structs, not generated proxies.

---

## Workspace Structure

```
thrust/
├── Cargo.toml              # workspace manifest
├── thrust-macros/          # proc-macro crate (no-op attribute stubs)
│   └── src/lib.rs          # #[service] — passes the item through unchanged
└── thrust-core/            # example app + scanner
    ├── Cargo.toml          # deps: thrust-macros; build-deps: syn, quote, walkdir
    ├── build.rs            # scans src/, generates OUT_DIR/generated.rs
    └── src/
        ├── main.rs         # include!(generated.rs), prints GENERATED_COMPONENTS
        └── services.rs     # example annotated structs
```

`thrust-macros` exists so that `#[service]` is a valid registered proc-macro attribute. Without it, `rustc` would reject the unknown attribute. The macro body is a pass-through — it returns the item unchanged. Actual metadata extraction happens in `build.rs` via `syn`, not at proc-macro expansion time.

---

## How It Works

1. Developer writes an annotated struct:
   ```rust
   use thrust_macros::service;

   #[service]
   pub struct UserService;
   ```

2. `cargo build` triggers `build.rs` before compiling application code.

3. `build.rs` walks every `.rs` file under `src/` with `walkdir`, parses each with `syn::parse_file`, and collects the names of all structs that carry a `#[service]` attribute.

4. `quote!` renders the collected names into a Rust constant and writes it to `$OUT_DIR/generated.rs`:
   ```rust
   pub const GENERATED_COMPONENTS: &[&str] = &["UserService", "EmailService"];
   ```

5. `main.rs` includes the generated file at compile time:
   ```rust
   include!(concat!(env!("OUT_DIR"), "/generated.rs"));
   ```

6. The compiled binary prints the discovered components.

---

## Quick Start

```bash
cargo build
cargo run -p thrust-core
# ["UserService", "EmailService"]
```

---

## Roadmap

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | ✓ done | Component discovery — scan `#[service]` structs, emit names |
| 2 | planned | Dependency extraction — parse struct fields to find injected types |
| 3 | planned | Dependency graph construction |
| 4 | planned | Missing dependency validation |
| 5 | planned | Cycle detection |
| 6 | planned | Topological ordering |
| 7 | planned | Generate container struct |
| 8 | planned | Generate constructor / build logic |
| 9 | planned | Compile generated container (end-to-end DI working) |
| later | — | Route discovery, router generation, controller adapters |
| later | — | Configuration binding (`#[configuration]`, `#[property]`) |
| later | — | Bean factories (`#[bean] async fn`) |
| later | — | Test container with mock substitution |
| later | — | `#[transactional]`, `#[scheduled]`, `#[retry]` via code gen |

---

## Design Constraints

- Source files must remain valid Rust — no custom syntax, no preprocessing step that breaks `rustfmt` or `clippy`.
- rust-analyzer must work without modification or a custom LSP.
- No new runtime, no garbage collector, no JVM-style container at runtime.
- Generated output is ordinary Rust — readable, diffable, auditable.
- `Arc<T>` and runtime indirection are acceptable trade-offs for developer experience.
- Target is backend web applications (Axum), not embedded or performance-critical systems.
