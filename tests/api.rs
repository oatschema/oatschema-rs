// SPDX-FileCopyrightText: 2026 The OATS Authors
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use oatschema::{Error, SCHEMA_VERSION, SchemaName, available_versions, schema_json};

#[test]
fn schema_version_is_the_newest_bundled_version() {
    assert_eq!(available_versions().last(), Some(&SCHEMA_VERSION));
}

#[test]
fn names_round_trip() {
    let mut seen = HashSet::new();
    for name in SchemaName::ALL {
        assert!(seen.insert(name.as_str()));
        assert_eq!(name.as_str().parse::<SchemaName>(), Ok(*name));
        assert_eq!(name.to_string(), name.as_str());
    }
    assert_eq!(seen.len(), 12);
    let mut sorted = SchemaName::ALL.to_vec();
    sorted.sort_by_key(|name| name.as_str());
    assert_eq!(sorted, SchemaName::ALL);
    let mut ordered = SchemaName::ALL.to_vec();
    ordered.sort();
    assert_eq!(ordered, SchemaName::ALL);
}

#[test]
fn display_respects_padding() {
    assert_eq!(format!("{:>10}|", SchemaName::Generic), "   generic|");
}

#[test]
fn names_are_servable() {
    for name in SchemaName::ALL {
        let name = name.as_str();
        assert!(!name.starts_with(|first: char| first.is_ascii_digit()));
        assert!(!["changelog", "docs", "latest"].contains(&name));
    }
}

#[test]
fn parse_rejects_unknown_names() {
    for name in [
        "classifications",
        "open/classification",
        "",
        "MANIFEST",
        "Classification",
        " generic",
    ] {
        let error = name.parse::<SchemaName>().unwrap_err();
        assert_eq!(
            error,
            Error::UnknownName {
                name: name.to_owned()
            }
        );
        assert!(error.to_string().starts_with(&format!(
            "unknown schema name {name:?}; expected one of audio-transcription, "
        )));
    }
}

#[test]
fn schema_json_defaults_to_schema_version() {
    assert_eq!(
        schema_json(SchemaName::Classification, None),
        schema_json(SchemaName::Classification, Some(SCHEMA_VERSION))
    );
}

#[test]
fn schema_json_rejects_inexact_or_unbundled_versions() {
    for version in [
        "0.8", "latest", "v0.8.0", "0.8.0.0", "0.7.0", "0.4.0", "9.9.9", "", " 0.8.0",
    ] {
        let error = schema_json(SchemaName::Classification, Some(version)).unwrap_err();
        assert_eq!(
            error,
            Error::UnknownVersion {
                version: version.to_owned()
            }
        );
        assert_eq!(
            error.to_string(),
            format!("schema version {version:?} is not bundled; available: 0.8.0")
        );
    }
}

#[test]
fn not_in_version_names_the_schema_and_version() {
    let error = Error::NotInVersion {
        name: SchemaName::Ranking,
        version: "0.8.0",
    };
    assert_eq!(
        error.to_string(),
        "schema \"ranking\" is not in schema version 0.8.0"
    );
}

#[test]
fn every_name_is_in_schema_version() {
    for name in SchemaName::ALL {
        assert!(schema_json(*name, None).is_ok(), "{name}");
    }
}

#[test]
fn error_is_a_std_error() {
    let error: Box<dyn std::error::Error + Send + Sync> = Box::new(Error::UnknownVersion {
        version: "0.8".into(),
    });
    assert!(error.source().is_none());
}

#[cfg(feature = "serde_json")]
#[test]
fn schema_parses_the_released_text() {
    for version in available_versions() {
        for name in SchemaName::ALL {
            match (
                oatschema::schema(*name, Some(version)),
                schema_json(*name, Some(version)),
            ) {
                (Ok(value), Ok(text)) => {
                    assert_eq!(
                        value,
                        serde_json::from_str::<serde_json::Value>(text).unwrap()
                    );
                    assert_eq!(value["x-version"], *version);
                }
                (Err(left), Err(right)) => assert_eq!(left, right),
                (left, right) => panic!("{left:?} != {right:?}"),
            }
        }
    }
}

#[cfg(feature = "serde_json")]
#[test]
fn schema_returns_independent_values() {
    let mut first = oatschema::schema(SchemaName::Classification, None).unwrap();
    first["properties"].as_object_mut().unwrap().clear();
    first["title"] = "changed".into();
    let second = oatschema::schema(SchemaName::Classification, None).unwrap();
    assert!(!second["properties"].as_object().unwrap().is_empty());
    assert_ne!(second["title"], "changed");
}

#[cfg(feature = "serde_json")]
#[test]
fn schema_reports_version_errors() {
    assert!(matches!(
        oatschema::schema(SchemaName::Classification, Some("0.7.0")),
        Err(Error::UnknownVersion { .. })
    ));
}
