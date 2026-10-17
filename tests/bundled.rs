// SPDX-FileCopyrightText: 2026 The OATS Authors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use oatschema::{SCHEMA_VERSION, SchemaName, available_versions, schema_json};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MANIFEST_NAME: &str = "MANIFEST.json";

fn schemas_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas")
}

fn version_key(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| part.parse().unwrap())
        .collect()
}

fn bundled_names(version: &str) -> BTreeSet<String> {
    fs::read_dir(schemas_dir().join(version))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|file| file != MANIFEST_NAME)
        .map(|file| {
            file.strip_suffix(".json")
                .expect("only JSON files are bundled")
                .to_owned()
        })
        .collect()
}

fn read_manifest(version: &str) -> Value {
    serde_json::from_slice(&fs::read(schemas_dir().join(version).join(MANIFEST_NAME)).unwrap())
        .unwrap()
}

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn available_versions_are_bundled_directories_in_order() {
    let directories: BTreeSet<String> = fs::read_dir(schemas_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    let versions = available_versions();
    assert_eq!(
        directories,
        versions.iter().map(|version| version.to_string()).collect()
    );
    let mut sorted = versions.to_vec();
    sorted.sort_by_key(|version| version_key(version));
    assert_eq!(sorted, versions);
    assert_eq!(versions[0], "0.8.0");
}

#[test]
fn schema_name_matches_schema_version_and_covers_older_versions() {
    let names: BTreeSet<String> = SchemaName::ALL
        .iter()
        .map(|name| name.as_str().to_owned())
        .collect();
    assert_eq!(bundled_names(SCHEMA_VERSION), names);
    for version in available_versions() {
        assert!(bundled_names(version).is_subset(&names), "{version}");
    }
}

#[test]
fn schema_json_returns_each_bundled_file_exactly() {
    for version in available_versions() {
        let names = bundled_names(version);
        for name in SchemaName::ALL {
            let result = schema_json(*name, Some(version));
            if names.contains(name.as_str()) {
                let path = schemas_dir().join(version).join(format!("{name}.json"));
                assert_eq!(
                    result.unwrap().as_bytes(),
                    fs::read(path).unwrap(),
                    "{version} {name}"
                );
            } else {
                assert!(result.is_err(), "{version} {name}");
            }
        }
    }
}

#[test]
fn schema_identity() {
    for version in available_versions() {
        for name in bundled_names(version) {
            let schema: Value =
                serde_json::from_str(schema_json(name.parse().unwrap(), Some(version)).unwrap())
                    .unwrap();
            assert_eq!(schema["x-version"], *version, "{version} {name}");
            assert!(schema.get("version").is_none(), "{version} {name}");
            assert_eq!(schema["x-license"]["id"], "Apache-2.0");
            let notice = schema["x-license"]["notice"].as_str().unwrap();
            assert!(
                notice.starts_with("Copyright 20") && notice.ends_with(" The OATS Authors"),
                "{version} {name}: {notice}"
            );
            assert_eq!(
                schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
            assert_eq!(
                schema["$id"],
                format!("https://oatschema.org/{version}/{name}"),
                "{version} {name}"
            );
        }
    }
}

#[test]
fn manifest_matches_bundled_files() {
    for version in available_versions() {
        let manifest = read_manifest(version);
        assert_eq!(manifest["tag"], format!("v{version}"));
        let commit = manifest["commit"].as_str().unwrap();
        assert!(
            commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{commit}"
        );
        let actual: BTreeMap<String, Value> = fs::read_dir(schemas_dir().join(version))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.file_name().unwrap() != MANIFEST_NAME)
            .map(|path| {
                (
                    path.file_name().unwrap().to_str().unwrap().to_owned(),
                    sha256(&fs::read(&path).unwrap()).into(),
                )
            })
            .collect();
        let expected: BTreeMap<String, Value> = manifest["files"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        assert_eq!(actual, expected, "{version}");
    }
}

fn oats_repo() -> Option<PathBuf> {
    let (repo, explicit) = match std::env::var_os("OATS_REPO") {
        Some(repo) => (PathBuf::from(repo), true),
        None => (
            PathBuf::from(std::env::var_os("HOME")?)
                .join("projects")
                .join("oats"),
            false,
        ),
    };
    if repo.join(".git").exists() {
        return Some(repo);
    }
    assert!(
        !explicit,
        "OATS_REPO is set to {}, which is not a git checkout",
        repo.display()
    );
    eprintln!(
        "skipping: no oats checkout at {}; set OATS_REPO to compare with the release tags",
        repo.display()
    );
    None
}

fn git(repo: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn bundled_files_match_oats_release_tags() {
    let Some(repo) = oats_repo() else {
        return;
    };
    for version in available_versions() {
        let tag = format!("refs/tags/v{version}");
        let commit = String::from_utf8(git(
            &repo,
            &["rev-parse", "--verify", &format!("{tag}^{{commit}}")],
        ))
        .unwrap();
        assert_eq!(read_manifest(version)["commit"], commit.trim(), "{version}");
        let listing = String::from_utf8(git(
            &repo,
            &["ls-tree", "--name-only", &format!("{tag}:schemas")],
        ))
        .unwrap();
        let released: BTreeSet<String> = listing
            .lines()
            .filter_map(|entry| entry.strip_suffix(".json"))
            .map(str::to_owned)
            .collect();
        assert_eq!(released, bundled_names(version), "{version}");
        for name in released {
            let data = git(&repo, &["show", &format!("{tag}:schemas/{name}.json")]);
            assert_eq!(
                data,
                fs::read(schemas_dir().join(version).join(format!("{name}.json"))).unwrap(),
                "{version} {name}"
            );
        }
    }
}
