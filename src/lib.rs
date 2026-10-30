// SPDX-FileCopyrightText: 2026 The OATS Authors
// SPDX-License-Identifier: Apache-2.0

//! The OATS (Open Annotation Task Schemas) JSON schemas for Rust.
//!
//! OATS is a collection of strict, versioned JSON Schema 2020-12 schemas, defining and validating
//! the structured outputs of machine learning models, workflow steps and annotation. Models,
//! pipelines and people write the same JSON for each task, whatever produced it.
//!
//! Every OATS release from 0.8.0 on is compiled into the crate, unchanged, so nothing touches
//! the network or the filesystem. Schemas, documentation and the changelog live at
//! [oatschema.org](https://oatschema.org).
//!
//! ```
//! use oatschema::{SchemaName, schema_json};
//!
//! let text = schema_json(SchemaName::Classification, None)?;
//! assert!(text.contains(r#""$id": "https://oatschema.org/0.8.0/classification""#));
//!
//! let name: SchemaName = "llm-output".parse()?;
//! let pinned = schema_json(name, Some("0.8.0"))?;
//! assert!(pinned.contains(r#""x-version": "0.8.0""#));
//! # Ok::<(), oatschema::Error>(())
//! ```
//!
//! # Features
//!
//! - `serde_json`: adds [`schema`], which returns a schema as a [`serde_json::Value`].
//!
//! # Schema versions
//!
//! Every lookup takes a version, which defaults to [`SCHEMA_VERSION`]. Versions must be exact,
//! such as `"0.8.0"`; `"0.8"` and `"latest"` are rejected. [`available_versions`] lists every
//! bundled version. Before 1.0.0, a schema version may reject records that an older one accepted,
//! so validate a record against the version it was written for.
//!
//! Each schema states its own version in its `x-version` keyword, and its `$id` includes the
//! version (`https://oatschema.org/0.8.0/classification`), so versions can sit side by side in one
//! JSON Schema registry. Releases before 0.8.0 are not bundled here; the Python package
//! [`oatschema`](https://pypi.org/project/oatschema/) bundles them.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![doc(test(attr(deny(warnings))))]

mod generated;

#[cfg(all(doctest, feature = "serde_json"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

use std::fmt;
use std::str::FromStr;

/// The newest bundled schema version, used when no version is given.
pub const SCHEMA_VERSION: &str = "0.8.0";

/// The name of an OATS schema.
///
/// The variants are the schemas in [`SCHEMA_VERSION`]. Other bundled versions contain a subset of
/// them.
///
/// ```
/// use oatschema::SchemaName;
///
/// let name: SchemaName = "object-detection".parse()?;
/// assert_eq!(name, SchemaName::ObjectDetection);
/// assert_eq!(name.as_str(), "object-detection");
/// assert_eq!(name.to_string(), "object-detection");
/// assert!("Object-Detection".parse::<SchemaName>().is_err());
/// # Ok::<(), oatschema::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum SchemaName {
    /// `audio-transcription`
    AudioTranscription,
    /// `classification`
    Classification,
    /// `document-extraction`
    DocumentExtraction,
    /// `embedding`
    Embedding,
    /// `entity-extraction`
    EntityExtraction,
    /// `generic`
    Generic,
    /// `geolocation`
    Geolocation,
    /// `llm-output`
    LlmOutput,
    /// `object-detection`
    ObjectDetection,
    /// `ranking`, new in 0.8.0
    Ranking,
    /// `regression`
    Regression,
    /// `temporal-segmentation`
    TemporalSegmentation,
}

impl SchemaName {
    /// Every schema name, in alphabetical order.
    ///
    /// ```
    /// use oatschema::SchemaName;
    ///
    /// assert_eq!(SchemaName::ALL.first(), Some(&SchemaName::AudioTranscription));
    /// assert!(SchemaName::ALL.iter().all(|name| name.as_str().parse() == Ok(*name)));
    /// ```
    pub const ALL: &'static [SchemaName] = &[
        SchemaName::AudioTranscription,
        SchemaName::Classification,
        SchemaName::DocumentExtraction,
        SchemaName::Embedding,
        SchemaName::EntityExtraction,
        SchemaName::Generic,
        SchemaName::Geolocation,
        SchemaName::LlmOutput,
        SchemaName::ObjectDetection,
        SchemaName::Ranking,
        SchemaName::Regression,
        SchemaName::TemporalSegmentation,
    ];

    /// The schema's name as OATS spells it, such as `"llm-output"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            SchemaName::AudioTranscription => "audio-transcription",
            SchemaName::Classification => "classification",
            SchemaName::DocumentExtraction => "document-extraction",
            SchemaName::Embedding => "embedding",
            SchemaName::EntityExtraction => "entity-extraction",
            SchemaName::Generic => "generic",
            SchemaName::Geolocation => "geolocation",
            SchemaName::LlmOutput => "llm-output",
            SchemaName::ObjectDetection => "object-detection",
            SchemaName::Ranking => "ranking",
            SchemaName::Regression => "regression",
            SchemaName::TemporalSegmentation => "temporal-segmentation",
        }
    }
}

