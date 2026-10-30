# oatschema

[![crates.io](https://img.shields.io/crates/v/oatschema.svg)](https://crates.io/crates/oatschema)
[![docs.rs](https://img.shields.io/docsrs/oatschema)](https://docs.rs/oatschema)
[![MSRV](https://img.shields.io/crates/msrv/oatschema)](https://crates.io/crates/oatschema)
[![CI](https://github.com/oatschema/oatschema-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/oatschema/oatschema-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/oatschema/oatschema-rs/blob/main/LICENSE)

The [OATS](https://oatschema.org) (Open Annotation Task Schemas) JSON schemas for Rust.

OATS is a collection of strict, versioned JSON Schema 2020-12 schemas, defining and validating the structured outputs of machine learning models, workflow steps and annotation. Models, pipelines and people write the same JSON for each task, so a classification, a transcription or a set of detected objects has one shape, whatever produced it. This crate bundles the schemas so you can validate those records anywhere. (OATS is unrelated to the W3C Web Annotation data model, formerly Open Annotation.)

- **No dependencies, no I/O.** Every schema is compiled into the crate with `include_str!`, so nothing touches the network or the filesystem, at build time or at run time. `serde_json` is an optional feature.
- **Exactly as released.** Each schema is byte-identical to its [oatschema/oats](https://github.com/oatschema/oats) release, and the crate's tests check every file against a checksum manifest.
- **Typed.** Schema names are a `SchemaName` enum, so typos fail at compile time.

## Quick start

```sh
cargo add oatschema
```

```rust
use oatschema::{SchemaName, schema_json};

fn main() -> Result<(), oatschema::Error> {
    let text = schema_json(SchemaName::Classification, None)?;  // the schema file, exactly as released
    assert!(text.contains(r#""$id": "https://oatschema.org/0.8.0/classification""#));
    assert_eq!(oatschema::SCHEMA_VERSION, "0.8.0");  // used when you don't pass a version

    let name: SchemaName = "llm-output".parse()?;  // or SchemaName::LlmOutput
    let pinned = schema_json(name, Some("0.8.0"))?;
    assert!(pinned.contains(r#""x-version": "0.8.0""#));
    Ok(())
}
```

Requires Rust 1.85 or later. With `cargo add oatschema --features serde_json`, `oatschema::schema` returns a `serde_json::Value` instead of text.

## Schemas

| Name | Holds |
| :--- | :--- |
| `audio-transcription` | Transcribed speech, with timed segments, speakers and non-verbal events |
| `classification` | Predicted labels with scores and an optional vocabulary |
| `document-extraction` | Document layout and content: text blocks, coordinates and pages |
| `embedding` | A dense or sparse vector and the model that produced it |
| `entity-extraction` | Entities linked to their evidence, and relations between them |
| `generic` | Arbitrary key-value data |
| `geolocation` | A geographic feature, as a strict subset of GeoJSON |
| `llm-output` | One prompt and response, with generation parameters and usage |
| `object-detection` | Boxes, polygons and their hierarchy within a source |
| `ranking` | Ranked results for a query, such as search or retrieval hits |
| `regression` | Numeric predictions, intervals and forecasts |
| `temporal-segmentation` | Labelled time intervals, such as chapters, scenes or speaker turns |

Browse every schema, its fields and the changelog at [oatschema.org](https://oatschema.org).

## Validating records

This release has no validator of its own; pass a schema to any JSON Schema 2020-12 validator. `x-license` and `x-version` are the only non-standard keywords, and 2020-12 validators treat them as annotations. With the [jsonschema](https://crates.io/crates/jsonschema) crate and the `serde_json` feature:

```rust
use oatschema::SchemaName;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = oatschema::schema(SchemaName::Classification, None)?;
    let validator = jsonschema::options().should_validate_formats(true).build(&schema)?;

    assert!(validator.is_valid(&json!({"labels": [{"label": "cat", "score": 0.92}]})));

    let record = json!({"labels": [{"label": "cat", "score": 1.5}]});
    let errors: Vec<String> = validator
        .iter_errors(&record)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert_eq!(errors, ["/labels/0/score: 1.5 is greater than the maximum of 1"]);
    Ok(())
}
```

Build each validator once and reuse it. Turn on format checking (`uri`, `date-time` and so on), as above, to match the OATS packages for other languages.

## API

| Item | Description |
| :--- | :--- |
| `schema_json(name, version)` | The schema file's text, exactly as released, as a `&'static str` |
| `schema(name, version)` | The schema as a new `serde_json::Value` on every call. Needs the `serde_json` feature |
| `available_versions()` | Every bundled schema version, oldest first |
| `SCHEMA_VERSION` | The default schema version, the newest bundled |
| `SchemaName` | The schema names, with `ALL`, `as_str`, `Display` and `FromStr` |
| `Error` | `UnknownName`, `UnknownVersion` or `NotInVersion` |

- `version` is an `Option<&str>`; `None` means `SCHEMA_VERSION`. It must be exact: `"0.8.0"`, never `"0.8"` or `"latest"`.
- Parsing a name that exists in no bundled version gives `Error::UnknownName`. A version that isn't bundled gives `Error::UnknownVersion`, and one that lacks the schema gives `Error::NotInVersion`.
- `SchemaName` and `Error` are `#[non_exhaustive]`, so new schemas and error cases don't break your `match`es.

| Feature | Default | Adds |
| :--- | :--- | :--- |
| `serde_json` | no | `schema()`, returning a `serde_json::Value` |

## Versions

**Schema versions.** Each schema states its release in `x-version`, and its `$id` includes it (`https://oatschema.org/0.8.0/classification`), so several versions can share one validator registry. Before OATS 1.0, a new release may reject records an older one accepted: pass the version a record was written for. This crate bundles OATS 0.8.0, the first public release, and every later one. For 0.4.0 to 0.7.0, use [oatschema-py](https://github.com/oatschema/oatschema-py).

**Crate versions.** The crate has its own semantic version, which never encodes a schema version. Upgrading never removes a schema version. Before 1.0, a release that bundles a new OATS release, and so changes the default `SCHEMA_VERSION`, is a minor bump, so Cargo's default `0.x.y` requirement keeps your default fixed. Raising the minimum supported Rust version is also a minor bump.

## Other languages

- Python: [`oatschema` on PyPI](https://pypi.org/project/oatschema/) ([oatschema-py](https://github.com/oatschema/oatschema-py)), which also bundles 0.4.0 to 0.7.0 and validates with jsonschema-rs.
- JavaScript and TypeScript: [`oatschema` on npm](https://www.npmjs.com/package/oatschema) ([oatschema-js](https://github.com/oatschema/oatschema-js)).

## Contributing

Schema changes belong in [oatschema/oats](https://github.com/oatschema/oats). For this crate, see [CONTRIBUTING.md](https://github.com/oatschema/oatschema-rs/blob/main/CONTRIBUTING.md). Commits need a DCO sign-off (`git commit -s`).

## License

Apache-2.0. Copyright [The OATS Authors](https://github.com/oatschema/oatschema-rs/blob/main/AUTHORS).
