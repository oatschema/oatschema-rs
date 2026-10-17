// SPDX-FileCopyrightText: 2026 The OATS Authors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::{Deserialize, Deserializer, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

const USAGE: &str = "\
usage:
  cargo xtask sync <oats-repo> <tag> [--dest <schemas-dir>]
  cargo xtask codegen [--check]";
const METASCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";
const FIRST_BUNDLED_VERSION: Version = (0, 8, 0);
const LICENSE_ID: &str = "Apache-2.0";
const COPYRIGHT_HOLDER: &str = "The OATS Authors";
const SPDX_HEADER: &str =
    "// SPDX-FileCopyrightText: 2026 The OATS Authors\n// SPDX-License-Identifier: Apache-2.0\n";
const RESERVED_NAMES: [&str; 3] = ["changelog", "docs", "latest"];
const MANIFEST_NAME: &str = "MANIFEST.json";
const GENERATED_PATH: &str = "src/generated.rs";

type Version = (u64, u64, u64);

#[derive(Debug, PartialEq, Eq)]
struct SyncError(String);

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn fail<T>(message: impl Into<String>) -> Result<T, SyncError> {
    Err(SyncError(message.into()))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace")
        .to_path_buf()
}

fn version_text(version: Version) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

fn parse_version(text: &str) -> Option<Version> {
    let parts: Vec<&str> = text.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return None;
    };
    let number = |part: &str| {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    Some((number(major)?, number(minor)?, number(patch)?))
}

fn parse_tag(tag: &str) -> Result<Version, SyncError> {
    match tag.strip_prefix('v').and_then(parse_version) {
        Some(version) if version >= FIRST_BUNDLED_VERSION => Ok(version),
        Some(_) => fail(format!(
            "{tag} predates OATS {}, the first release this crate bundles",
            version_text(FIRST_BUNDLED_VERSION)
        )),
        None => fail(format!("{tag:?} is not a release tag of the form vX.Y.Z")),
    }
}

fn expected_id(name: &str, version: Version) -> String {
    format!("https://oatschema.org/{}/{name}", version_text(version))
}

fn check_name(name: &str) -> Result<(), SyncError> {
    let kebab = name.starts_with(|first: char| first.is_ascii_lowercase())
        && name.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        });
    if !kebab {
        return fail(format!(
            "{name:?} is not a lowercase kebab-case name starting with a letter"
        ));
    }
    if RESERVED_NAMES.contains(&name) {
        return fail(format!("{name:?} is a reserved name"));
    }
    Ok(())
}

struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Strict;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Strict, E> {
        Ok(Strict(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Strict, E> {
        Ok(Strict(Value::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Strict, E> {
        Ok(Strict(Value::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Strict, E> {
        Ok(Strict(
            Number::from_f64(value).map_or(Value::Null, Value::Number),
        ))
    }

    fn visit_str<E>(self, value: &str) -> Result<Strict, E> {
        Ok(Strict(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Strict, E> {
        Ok(Strict(Value::String(value)))
    }

    fn visit_unit<E>(self) -> Result<Strict, E> {
        Ok(Strict(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
        let mut items = Vec::new();
        while let Some(Strict(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Strict(Value::Array(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Strict, A::Error> {
        let mut seen = HashSet::new();
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(A::Error::custom(format!("duplicate key {key:?}")));
            }
            let Strict(value) = map.next_value()?;
            object.insert(key, value);
        }
        Ok(Strict(Value::Object(object)))
    }
}

fn check_schema(name: &str, data: &[u8], version: Version) -> Result<(), SyncError> {
    check_name(name)?;
    let text = match std::str::from_utf8(data) {
        Ok(text) => text,
        Err(error) => return fail(format!("{name}: not UTF-8: {error}")),
    };
    let schema = match serde_json::from_str::<Strict>(text) {
        Ok(Strict(Value::Object(schema))) => schema,
        Ok(_) => return fail(format!("{name}: not a JSON object")),
        Err(error) if error.is_data() => return fail(format!("{name}: {error}")),
        Err(error) => return fail(format!("{name}: invalid JSON: {error}")),
    };
    let expected = [
        ("$schema", METASCHEMA.to_owned()),
        ("$id", expected_id(name, version)),
        ("x-version", version_text(version)),
    ];
    for (key, value) in expected {
        match schema.get(key) {
            Some(Value::String(actual)) if *actual == value => {}
            Some(actual) => return fail(format!("{name}: {key} is {actual}, expected {value:?}")),
            None => return fail(format!("{name}: {key} is missing, expected {value:?}")),
        }
    }
    if schema.contains_key("version") {
        return fail(format!(
            "{name}: has a top-level version; from 0.8.0 it is x-version"
        ));
    }
    check_license(name, schema.get("x-license"))
}

fn is_copyright_years(years: &str) -> bool {
    let year = |text: &str| text.len() == 4 && text.bytes().all(|byte| byte.is_ascii_digit());
    match years.split_once('-') {
        Some((first, last)) => year(first) && year(last) && first < last,
        None => year(years),
    }
}

fn check_license(name: &str, license: Option<&Value>) -> Result<(), SyncError> {
    let Some(Value::Object(license)) = license else {
        return fail(format!("{name}: x-license is missing or not an object"));
    };
    if license.get("id").and_then(Value::as_str) != Some(LICENSE_ID) {
        return fail(format!("{name}: x-license id is not {LICENSE_ID:?}"));
    }
    let notice = license
        .get("notice")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let years = notice
        .strip_prefix("Copyright ")
        .and_then(|rest| rest.strip_suffix(&format!(" {COPYRIGHT_HOLDER}")));
    if !years.is_some_and(is_copyright_years) {
        return fail(format!(
            "{name}: x-license notice {notice:?} is not \"Copyright <years> {COPYRIGHT_HOLDER}\""
        ));
    }
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, SyncError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| SyncError(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return fail(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn resolve_tag(repo: &Path, tag: &str) -> Result<String, SyncError> {
    let tag_ref = format!("refs/tags/{tag}");
    let peeled_ref = format!("{tag_ref}^{{}}");
    let local = git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{tag_ref}^{{commit}}"),
        ],
    )?;
    let local = String::from_utf8_lossy(&local).trim().to_owned();
    let listing = git(
        repo,
        &["ls-remote", "--tags", "origin", &tag_ref, &peeled_ref],
    )?;
    let listing = String::from_utf8_lossy(&listing);
    let remote: BTreeMap<&str, &str> = listing
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(sha, reference)| (reference, sha))
        .collect();
    let Some(remote_commit) = remote
        .get(peeled_ref.as_str())
        .or_else(|| remote.get(tag_ref.as_str()))
    else {
        return fail(format!("{tag} has not been pushed to origin"));
    };
    if *remote_commit != local {
        return fail(format!(
            "{tag} is {local} locally but {remote_commit} on origin"
        ));
    }
    Ok(local)
}

fn read_schemas(
    repo: &Path,
    tag: &str,
    commit: &str,
) -> Result<BTreeMap<String, Vec<u8>>, SyncError> {
    let listing = git(
        repo,
        &["ls-tree", "--name-only", &format!("{commit}:schemas")],
    )?;
    let mut schemas = BTreeMap::new();
    for entry in String::from_utf8_lossy(&listing).lines() {
        if let Some(name) = entry.strip_suffix(".json") {
            let data = git(repo, &["show", &format!("{commit}:schemas/{entry}")])?;
            schemas.insert(name.to_owned(), data);
        }
    }
    if schemas.is_empty() {
        return fail(format!("{tag} has no schemas"));
    }
    Ok(schemas)
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("strings always serialize")
}

fn build_manifest(tag: &str, commit: &str, schemas: &BTreeMap<String, Vec<u8>>) -> String {
    let files: Vec<String> = schemas
        .iter()
        .map(|(name, data)| {
            let digest: String = Sha256::digest(data)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            format!(
                "    {}: {}",
                json_string(&format!("{name}.json")),
                json_string(&digest)
            )
        })
        .collect();
    format!(
        "{{\n  \"tag\": {},\n  \"commit\": {},\n  \"files\": {{\n{}\n  }}\n}}\n",
        json_string(tag),
        json_string(commit),
        files.join(",\n")
    )
}

fn sync(repo: &Path, tag: &str, dest: &Path) -> Result<PathBuf, SyncError> {
    let version = parse_tag(tag)?;
    let target = dest.join(version_text(version));
    let exists = || target.symlink_metadata().is_ok();
    if exists() {
        return fail(format!(
            "{} already exists; bundled releases are never rewritten",
            target.display()
        ));
    }
    let commit = resolve_tag(repo, tag)?;
    let schemas = read_schemas(repo, tag, &commit)?;
    for (name, data) in &schemas {
        check_schema(name, data, version)?;
    }
    let io = |error: std::io::Error| SyncError(format!("cannot write {}: {error}", dest.display()));
    fs::create_dir_all(dest).map_err(io)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let staging = dest.join(format!(
        ".{}-{}-{nonce}",
        version_text(version),
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(io)?;
    let written = (|| {
        for (name, data) in &schemas {
            fs::write(staging.join(format!("{name}.json")), data).map_err(io)?;
        }
        fs::write(
            staging.join(MANIFEST_NAME),
            build_manifest(tag, &commit, &schemas),
        )
        .map_err(io)?;
        if exists() {
            return fail(format!(
                "{} appeared while syncing; bundled releases are never rewritten",
                target.display()
            ));
        }
        fs::rename(&staging, &target).map_err(io)
    })();
    if let Err(error) = written {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(target)
}

fn variant(name: &str) -> String {
    name.split('-')
        .map(|segment| {
            let mut chars = segment.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

fn bundled(schemas_dir: &Path) -> Result<Vec<(Version, Vec<String>)>, SyncError> {
    let io = |error: std::io::Error| {
        SyncError(format!("cannot read {}: {error}", schemas_dir.display()))
    };
    let mut versions = Vec::new();
    for entry in fs::read_dir(schemas_dir).map_err(io)? {
        let entry = entry.map_err(io)?;
        let Some(version) = entry.file_name().to_str().and_then(parse_version) else {
            continue;
        };
        if !entry.file_type().map_err(io)?.is_dir() {
            continue;
        }
        let mut names = Vec::new();
        for file in fs::read_dir(entry.path()).map_err(io)? {
            let file_name = file.map_err(io)?.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if file_name == MANIFEST_NAME {
                continue;
            }
            if let Some(name) = file_name.strip_suffix(".json") {
                names.push(name.to_owned());
            }
        }
        names.sort();
        versions.push((version, names));
    }
    versions.sort();
    if versions.is_empty() {
        return fail(format!("{} has no bundled versions", schemas_dir.display()));
    }
    Ok(versions)
}

fn rustfmt(source: &str) -> Result<String, SyncError> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| SyncError(format!("cannot run rustfmt: {error}")))?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(source.as_bytes())
        .map_err(|error| SyncError(format!("cannot write to rustfmt: {error}")))?;
    let output = child
        .wait_with_output()
        .map_err(|error| SyncError(format!("rustfmt failed: {error}")))?;
    if !output.status.success() {
        return fail(format!(
            "rustfmt failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| SyncError(format!("rustfmt output: {error}")))
}

fn render(schemas_dir: &Path) -> Result<String, SyncError> {
    let versions = bundled(schemas_dir)?;
    let mut source = format!("{SPDX_HEADER}\nuse crate::SchemaName;\n\n");
    let list: Vec<String> = versions
        .iter()
        .map(|(version, _)| json_string(&version_text(*version)))
        .collect();
    source += &format!(
        "pub(crate) const VERSIONS: &[&str] = &[{}];\n\n",
        list.join(", ")
    );
    source += "pub(crate) const SCHEMAS: &[&[(SchemaName, &str)]] = &[\n";
    for (version, names) in &versions {
        source += "&[\n";
        for name in names {
            check_name(name).map_err(|SyncError(error)| {
                SyncError(format!("{}: {error}", version_text(*version)))
            })?;
            source += &format!(
                "(SchemaName::{}, include_str!(\"../schemas/{}/{name}.json\")),\n",
                variant(name),
                version_text(*version)
            );
        }
        source += "],\n";
    }
    source += "];\n";
    Ok(source)
}

fn generate(schemas_dir: &Path) -> Result<String, SyncError> {
    rustfmt(&render(schemas_dir)?)
}

fn codegen(check: bool) -> Result<(), SyncError> {
    let root = workspace_root();
    let path = root.join(GENERATED_PATH);
    let generated = generate(&root.join("schemas"))?;
    let current = fs::read_to_string(&path).unwrap_or_default();
    if check {
        if current != generated {
            return fail(format!(
                "{GENERATED_PATH} is out of date; run `cargo xtask codegen`"
            ));
        }
        println!("{GENERATED_PATH} is up to date");
    } else if current != generated {
        fs::write(&path, generated)
            .map_err(|error| SyncError(format!("cannot write {}: {error}", path.display())))?;
        println!("wrote {GENERATED_PATH}");
    } else {
        println!("{GENERATED_PATH} is up to date");
    }
    Ok(())
}

fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn run(args: &[String]) -> Result<(), SyncError> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["sync", repo, tag, rest @ ..] => {
            let default_dest = workspace_root().join("schemas");
            let dest = match rest {
                [] => default_dest.clone(),
                ["--dest", dest] => expand_home(dest),
                _ => return fail(USAGE),
            };
            let target = sync(&expand_home(repo), tag, &dest)?;
            println!("bundled {tag} into {}", target.display());
            if dest == default_dest {
                codegen(false)?;
                println!("check SCHEMA_VERSION and SchemaName in src/lib.rs, then run the tests");
            }
            Ok(())
        }
        ["codegen"] => codegen(false),
        ["codegen", "--check"] => codegen(true),
        _ => fail(USAGE),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "oatschema-xtask-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn schema_bytes(name: &str, version: &str, overrides: &[(&str, Value)]) -> Vec<u8> {
        let numeric = parse_tag(&format!("v{version}")).unwrap();
        let mut schema = Map::new();
        schema.insert("$schema".into(), METASCHEMA.into());
        schema.insert("$id".into(), expected_id(name, numeric).into());
        schema.insert("title".into(), name.into());
        schema.insert("x-version".into(), version.into());
        schema.insert(
            "x-license".into(),
            serde_json::json!({
                "id": "Apache-2.0",
                "notice": "Copyright 2025-2026 The OATS Authors",
                "url": "https://github.com/oatschema/oats/blob/main/LICENSE",
            }),
        );
        schema.insert("type".into(), "object".into());
        for (key, value) in overrides {
            schema.insert((*key).into(), value.clone());
        }
        serde_json::to_vec_pretty(&schema).unwrap()
    }

    fn schema_without(name: &str, version: &str, key: &str) -> Vec<u8> {
        let mut schema: Map<String, Value> =
            serde_json::from_slice(&schema_bytes(name, version, &[])).unwrap();
        schema.remove(key);
        serde_json::to_vec_pretty(&schema).unwrap()
    }

    fn with_notice(notice: &str) -> Vec<u8> {
        schema_bytes(
            "generic",
            "0.8.0",
            &[(
                "x-license",
                serde_json::json!({"id": "Apache-2.0", "notice": notice}),
            )],
        )
    }

    fn git_ok(repo: &Path, args: &[&str]) -> String {
        String::from_utf8(git(repo, args).unwrap()).unwrap()
    }

    fn release_repo(tmp: &Path) -> PathBuf {
        let origin = tmp.join("origin.git");
        let repo = tmp.join("oats");
        git_ok(tmp, &["init", "--bare", "-q", origin.to_str().unwrap()]);
        git_ok(tmp, &["init", "-q", repo.to_str().unwrap()]);
        for (key, value) in [
            ("user.email", "test@example.com"),
            ("user.name", "Test"),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
        ] {
            git_ok(&repo, &["config", key, value]);
        }
        git_ok(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        fs::create_dir(repo.join("schemas")).unwrap();
        for name in ["classification", "temporal-segmentation"] {
            fs::write(
                repo.join("schemas").join(format!("{name}.json")),
                schema_bytes(name, "0.8.0", &[]),
            )
            .unwrap();
        }
        fs::write(repo.join("schemas/README.md"), "not a schema\n").unwrap();
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "release"]);
        git_ok(&repo, &["tag", "-a", "v0.8.0", "-m", "v0.8.0"]);
        git_ok(&repo, &["push", "-q", "origin", "HEAD", "--tags"]);
        repo
    }

    fn assert_fails<T: fmt::Debug>(result: Result<T, SyncError>, message: &str) {
        let error = result.unwrap_err();
        assert!(error.0.contains(message), "{error:?} lacks {message:?}");
    }

    #[test]
    fn parse_tag_accepts_release_tags() {
        assert_eq!(parse_tag("v0.8.0"), Ok((0, 8, 0)));
        assert_eq!(parse_tag("v0.10.0"), Ok((0, 10, 0)));
        assert_eq!(parse_tag("v10.20.30"), Ok((10, 20, 30)));
    }

    #[test]
    fn parse_tag_rejects() {
        for tag in [
            "0.8.0",
            "v0.8",
            "v0.8.0-rc1",
            "latest",
            "v0.8.0.0",
            "v0..0",
            "v+1.0.0",
        ] {
            assert!(parse_tag(tag).is_err(), "{tag}");
        }
    }

    #[test]
    fn parse_tag_rejects_releases_before_0_8_0() {
        for tag in ["v0.7.0", "v0.4.0", "v0.7.99"] {
            assert_fails(parse_tag(tag), "predates OATS 0.8.0");
        }
    }

    #[test]
    fn expected_id_includes_the_version() {
        assert_eq!(
            expected_id("generic", (0, 8, 0)),
            "https://oatschema.org/0.8.0/generic"
        );
        assert_eq!(
            expected_id("generic", (1, 0, 0)),
            "https://oatschema.org/1.0.0/generic"
        );
    }

    #[test]
    fn check_name_accepts_kebab_case() {
        for name in ["generic", "llm-output", "object-detection", "a1-b2"] {
            assert_eq!(check_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn check_name_rejects() {
        for name in [
            "docs",
            "latest",
            "changelog",
            "3d-detection",
            "Generic",
            "llm_output",
            "a--b",
            "-a",
            "a-",
            "",
        ] {
            assert!(check_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn check_schema_accepts_a_release() {
        assert_eq!(
            check_schema("generic", &schema_bytes("generic", "0.8.0", &[]), (0, 8, 0)),
            Ok(())
        );
    }

    #[test]
    fn check_schema_accepts_any_copyright_years() {
        for notice in [
            "Copyright 2025 The OATS Authors",
            "Copyright 2025-2026 The OATS Authors",
            "Copyright 2026-2031 The OATS Authors",
        ] {
            assert_eq!(
                check_schema("generic", &with_notice(notice), (0, 8, 0)),
                Ok(()),
                "{notice}"
            );
        }
    }

    #[test]
    fn check_schema_rejects_other_notices() {
        for notice in [
            "Copyright 2025-2026 Dorsal Hub LTD",
            "Copyright The OATS Authors",
            "Copyright 25 The OATS Authors",
            "Copyright 2026-2025 The OATS Authors",
            "Copyright 2025-2026 The OATS Authors.",
            "",
        ] {
            assert_fails(
                check_schema("generic", &with_notice(notice), (0, 8, 0)),
                "notice",
            );
        }
    }

    #[test]
    fn check_schema_rejects() {
        let cases: Vec<(Vec<u8>, &str)> = vec![
            (
                schema_bytes("generic", "0.8.0", &[("x-version", "0.7.0".into())]),
                "x-version",
            ),
            (
                schema_without("generic", "0.8.0", "x-version"),
                "x-version is missing",
            ),
            (
                schema_bytes("generic", "0.8.0", &[("version", "0.8.0".into())]),
                "top-level version",
            ),
            (
                schema_bytes(
                    "generic",
                    "0.8.0",
                    &[("$id", "https://oatschema.org/0.7.0/generic".into())],
                ),
                "$id",
            ),
            (
                schema_bytes(
                    "generic",
                    "0.8.0",
                    &[("$id", "https://example.com/generic".into())],
                ),
                "$id",
            ),
            (
                schema_without("generic", "0.8.0", "x-license"),
                "x-license is missing",
            ),
            (
                schema_bytes(
                    "generic",
                    "0.8.0",
                    &[(
                        "x-license",
                        serde_json::json!({"id": "MIT", "notice": "Copyright 2026 The OATS Authors"}),
                    )],
                ),
                "x-license id",
            ),
            (
                schema_bytes(
                    "generic",
                    "0.8.0",
                    &[("$schema", "http://json-schema.org/draft-07/schema#".into())],
                ),
                "$schema",
            ),
            (
                schema_bytes("generic", "0.8.0", &[("x-version", Value::Null)]),
                "x-version is null",
            ),
            (
                br#"{"x-version": "0.8.0", "x-version": "0.8.0"}"#.to_vec(),
                "duplicate key",
            ),
            (
                br#"{"a": [{"b": 1, "b": 2}]}"#.to_vec(),
                "duplicate key \"b\"",
            ),
            (b"[]".to_vec(), "not a JSON object"),
            (b"{".to_vec(), "invalid JSON"),
            (b"{\"title\": \"\xff\"}".to_vec(), "not UTF-8"),
            (b"{}".to_vec(), "$schema is missing"),
        ];
        for (data, message) in cases {
            assert_fails(check_schema("generic", &data, (0, 8, 0)), message);
        }
    }

    #[test]
    fn manifest_matches_python_layout() {
        let schemas = BTreeMap::from([
            ("b".to_owned(), b"two".to_vec()),
            ("a".to_owned(), b"one".to_vec()),
        ]);
        assert_eq!(
            build_manifest("v0.8.0", "abc", &schemas),
            "{\n  \"tag\": \"v0.8.0\",\n  \"commit\": \"abc\",\n  \"files\": {\n    \
             \"a.json\": \"7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed\",\n    \
             \"b.json\": \"3fc4ccfe745870e2c0d99f71f30ff0656c8dedd41cc1d7d3d376b0dbe685e2f3\"\n  }\n}\n"
        );
    }

    #[test]
    fn sync_copies_tag_byte_for_byte() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        let dest = tmp.0.join("schemas");
        fs::write(
            repo.join("schemas/classification.json"),
            "working tree edit\n",
        )
        .unwrap();
        let target = sync(&repo, "v0.8.0", &dest).unwrap();
        assert_eq!(target, dest.join("0.8.0"));
        let mut files: Vec<String> = fs::read_dir(&target)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        assert_eq!(
            files,
            [
                "MANIFEST.json",
                "classification.json",
                "temporal-segmentation.json"
            ]
        );
        let original = schema_bytes("classification", "0.8.0", &[]);
        assert_eq!(
            fs::read(target.join("classification.json")).unwrap(),
            original
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(target.join(MANIFEST_NAME)).unwrap()).unwrap();
        assert_eq!(manifest["tag"], "v0.8.0");
        assert_eq!(
            manifest["commit"],
            git_ok(&repo, &["rev-parse", "v0.8.0^{commit}"]).trim()
        );
        let digest: String = Sha256::digest(&original)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(manifest["files"]["classification.json"], digest);
        let entries: Vec<String> = fs::read_dir(&dest)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(entries, ["0.8.0"]);
    }

    #[test]
    fn sync_refuses_existing_version() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        let dest = tmp.0.join("schemas");
        fs::create_dir_all(dest.join("0.8.0")).unwrap();
        assert_fails(sync(&repo, "v0.8.0", &dest), "never rewritten");
    }

    #[test]
    fn sync_refuses_unpushed_tag() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        git_ok(&repo, &["tag", "v0.8.1"]);
        assert_fails(
            sync(&repo, "v0.8.1", &tmp.0.join("schemas")),
            "not been pushed",
        );
    }

    #[test]
    fn sync_refuses_moved_tag() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        git_ok(&repo, &["commit", "-q", "--allow-empty", "-m", "later"]);
        git_ok(&repo, &["tag", "-f", "-a", "v0.8.0", "-m", "moved"]);
        assert_fails(sync(&repo, "v0.8.0", &tmp.0.join("schemas")), "on origin");
    }

    #[test]
    fn sync_refuses_missing_tag() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        assert_fails(sync(&repo, "v9.9.9", &tmp.0.join("schemas")), "rev-parse");
    }

    #[test]
    fn sync_writes_nothing_when_a_schema_fails() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        fs::write(
            repo.join("schemas/docs.json"),
            schema_bytes("docs", "0.8.1", &[]),
        )
        .unwrap();
        for name in ["classification", "temporal-segmentation"] {
            fs::write(
                repo.join("schemas").join(format!("{name}.json")),
                schema_bytes(name, "0.8.1", &[]),
            )
            .unwrap();
        }
        git_ok(&repo, &["add", "."]);
        git_ok(&repo, &["commit", "-q", "-m", "bad release"]);
        git_ok(&repo, &["tag", "-a", "v0.8.1", "-m", "v0.8.1"]);
        git_ok(&repo, &["push", "-q", "origin", "--tags"]);
        let dest = tmp.0.join("schemas");
        assert_fails(sync(&repo, "v0.8.1", &dest), "reserved");
        assert!(!dest.exists());
    }

    #[test]
    fn sync_refuses_releases_before_0_8_0() {
        let tmp = TempDir::new();
        let repo = release_repo(&tmp.0);
        let dest = tmp.0.join("schemas");
        assert_fails(sync(&repo, "v0.7.0", &dest), "predates OATS 0.8.0");
        assert!(!dest.exists());
    }

    #[test]
    fn rendered_source_starts_with_the_licence_header() {
        let tmp = TempDir::new();
        let version = tmp.0.join("0.8.0");
        fs::create_dir(&version).unwrap();
        fs::write(version.join("generic.json"), "{}").unwrap();
        fs::write(version.join(MANIFEST_NAME), "{}").unwrap();
        let source = render(&tmp.0).unwrap();
        assert!(source.starts_with(SPDX_HEADER), "{source}");
        assert!(source.contains("include_str!(\"../schemas/0.8.0/generic.json\")"));
        assert!(!source.contains("MANIFEST"));
    }

    fn header_files(root: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = [
            "Cargo.toml",
            "xtask/Cargo.toml",
            ".cargo/config.toml",
            ".gitignore",
        ]
        .iter()
        .map(|path| root.join(path))
        .collect();
        let mut pending: Vec<PathBuf> = ["src", "tests", "xtask/src", ".github"]
            .iter()
            .map(|path| root.join(path))
            .collect();
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "rs" || extension == "yml")
                {
                    files.push(path);
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn source_files_start_with_the_licence_header() {
        let root = workspace_root();
        let files = header_files(&root);
        assert!(files.len() > 8, "{files:?}");
        for path in files {
            let text = fs::read_to_string(&path).unwrap();
            let comment = if path.extension().is_some_and(|extension| extension == "rs") {
                "//"
            } else {
                "#"
            };
            let header = SPDX_HEADER.replace("//", comment);
            assert!(
                text.starts_with(&header),
                "{} lacks the SPDX header",
                path.display()
            );
        }
    }

    #[test]
    fn variant_names_are_pascal_case() {
        assert_eq!(variant("generic"), "Generic");
        assert_eq!(variant("llm-output"), "LlmOutput");
        assert_eq!(variant("temporal-segmentation"), "TemporalSegmentation");
    }
}