impl fmt::Display for SchemaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl FromStr for SchemaName {
    type Err = Error;

    /// Parses a schema name exactly as OATS spells it, such as `"llm-output"`.
    ///
    /// Returns [`Error::UnknownName`] for any other text.
    fn from_str(name: &str) -> Result<Self, Error> {
        SchemaName::ALL
            .iter()
            .copied()
            .find(|known| known.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                name: name.to_owned(),
            })
    }
}

/// An unknown schema name or version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The name is not a schema in any bundled version.
    UnknownName {
        /// The name that was asked for.
        name: String,
    },
    /// The version is not bundled. Only exact versions such as `"0.8.0"` are accepted.
    UnknownVersion {
        /// The version that was asked for.
        version: String,
    },
    /// The version is bundled but does not contain the schema.
    NotInVersion {
        /// The schema that was asked for.
        name: SchemaName,
        /// The bundled version that lacks it.
        version: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownName { name } => {
                write!(f, "unknown schema name {name:?}; expected one of ")?;
                for (index, known) in SchemaName::ALL.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str(known.as_str())?;
                }
                Ok(())
            }
            Error::UnknownVersion { version } => {
                write!(
                    f,
                    "schema version {version:?} is not bundled; available: {}",
                    available_versions().join(", ")
                )
            }
            Error::NotInVersion { name, version } => {
                write!(
                    f,
                    "schema {:?} is not in schema version {version}",
                    name.as_str()
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// Every bundled schema version, oldest first. The first is 0.8.0.
///
/// ```
/// let versions = oatschema::available_versions();
/// assert_eq!(versions.first(), Some(&"0.8.0"));
/// assert_eq!(versions.last(), Some(&oatschema::SCHEMA_VERSION));
/// ```
pub fn available_versions() -> &'static [&'static str] {
    generated::VERSIONS
}

/// Returns the schema `name` from schema version `version`, or from [`SCHEMA_VERSION`] if it is
/// `None`, as the schema file's text exactly as released.
///
/// `version` must be an exact bundled version such as `"0.8.0"`; partial versions and aliases are
/// not accepted.
///
/// # Errors
///
/// - [`Error::UnknownVersion`] if `version` is not bundled.
/// - [`Error::NotInVersion`] if `version` does not contain `name`.
///
/// ```
/// use oatschema::{Error, SchemaName, schema_json};
///
/// let text = schema_json(SchemaName::TemporalSegmentation, Some("0.8.0"))?;
/// assert!(text.contains(r#""title": "Temporal Segmentation""#));
///
/// assert!(matches!(
///     schema_json(SchemaName::Classification, Some("0.8")),
///     Err(Error::UnknownVersion { .. })
/// ));
/// # Ok::<(), Error>(())
/// ```
pub fn schema_json(name: SchemaName, version: Option<&str>) -> Result<&'static str, Error> {
    let version = version.unwrap_or(SCHEMA_VERSION);
    let Some(index) = generated::VERSIONS
        .iter()
        .position(|known| *known == version)
    else {
        return Err(Error::UnknownVersion {
            version: version.to_owned(),
        });
    };
    generated::SCHEMAS[index]
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| *text)
        .ok_or(Error::NotInVersion {
            name,
            version: generated::VERSIONS[index],
        })
}

/// Returns the schema `name` from schema version `version`, or from [`SCHEMA_VERSION`] if it is
/// `None`, parsed into a new [`serde_json::Value`] on every call.
///
/// Available with the `serde_json` feature.
///
/// # Errors
///
/// The same as [`schema_json`].
///
/// ```
/// use oatschema::{SchemaName, schema};
///
/// let ranking = schema(SchemaName::Ranking, None)?;
/// assert_eq!(ranking["x-version"], oatschema::SCHEMA_VERSION);
/// assert_eq!(ranking["$id"], "https://oatschema.org/0.8.0/ranking");
/// # Ok::<(), oatschema::Error>(())
/// ```
#[cfg(feature = "serde_json")]
pub fn schema(name: SchemaName, version: Option<&str>) -> Result<serde_json::Value, Error> {
    let text = schema_json(name, version)?;
    Ok(serde_json::from_str(text).expect("bundled schemas are valid JSON"))
}
