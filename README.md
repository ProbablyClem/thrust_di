# Thrust

A Spring-Boot-like build-time code generator for Rust/Axum backends.

Developers write annotated Rust structs and functions. A build step scans the source, extracts metadata, and generates ordinary Rust code — dependency wiring, a DI container, the Axum router, server startup — before `cargo` compiles anything. The full Rust toolchain (rust-analyzer, clippy, cargo check) works on the source unchanged.

---

## Why

Rust backend development requires substantial boilerplate: manual `Arc` wiring, `AppState` construction, router registration, constructor chaining. Existing DI frameworks often fight the borrow checker directly. Thrust sidesteps this by treating ownership as an implementation detail hidden behind generated code — you declare *what* depends on *what*, and the generated container does the wiring.

---

## Architecture

Three approaches were considered:

**New language** (`*.rux` files) — rejected. Would require a parser, type checker, formatter, LSP, and cargo integration. Multi-year effort, zero ecosystem compatibility.

**Proc-macros only** — viable for single-struct transforms, but cross-module dependency graph analysis is hard to implement cleanly as a proc-macro. Validation errors are difficult to surface.

**Build-time code generator (chosen)** — developers write normal Rust with framework attributes. A `build.rs` script runs before compilation, parses the source with `syn`, builds a project-wide metadata model, and writes generated Rust into `OUT_DIR`. `cargo build` then compiles everything together.

```
Annotated Rust source
        ↓
   build.rs → thrust_build::scan_and_generate (syn + quote)
        ↓
   OUT_DIR/generated.rs  (container, router, aliases, metadata)
        ↓
   thrust_macros::init!()  includes the generated code
        ↓
   cargo build → final binary
```

Source files remain valid Rust at all times. rust-analyzer sees the real structs, not generated proxies. The proc-macro attributes (`#[service]`, `#[get]`, `#[bean]`, …) are near pass-through markers; the actual project-wide analysis happens in `build.rs` via `syn`, not at macro-expansion time.

---

## Workspace Structure

```
thrust/
├── Cargo.toml          # workspace manifest
├── thrust-macros/      # proc-macro crate — attribute markers + init!()
├── thrust-build/       # the scanner + code generator (a build-dependency)
│   └── src/
│       ├── lib.rs      # scan_and_generate orchestrator
│       ├── scanner.rs  # syn AST walkers (services, beans, layers, routes, impls)
│       ├── graph.rs    # dep graph, validation, cycle detection, topo sort, trait resolution
│       ├── codegen.rs  # container / router / run() / alias generation
│       ├── models.rs   # ComponentInfo, BeanInfo, RouteInfo, …
│       └── utils.rs    # Arc unwrapping, naming, module-path derivation
├── thrust-core/        # internal app used to exercise the scanner
└── examples/
    ├── basic/          # DI container only, no web server
    ├── basic-axum/     # services, trait DI, routes, config bean
    ├── config-bean/    # #[bean] + #[layer] (Arc<T> bean form)
    └── inspect/        # prints the generated component/dependency graph
```

Each consumer crate has a three-line `build.rs`:

```rust
fn main() {
    let src = std::path::Path::new("src");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    thrust_build::scan_and_generate(src, &out);
}
```

…lists `thrust-build` under `[build-dependencies]` and `thrust-macros` under `[dependencies]`, and calls `thrust_macros::init!()` once at the crate root.

---

## Features

### Services & dependency injection

Annotate a struct with `#[service]` and declare dependencies as fields. Thrust builds the dependency graph, validates it (missing deps and cycles are build errors), topologically sorts it, and generates a `Container` that constructs everything in order.

```rust
use thrust_macros::service;

#[service]
pub struct GreetingService;

#[service]
pub struct UserService {
    pub greeting: Arc<GreetingService>,   // injected automatically
}
```

`Arc` is the wiring currency: the container stores every component as `Arc<T>` and clones the handle into each dependent.

### Dependency inversion with static dispatch

Depend on a trait, not a concrete type. Write the field as a **bare `dyn Trait`** and thrust resolves it to the single `#[service]` that implements the trait, at build time — compiling it to `Arc<ConcreteImpl>` with **no trait object, no vtable**:

