// SPDX-FileCopyrightText: 2026 The OATS Authors
// SPDX-License-Identifier: Apache-2.0

use crate::SchemaName;

pub(crate) const VERSIONS: &[&str] = &["0.8.0"];

pub(crate) const SCHEMAS: &[&[(SchemaName, &str)]] = &[&[
    (
        SchemaName::AudioTranscription,
        include_str!("../schemas/0.8.0/audio-transcription.json"),
    ),
    (
        SchemaName::Classification,
        include_str!("../schemas/0.8.0/classification.json"),
    ),
    (
        SchemaName::DocumentExtraction,
        include_str!("../schemas/0.8.0/document-extraction.json"),
    ),
    (
        SchemaName::Embedding,
        include_str!("../schemas/0.8.0/embedding.json"),
    ),
    (
        SchemaName::EntityExtraction,
        include_str!("../schemas/0.8.0/entity-extraction.json"),
    ),
    (
        SchemaName::Generic,
        include_str!("../schemas/0.8.0/generic.json"),
    ),
    (
        SchemaName::Geolocation,
        include_str!("../schemas/0.8.0/geolocation.json"),
    ),
    (
        SchemaName::LlmOutput,
        include_str!("../schemas/0.8.0/llm-output.json"),
    ),
    (
        SchemaName::ObjectDetection,
        include_str!("../schemas/0.8.0/object-detection.json"),
    ),
    (
        SchemaName::Ranking,
        include_str!("../schemas/0.8.0/ranking.json"),
    ),
    (
        SchemaName::Regression,
        include_str!("../schemas/0.8.0/regression.json"),
    ),
    (
        SchemaName::TemporalSegmentation,
        include_str!("../schemas/0.8.0/temporal-segmentation.json"),
    ),
]];
