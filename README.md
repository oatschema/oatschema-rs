# oatschema

[![crates.io](https://img.shields.io/crates/v/oatschema.svg)](https://crates.io/crates/oatschema)
[![docs.rs](https://img.shields.io/docsrs/oatschema)](https://docs.rs/oatschema)

The OATS (Open Annotation Task Schemas) JSON schemas, bundled for Rust: strict, versioned schemas for the outputs of machine learning models, workflows and human annotation. OATS is unrelated to the W3C Web Annotation data model, formerly Open Annotation.

Schemas, documentation and the changelog live at [oatschema.org](https://oatschema.org). The schemas are maintained in [oatschema/oats](https://github.com/oatschema/oats); this crate bundles them unchanged. There are sibling packages for [Python](https://pypi.org/project/oatschema/) ([source](https://github.com/oatschema/oatschema-py)) and [JavaScript](https://www.npmjs.com/package/oatschema) ([source](https://github.com/oatschema/oatschema-js)).

## Install

```
cargo add oatschema                       # schemas only, no dependencies
cargo add oatschema --features serde_json # adds schema(), returning a serde_json::Value
```

The minimum supported Rust version is 1.85.

## Usage

```rust
use oatschema::{SchemaName, schema_json};

let text = schema_json(SchemaName::Classification, None)?;  // the schema file exactly as released
let pinned = schema_json(SchemaName::Ranking, Some("0.8.0"))?;
let name: SchemaName = "llm-output".parse()?;  // or SchemaName::LlmOutput
```

With the `serde_json` feature:

```rust
let schema = oatschema::schema(oatschema::SchemaName::Classification, None)?;  // a serde_json::Value
assert_eq!(schema["x-version"], oatschema::SCHEMA_VERSION);
```

- Every schema is compiled into the crate with `include_str!`. Nothing touches the network or the filesystem.
- `schema_json` returns a `&'static str` that is byte-for-byte the released file, so you can hand it to any JSON Schema validator or tool. `schema` parses a new `serde_json::Value` on every call, so you can change it without affecting other callers.
- Lookups return `oatschema::Error`. `Error::UnknownName` comes from parsing a name that isn't a schema in any version. `Error::UnknownVersion` means the version isn't bundled, and `Error::NotInVersion` means the version is bundled but doesn't contain that schema.

## Feature flags

| Feature      | Default | Adds                                              | Dependencies |
| ------------ | ------- | ------------------------------------------------- | ------------ |
| `serde_json` | no      | `schema()`, which returns a `serde_json::Value`   | `serde_json` |

Validation is planned for 0.1.0, behind a `validate` feature.

## Schema versions

Every lookup takes a `version`, which defaults to `oatschema::SCHEMA_VERSION`, the newest bundled version. Versions must be exact, such as `"0.8.0"`; `"0.8"` and `"latest"` are rejected.

`oatschema::available_versions()` lists every bundled version, oldest first. The crate bundles OATS 0.8.0 and every later release; today that is 0.8.0 alone. Before 1.0.0, a schema version may reject records that an older one accepted, so validate a record against the version it was written for.

Each schema states its version in its `x-version` keyword, and its `$id` includes the version (`https://oatschema.org/0.8.0/classification`), so versions can sit side by side in one JSON Schema registry. Releases before 0.8.0 are only bundled by the [Python package](https://pypi.org/project/oatschema/).

## Versioning policy

The crate follows its own semantic versioning, independent of the schema versions. Its version never encodes a schema version: `oatschema::SCHEMA_VERSION` tells you which OATS release is the default.

- A new OATS release is bundled in a new minor version before 1.0 (for example 0.1.x to 0.2.0), because it changes the default schema and adds `SchemaName` variants.
- Every release keeps every schema version it bundled before, so upgrading never removes a version you validate against.
- `SchemaName` and `Error` are `#[non_exhaustive]`, so new schemas and error cases don't break your `match`es.
- Raising the minimum supported Rust version is a minor-version change.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development commands and how a new OATS release is bundled.

## License

Apache-2.0. See [LICENSE](LICENSE). Copyright The OATS Authors, listed in [AUTHORS](AUTHORS).