```rust
pub trait TodoRepository {
    fn all(&self) -> Vec<&'static str>;
}

#[service]
pub struct InMemoryTodoRepository;
impl TodoRepository for InMemoryTodoRepository { /* … */ }

#[service]
pub struct TodoService {
    pub repo: dyn TodoRepository,   // → resolved to Arc<InMemoryTodoRepository>
}
```

(More than one impl of the trait is an ambiguity error.) If you prefer dynamic dispatch, write the field as an explicit `Arc<dyn TodoRepository + Send + Sync>` — thrust leaves that form untouched.

### Routes & handler injection

Annotate `async fn` handlers with `#[get("/path")]`, `#[post]`, `#[put]`, `#[delete]`, `#[patch]`. Declare the services a handler needs as `Arc<T>` parameters — thrust generates wrappers that pull them from the container and a `build_router(Arc<Container>) -> axum::Router`:

```rust
#[get("/todos")]
pub async fn list_todos(svc: Arc<TodoService>) -> impl IntoResponse {
    Json(json!({ "todos": svc.find_all() }))
}
```

### Beans (factory components)

For components that need custom construction (pools, clients, config), use a `#[bean]` factory function. Its parameters are injected like any other dependency. **Return a bare `T` and thrust wraps it in `Arc` for you** (an explicit `Arc<T>` return also works):

```rust
#[bean]
pub fn db_pool() -> DbPool {
    DbPool::connect("postgres://localhost/app")
}
```

`async fn` beans are awaited during container construction (`Container::build()` becomes `async` automatically).

### Layers

Apply Tower/Axum middleware with `#[layer]`. The function returns a layer and thrust appends it to the generated router:

```rust
#[layer]
pub fn request_tracing() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}
```

### Server startup & configuration

When routes exist, thrust generates a `run()` that builds the container, binds, and serves. Configure the bind address **in code** by declaring a `#[bean]` that returns `ServerConfig` — thrust generates the `ServerConfig` type for you (`Default` = `0.0.0.0`, `PORT` env var or `8080`):

```rust
use crate::ServerConfig;

#[bean]
pub fn server_config() -> ServerConfig {
    ServerConfig { port: 3000, ..Default::default() }
}
```

```rust
// main.rs — that's the whole entrypoint
#[tokio::main]
async fn main() {
    run().await;
}
```

Without a `server_config` bean, `run()` falls back to `ServerConfig::default()`.

---

## Quick Start

```bash
cargo run -p basic-axum
# thrust: listening on http://0.0.0.0:3000

curl localhost:3000/todos
# {"todos":["buy milk","write tests"]}
```

Inspect the generated dependency graph for an example:

```bash
cargo run -p inspect
```

---

## Status

The core pipeline is implemented end to end:

| Area                         | Status | Notes                                                              |
| ---------------------------- | ------ | ------------------------------------------------------------------ |
| Component discovery          | ✓      | `#[service]` structs                                               |
| Dependency extraction        | ✓      | struct fields, bean params, route params                          |
| Graph build / validation     | ✓      | missing-dep and cycle detection are build errors                  |
| Topological ordering         | ✓      | unified order across services and beans                          |
| Container + `build()`        | ✓      | sync or `async` depending on beans                               |
| Trait DI                     | ✓      | static dispatch via build-time aliases, or explicit `Arc<dyn _>`  |
| Axum routes + router         | ✓      | handler injection, `build_router`                                |
| Beans / layers               | ✓      | `#[bean]` (bare `T` or `Arc<T>`), `#[layer]`                      |
| Server `run()` + config bean | ✓      | `ServerConfig` overridable in code                               |
| Property/config binding      | —      | planned                                                           |
| Test container / mocks       | —      | planned                                                           |
| `#[transactional]`, `#[scheduled]`, `#[retry]` | — | planned                                              |

---

## Design Constraints

- Source files must remain valid Rust — no custom syntax, no preprocessing step that breaks `rustfmt` or `clippy`.
- rust-analyzer must work without modification or a custom LSP.
- No new runtime, no garbage collector, no JVM-style container at runtime.
- Generated output is ordinary Rust — readable, diffable, auditable.
- `Arc<T>` and runtime indirection are acceptable trade-offs for developer experience.
- Target is backend web applications (Axum), not embedded or performance-critical systems.
