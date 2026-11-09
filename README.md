# jstrict

[![Crate](https://img.shields.io/crates/v/jstrict.svg?style=flat-square)](https://crates.io/crates/jstrict)
[![Docs](https://img.shields.io/docsrs/jstrict?style=flat-square)](https://docs.rs/jstrict)
[![MSRV](https://img.shields.io/crates/msrv/jstrict?style=flat-square)](https://crates.io/crates/jstrict)
[![License](https://img.shields.io/crates/l/jstrict.svg?style=flat-square)](#license)

A strict [RFC 8259](https://datatracker.ietf.org/doc/html/rfc8259) / [ECMA-404](https://www.ecma-international.org/publications-and-standards/standards/ecma-404/) JSON parser for Rust that keeps what the standard leaves in the document — entry order, duplicate keys, number precision — and tells you where every value came from.

```rust
use jstrict::{Parse, Print, Value};

let (value, code_map) = Value::parse_str(r#"{ "a": 0, "b": [1, 2] }"#)?;

// Fragment 3 is the `0` bound to "a" — byte 7 of the source.
assert_eq!(code_map[3].span.start, 7);

println!("{}", value.pretty_print());
```

When you don't need the spans, take the fast path — it skips span tracking entirely:

```rust
let value = jstrict::parse::parse_str_value("[1, 2, 3]")?;
```

Build values from a literal with `json!`:

```rust
use jstrict::json;

let value = json!({
    "code": 200,
    "features": ["spans", "duplicates", "precision"]
});
```

[`jstrict` and what JSON leaves open](https://andromeda.skvarc.com/docs/libraries/jstrict/), on the Andromeda documentation site, explains why a JSON-LD processor needs a parser that keeps all of this.

---

## Why this fork

Fork of [`json-syntax`](https://crates.io/crates/json-syntax) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/json-syntax). Semantics are unchanged — same strictness, same `Value`, same `CodeMap`. This fork adds:

- A byte-slice parser behind `parse_str` / `parse_slice`, replacing the char-iterator front end: strings and numbers are scanned byte-wise, un-escaped runs are copied as slices instead of pushed per char, and `null` / `true` / `false` are matched with a single 4/5-byte compare.
- SWAR scanners (`u64`-at-a-time) for control and escape bytes, with a `memchr`-accelerated `"` / `\` scan in the string path.
- SIMD UTF-8 validation via [`simdutf8`](https://crates.io/crates/simdutf8) (default-on) and SIMD string escaping via [`json-escape-simd`](https://crates.io/crates/json-escape-simd) when printing.
- `parse::parse_str_value` / `parse::parse_slice_value`, which skip the code map. The parser is generic over its span recorder, so the no-op path monomorphises away rather than being branched around.
- Object key index rebuilt on `hashbrown::HashTable` with cached key hashes, so growing the table does not re-hash.
- [`json-number`](https://crates.io/crates/json-number) folded in as the `number` module; number formatting and parsing go through [`lexical`](https://crates.io/crates/lexical).
- Conversion from/to `sonic_rs::Value`, alongside the existing `serde_json::Value` conversion.
- Criterion bench suite under `benches/` — parse, print, access, object, serde, convert, canonicalize, numbers, strings, macros, end-to-end.
- Rust 2024 edition, MSRV 1.96.
- Renamed crate: `json-syntax` → `jstrict`.

Credit and history preserved — see [Attribution](#attribution).

## Install

```sh
cargo add jstrict
```

## Feature flags

| Flag           | Default | Enables                                                                          |
| -------------- | :-----: | -------------------------------------------------------------------------------- |
| `simdutf8`     |   yes   | SIMD-accelerated UTF-8 validation in the slice parser                            |
| `canonicalize` |         | JSON Canonicalization Scheme, [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) |
| `serde`        |         | `Serialize` / `Deserialize`, plus `to_value` / `from_value`                      |
| `serde_json`   |         | `Value::from_serde_json` / `Value::into_serde_json`                              |
| `sonic-rs`     |         | `Value::from_sonic_rs` / `Value::into_sonic_rs`                                  |
| `contextual`   |         | `contextual::WithContext` printing adapters                                      |

## Code maps

Parsing returns a `CodeMap` alongside the value: a flat list of entries, one per *fragment* (a value, an object entry, or an object key), in the same depth-first order as `Value::traverse`. Each entry carries the fragment's byte span and its volume — the number of fragments it contains, itself included — so a subtree can be skipped in one step.

Nothing about the source layout is stored inside the value tree, so `Value` stays cheap to clone and compare. `Object::iter_mapped` and `JsonArray::iter_mapped` walk a value and its code map together, and `TryFromJson` uses that to report conversion errors against a span.

## Duplicates and ordering

A JSON object here is a *list* of entries, not a map. Duplicate keys are preserved exactly as written, and `{ "a": 0, "b": 1 }` is **not** equal to `{ "b": 1, "a": 0 }` under the derived `PartialEq`. An internal index still gives `O(1)` average lookup.

- `get` / `get_entries` return an iterator over *every* match.
- `get_unique` and friends return `Err(Duplicate(..))` when a key matches more than once.
- `insert` replaces the first match and removes the rest; `push` appends without touching duplicates.
- For order-insensitive comparison, wrap in `Unordered` or call `as_unordered()`.

## Numbers

Numbers are stored in their lexical form, so `1E30` round-trips as `1E30` and precision is never silently lost. `Number` exposes range-checked `as_i64` / `as_u64`, explicitly lossy `as_f64_lossy`, and `as_f64_lossless`, which returns `None` if the conversion would not round-trip.

With the `canonicalize` feature, `Value::canonicalize` rewrites a value into the JSON Canonicalization Scheme form of [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785): keys sorted by their UTF-16 code units, numbers in shortest round-trip form. Use `canonicalize_with` in tight loops to reuse the float buffer.

## Printing

```rust
use jstrict::{Parse, Print, Value};

let value = Value::parse_str(r#"[0, 1, { "key": "value" }]"#)?.0;

println!("{}", value.compact_print()); // one line, no spaces
println!("{}", value.inline_print());  // one line, with spaces
println!("{}", value.pretty_print());  // multi-line, 2-space indent

let mut options = jstrict::print::Options::pretty();
options.indent = jstrict::print::Indent::Tabs(1);
println!("{}", value.print_with(options));
```

`Options` also controls the padding around every delimiter and the width or item count at which an array or object is expanded onto several lines.

## Benchmarks

```sh
cargo bench
cargo bench --bench parse
cargo bench --all-features --bench convert
```

Criterion output: `target/criterion/`.

## Examples

```sh
cargo run --features serde --example serde
cargo run --features serde_json --example serde_json
cargo run --features sonic-rs --example sonic_rs
```

## MSRV

Rust 1.96 (edition 2024).

## Attribution

Original crate: [`json-syntax`](https://crates.io/crates/json-syntax) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/json-syntax). Upstream commits are preserved in this repo's history under their original authorship. This fork is a layer of performance work and interop on top of their design.

The test corpus is derived from [JSONTestSuite](https://github.com/nst/JSONTestSuite) by Nicolas Seriot.

## License

Dual-licensed, same as upstream. Pick whichever fits:

- [Apache-2.0](LICENSE-APACHE)
- [MIT](LICENSE-MIT)
