# rust-expect-macros

Procedural macros for the rust-expect terminal automation library.

Import them from `rust_expect`, which re-exports all four. The expansions name
types through `::rust_expect::`, so using this crate on its own will not
compile.

## Macros

### `patterns!`

Build a `PatternSet`, braced or bare:

```rust
use rust_expect::patterns;

let set = patterns! {
    login: "login:",
    password: "password:",
    prompt: regex(r"\$\s*$"),
    source: glob("*.rs"),
};
```

A bare name before `:` names the pattern in the set. `regex(..)`/`re(..)` are
validated at compile time; `glob(..)` and plain string literals are taken
literally.

`"pattern" => action` is rejected: a `PatternSet` holds patterns and has no
handler slot. Register handlers with `Session::pattern_manager_mut()` and
`PersistentPattern`.

### `dialog!`

Define dialog flows declaratively. Steps are separated by `;`:

```rust
use rust_expect::dialog;
use std::time::Duration;

let login_dialog = dialog! {
    timeout Duration::from_secs(30);
    expect "login: ";
    sendln "admin";
    expect "password: ";
    sendln "secret";
    expect "$ ", Duration::from_secs(5);
};
```

`timeout` sets the timeout for every expectation after it; a step's own
`, duration` wins. `sendln` appends a bare LF, not the session's configured
line ending.

`expect_re`/`expect_regex` and `wait`/`sleep` are rejected: dialog steps match
literally and a dialog has no timing of its own. Match a regex with
`session.expect(Pattern::regex(..))`, and sleep around `run_dialog`.

### `regex!`

Compile-time verified regex patterns, compiled once per call site:

```rust
use rust_expect::regex;

let pattern = regex!(r"\d{3}-\d{4}");
```

### `timeout!`

Human-readable duration syntax. Compound values are joined with `+`:

```rust
use rust_expect::timeout;

let duration = timeout!(5 seconds);
let short = timeout!(500 ms);
let compound = timeout!(1 m + 30 s);
```

## License

Licensed under MIT or Apache-2.0.
