# the-dispatch

A zero-cost, generic dispatch system for building overridable, composable behavior graphs.

## Why This Exists

Most applications hard-code their control flow:

```rust
fn handle_key(editor: &mut Editor, key: Key) {
    let action = key_to_action(key);  // hard-coded
    execute_action(editor, action);   // hard-coded
    render(editor);                   // hard-coded
}
```

This makes customization painful. Want to intercept keys before processing? Wrap actions with logging? Skip rendering for batch operations? You're fighting the code.

`the-dispatch` flips this around. Instead of hard-coded function calls, you define **dispatch points** — named slots where behavior is injected:

```rust
define! {
    Editor {
        on_key: Key,
        on_action: Action,
        render: (),
    }
}
```

Now behavior is **configuration**, not code:

```rust
let dispatch = EditorDispatch::new()
    .with_on_key(my_key_handler)
    .with_on_action(my_action_handler)
    .with_render(my_render_handler);
```

Different configurations = different behaviors. Same core, different personalities.

## Core Concepts

### Dispatch Points

A dispatch point is a named entry where handlers are invoked. Define them with the `define!` macro:

```rust
use the_dispatch::define;

define! {
    Calculator {
        parse: String => Option<Expr>,   // input => output
        add: (i64, i64) => i64,
        sub: (i64, i64) => i64,
        render: (),                       // no output = ()
    }
}
```

This generates:
- `CalculatorDispatch<Ctx, ...>` — the dispatch struct with generic handler slots
- `CalculatorApi<Ctx>` — a trait for ergonomic bounds without exposing handler generics
- `.with_*()` builder methods for each dispatch point

### Handlers

Handlers are functions or closures that receive a context and input:

```rust
fn add_handler(ctx: &mut CalcCtx, (a, b): (i64, i64)) -> i64 {
    a + b
}

// Or as a closure
.with_add(|ctx, (a, b)| a + b)
```

Handlers are **statically dispatched** — zero virtual call overhead by default.

### External Orchestration

Handlers do **one thing**. Chaining multiple dispatch points together is done externally:

```rust
// Handlers are simple
.with_parse(|ctx, input| parse_expr(&input))
.with_add(|ctx, (a, b)| a + b)

// Orchestration is external
fn eval(dispatch: &impl CalculatorApi<Ctx>, ctx: &mut Ctx, input: &str) -> Option<i64> {
    let expr = dispatch.parse(ctx, input.to_string())?;
    match expr.op {
        Op::Add => Some(dispatch.add(ctx, (expr.left, expr.right))),
        Op::Sub => Some(dispatch.sub(ctx, (expr.left, expr.right))),
    }
}
```

This design avoids borrow checker complexity and keeps handlers testable in isolation.

## Usage

### Basic Example

```rust
use the_dispatch::define;

define! {
    Greeter {
        greet: String => String,
    }
}

fn main() {
    let dispatch = GreeterDispatch::<(), _>::new()
        .with_greet(|_ctx, name| format!("Hello, {name}!"));

    let mut ctx = ();
    let message = dispatch.greet(&mut ctx, "world".to_string());
    println!("{message}");  // "Hello, world!"
}
```

### Flow with Multiple Dispatch Points

```rust
define! {
    Pipeline {
        pre_process: String => String,
        process: String => String,
        post_process: String => String,
    }
}

let dispatch = PipelineDispatch::<Ctx, _, _, _>::new()
    .with_pre_process(|ctx, s| s.trim().to_string())
    .with_process(|ctx, s| s.to_uppercase())
    .with_post_process(|ctx, s| format!("[{s}]"));

// Orchestrate the flow
let mut ctx = Ctx::default();
let input = "  hello  ".to_string();
let result = dispatch.pre_process(&mut ctx, input);
let result = dispatch.process(&mut ctx, result);
let result = dispatch.post_process(&mut ctx, result);
assert_eq!(result, "[HELLO]");
```

### Using the Generated Trait

The `*Api` trait lets you write generic code without the handler type parameters:

