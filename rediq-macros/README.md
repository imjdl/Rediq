# rediq-macros

Proc macros for [Rediq](https://github.com/imjdl/rediq) - a distributed task queue framework for Rust.

This crate provides attribute macros to simplify handler registration in Rediq.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rediq = { version = "0.2", features = ["macros"] }
```

## Macros

### `#[task_handler]`

Transforms an async function into a `Handler` implementation:

```rust
use rediq::{Task, Result};
use rediq_macros::task_handler;

#[task_handler]
async fn send_email(task: &Task) -> Result<()> {
    let payload: EmailData = task.payload_json()?;
    // Process email...
    Ok(())
}
```

### `register_handlers!`

Batch register multiple handlers:

```rust
use rediq_macros::{task_handler, register_handlers};

#[task_handler]
async fn send_email(task: &Task) -> Result<()> { /* ... */ }

#[task_handler]
async fn send_sms(task: &Task) -> Result<()> { /* ... */ }

let mux = register_handlers!(
    "email:send" => send_email,
    "sms:send" => send_sms,
);
```

### `handler_fn!`

Create a handler from a closure or function:

```rust
use rediq_macros::handler_fn;

let handler = handler_fn!(async fn(task: &Task) -> rediq::Result<()> {
    println!("Processing: {}", task.id);
    Ok(())
});
```

### `def_handler!`

Define and register a handler inline:

```rust
use rediq_macros::def_handler;

let handler = def_handler!(async fn my_handler(task: &Task) -> rediq::Result<()> {
    // Process task...
    Ok(())
});
```

## Documentation

For more details, see:

- [Rediq GitHub Repository](https://github.com/imjdl/rediq)
- [Rediq Documentation](https://docs.rs/rediq)

## License

Licensed under either of

- Apache License, Version 2.0
- MIT license

at your option.
