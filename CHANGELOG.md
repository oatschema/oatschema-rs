# Changelog

All notable changes to this crate are recorded here. The crate follows [semantic versioning](https://semver.org), independent of the OATS schema versions; see "Versions" in the README.

## 0.0.2 (2026-10-30)

- Fixed the README's validation example, which no longer compiled with current versions of the `jsonschema` crate (`instance_path` is now a method). Every Rust example in the README now runs as a doctest.
- The release workflow skips a version that is already on crates.io, instead of failing.
- No API or schema changes; still bundles OATS 0.8.0.

## 0.0.1 (2026-10-18)

First release.

- Bundles OATS 0.8.0 (12 schemas, including `ranking`), copied unchanged from the `oatschema/oats` release tag.
- `SCHEMA_VERSION`, `SchemaName`, `available_versions`, `schema_json` and `Error`.
- `schema`, with the `serde_json` feature.
- Minimum supported Rust version: 1.85.