```rust
fn run_pipeline<Ctx>(dispatch: &impl PipelineApi<Ctx>, ctx: &mut Ctx, input: String) -> String {
    let result = dispatch.pre_process(ctx, input);
    let result = dispatch.process(ctx, result);
    dispatch.post_process(ctx, result)
}
```

## Features

### `cow-handlers`

Store handlers behind `Arc` for cheap cloning. Useful when you want to derive variations from a base dispatch:

```toml
[dependencies]
the-dispatch = { version = "0.1", features = ["cow-handlers"] }
```

```rust
let base = EditorDispatch::new()
    .with_on_key(default_key_handler);

// Clone and override one handler
let vim_mode = base.clone()
    .with_on_key(vim_key_handler);
```

Without `cow-handlers`, the dispatch is not `Clone`.

### `dynamic-registry`

Add a string-keyed registry for runtime handler lookup. Useful for scripting:

```toml
[dependencies]
the-dispatch = { version = "0.1", features = ["dynamic-registry"] }
```

```rust
use std::sync::Arc;
use the_dispatch::{DynHandler, DynValue};

let mut dispatch = EditorDispatch::new();

// Register a dynamic handler
let handler: DynHandler<Ctx> = Arc::new(|ctx, input| {
    // input is Box<dyn Any>, downcast as needed
    let val = input.downcast::<i64>().unwrap();
    Box::new(*val + 1) as DynValue
});
dispatch.registry_mut().set("post_eval", handler);

// Look up and call
if let Some(handler) = dispatch.registry().get("post_eval") {
    let result = handler(&mut ctx, Box::new(42i64));
}
```

Dynamic handlers are **opt-in** and **never** on the hot path unless you explicitly call them.

## Design Philosophy

### Not a Pipeline

This is **not** middleware. There's no implicit "next" to call, no forced ordering, no `ControlFlow` return type. Handlers are independent. You decide what calls what.

### Not an Event System

Dispatch points are **synchronous, typed, direct calls**. No event queues, no subscribers, no broadcast. One handler per dispatch point (though you can compose handlers).

### Compile-Time by Default

All handlers are generic type parameters, statically dispatched. The dynamic registry is opt-in for when you genuinely need runtime flexibility.

### Handlers Are Pure-ish

Handlers receive `(&mut Ctx, Input)` and return `Output`. They can mutate context but shouldn't have hidden side effects. This makes them testable and predictable.

## Examples

See the `examples/` directory:

- **`hello_world.rs`** — minimal single dispatch point
- **`hello_flow.rs`** — chaining multiple dispatch points
- **`calculator.rs`** — full example with dynamic registry

Run with:

```sh
cargo run --example hello_world
cargo run --example hello_flow
cargo run --example calculator
cargo run --example calculator --features dynamic-registry
```

## API Reference

### `define!` Macro

```rust
define! {
    Name {
        point_name: InputType => OutputType,
        another_point: InputType,  // OutputType defaults to ()
    }
}
```

Generates:
- `NameDispatch<Ctx, Handler1, Handler2, ...>` — dispatch struct
- `NameApi<Ctx>` — trait with dispatch methods
- `NameDispatch::new()` — constructor with no-op default handlers
- `.with_point_name(handler)` — builder methods

### `HandlerFn` Trait

```rust
pub trait HandlerFn<Ctx, Input, Output> {
    fn call(&self, ctx: &mut Ctx, input: Input) -> Output;
}
```

Implemented for `Fn(&mut Ctx, Input) -> Output` and (with `cow-handlers`) `Arc<F>`.

### `DispatchRegistry<Ctx>` (with `dynamic-registry`)

```rust
impl<Ctx> DispatchRegistry<Ctx> {
    pub fn new() -> Self;
    pub fn set(&mut self, name: &'static str, handler: DynHandler<Ctx>);
    pub fn get(&self, name: &'static str) -> Option<&DynHandler<Ctx>>;
    pub fn remove(&mut self, name: &'static str) -> Option<DynHandler<Ctx>>;
}
```
