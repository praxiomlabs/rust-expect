# rust-expect-macros

Procedural macros for the rust-expect terminal automation library.

## Macros

### `patterns!`

Create multiple patterns at once:

```rust
use rust_expect_macros::patterns;

let patterns = patterns![
    "login:",
    "password:",
    r"\$ $",
];
```

### `dialog!`

Define dialog flows declaratively:

```rust
use rust_expect_macros::dialog;

let login_dialog = dialog! {
    expect "login: " => send "admin\n",
    expect "password: " => send "secret\n",
    expect "$ ",
};
```

### `regex!`

Compile-time verified regex patterns:

```rust
use rust_expect_macros::regex;

let pattern = regex!(r"\d{3}-\d{4}");
```

### `timeout!`

Human-readable duration syntax:

```rust
use rust_expect_macros::timeout;

let duration = timeout!(5 seconds);
let short = timeout!(500 ms);
```

## License

Licensed under MIT or Apache-2.0.
