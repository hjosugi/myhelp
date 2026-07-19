use crate::{
    Error, MAX_PAGE_BYTES, Result, Vault, content_sha256, is_symlink_or_reparse, metadata_path,
    no_replace_rename, open_read_nofollow, read_snapshot, reject_oversized_page, validate_topic,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use uuid::Uuid;

const PAGE_SUFFIX: &str = ".page.md";
const METADATA_SUFFIX: &str = ".page.meta.yaml";
const MAX_METADATA_BYTES: usize = 256 * 1024;
const MAX_EXPORT_STEM_BYTES: usize = 180;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TldrDiagnosticLevel {
    Error,
    Warning,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TldrDiagnostic {
    pub level: TldrDiagnosticLevel,
    pub line: usize,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TldrValidation {
    pub valid: bool,
    pub diagnostics: Vec<TldrDiagnostic>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TldrSource {
    pub url: String,
    pub title: Option<String>,
    pub license: Option<String>,
    pub attribution: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TldrImportOptions {
    pub topic: Option<String>,
    pub page_license: Option<String>,
    pub source: Option<TldrSource>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TldrImportReport {
    pub topic: String,
    pub page_path: PathBuf,
    pub metadata_path: Option<PathBuf>,
    pub preserved_source_metadata: bool,
    pub validation: TldrValidation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TldrExportMapping {
    pub topic: String,
    pub page_file: String,
    pub metadata_file: Option<String>,
    pub collision_resolved: bool,
    pub validation: TldrValidation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TldrExportReport {
    pub destination: PathBuf,
    pub mappings: Vec<TldrExportMapping>,
}

pub fn validate_tldr_page(content: &str, expected_topic: Option<&str>) -> TldrValidation {
    let mut diagnostics = Vec::new();
    let lines = content.lines().collect::<Vec<_>>();

    if content.is_empty() {
        error(
            &mut diagnostics,
            1,
            "missing-heading",
            "page must start with a level-one heading",
        );
        return validation(diagnostics);
    }
    if content.starts_with('\u{feff}') {
        error(
            &mut diagnostics,
            1,
            "utf8-bom",
            "UTF-8 BOM is outside the supported tldr subset",
        );
    }
    if !content.ends_with('\n') {
        warning(
            &mut diagnostics,
            lines.len().max(1),
            "missing-final-newline",
            "tldr pages should end with a newline",
        );
    }

    let Some(first_nonempty) = lines.iter().position(|line| !line.trim().is_empty()) else {
        error(
            &mut diagnostics,
            1,
            "missing-heading",
            "page must contain a level-one heading",
        );
        return validation(diagnostics);
    };
    if first_nonempty != 0 {
        error(
            &mut diagnostics,
            1,
            "leading-blank-lines",
            "the heading must be the first line",
        );
    }

    let heading_line = lines[first_nonempty];
    let heading = heading_line
        .strip_prefix("# ")
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match heading {
        Some(heading) => {
            if let Some(expected_topic) = expected_topic {
                let expected_name = expected_topic
                    .rsplit('/')
                    .next()
                    .unwrap_or(expected_topic)
                    .to_lowercase();
                let heading_name = heading
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("-")
                    .to_lowercase();
                if heading_name != expected_name {
                    warning(
                        &mut diagnostics,
                        first_nonempty + 1,
                        "heading-topic-mismatch",
                        "heading does not match the page filename after tldr normalization",
                    );
                }
            }
        }
        None => error(
            &mut diagnostics,
            first_nonempty + 1,
            "invalid-heading",
            "the first content line must be `# <title>`",
        ),
    }

    let mut description_count = 0_usize;
    let mut example_count = 0_usize;
    let mut awaiting_command: Option<usize> = None;
    let mut examples_started = false;

    for (index, line) in lines.iter().enumerate().skip(first_nonempty + 1) {
        let line_number = index + 1;
        if line.trim().is_empty() {
            continue;
        }

        if let Some(description_line) = awaiting_command {
            if is_command_line(line) {
                example_count += 1;
                validate_placeholders(
                    command_body(line).expect("is_command_line checked the shape"),
                    line_number,
                    &mut diagnostics,
                );
                awaiting_command = None;
                continue;
            }
            error(
                &mut diagnostics,
                line_number,
                "missing-example-command",
                &format!(
                    "example description on line {description_line} must be followed by one backtick-wrapped command"
                ),
            );
            awaiting_command = None;
        }

        if let Some(description) = line.strip_prefix("> ") {
            if examples_started {
                error(
                    &mut diagnostics,
                    line_number,
                    "description-after-examples",
                    "page description lines must appear before examples",
                );
            }
            if description.trim().is_empty() {
                error(
                    &mut diagnostics,
                    line_number,
                    "empty-description",
                    "description lines cannot be empty",
                );
            }
            description_count += 1;
        } else if let Some(description) = line.strip_prefix("- ") {
            examples_started = true;
            if description.trim().is_empty() {
                error(
                    &mut diagnostics,
                    line_number,
                    "empty-example-description",
                    "example descriptions cannot be empty",
                );
            } else if !description.trim_end().ends_with(':') {
                warning(
                    &mut diagnostics,
                    line_number,
                    "example-description-colon",
                    "tldr example descriptions should end with a colon",
                );
            }
            awaiting_command = Some(line_number);
        } else if is_command_line(line) {
            error(
                &mut diagnostics,
                line_number,
                "command-without-description",
                "each command must follow a `- <description>:` line",
            );
            validate_placeholders(
                command_body(line).expect("is_command_line checked the shape"),
                line_number,
                &mut diagnostics,
            );
        } else if line.starts_with('#') {
            error(
                &mut diagnostics,
                line_number,
                "extra-heading",
                "additional headings are outside the supported tldr subset",
            );
        } else {
            error(
                &mut diagnostics,
                line_number,
                "unsupported-block",
                "expected a description, example description, or backtick-wrapped command",
            );
        }
    }

    if let Some(description_line) = awaiting_command {
        error(
            &mut diagnostics,
            lines.len().max(description_line),
            "missing-example-command",
            &format!(
                "example description on line {description_line} must be followed by one backtick-wrapped command"
            ),
        );
    }
    if description_count == 0 {
        error(
            &mut diagnostics,
            first_nonempty + 1,
            "missing-description",
            "page must contain at least one `> <description>` line",
        );
    }
    if example_count == 0 {
        error(
            &mut diagnostics,
            lines.len().max(1),
            "missing-example",
            "page must contain at least one description and command example",
        );
    } else if example_count > 8 {
        warning(
            &mut diagnostics,
            lines.len(),
            "too-many-examples",
            "the tldr style guide recommends no more than eight examples",
        );
    }

    validation(diagnostics)
}

pub fn validate_tldr_file(path: &Path, expected_topic: Option<&str>) -> Result<TldrValidation> {
    let content = read_external_utf8(path, MAX_PAGE_BYTES, "page")?;
    let derived_topic;
    let topic = match expected_topic {
        Some(topic) => {
            validate_topic(topic)?;
            topic
        }
        None => {
            derived_topic = topic_from_source(path)?;
            &derived_topic
        }
    };
    Ok(validate_tldr_page(&content, Some(topic)))
}

impl Vault {
    pub fn validate_tldr_topic(&self, topic: &str) -> Result<TldrValidation> {
        let page = self.read(topic)?;
        Ok(validate_tldr_page(&page.content, Some(topic)))
    }

    pub fn import_tldr_page(
        &self,
        source: &Path,
        options: &TldrImportOptions,
    ) -> Result<TldrImportReport> {
        let content = read_external_utf8(source, MAX_PAGE_BYTES, "page")?;
        let topic = match &options.topic {
            Some(topic) => topic.clone(),
            None => topic_from_source(source)?,
        };
        validate_topic(&topic)?;
        let validation = validate_tldr_page(&content, Some(&topic));
        if !validation.valid {
            return Err(Error::InvalidTldr {
                path: source.to_path_buf(),
                diagnostics: validation.diagnostics,
            });
        }

        let source_metadata = metadata_path(source)?;
        let copied_metadata = match fs::symlink_metadata(&source_metadata) {
            Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                return Err(Error::UnsafeSymlink(source_metadata));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(Error::UnsafeFileType(source_metadata));
            }
            Ok(_) => Some(read_external_utf8(
                &source_metadata,
                MAX_METADATA_BYTES,
                "metadata",
            )?),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };

        if copied_metadata.is_some() && (options.page_license.is_some() || options.source.is_some())
        {
            return Err(Error::InvalidImportMetadata(
                "cannot merge provenance options into an existing source sidecar".to_owned(),
            ));
        }
        let generated_metadata = if copied_metadata.is_none()
            && (options.page_license.is_some() || options.source.is_some())
        {
            Some(render_metadata(options)?)
        } else {
            None
        };
        let metadata = copied_metadata.as_ref().or(generated_metadata.as_ref());

        let page = self.create_with_content(&topic, &content)?;
        let target_metadata = metadata_path(&page.path)?;
        if let Some(metadata) = metadata {
            if let Err(error) = write_new_file_atomic(&target_metadata, metadata.as_bytes()) {
                rollback_imported_page(&page.path, &content);
                return Err(error);
            }
        }

        Ok(TldrImportReport {
            topic,
            page_path: page.path,
            metadata_path: metadata.map(|_| target_metadata),
            preserved_source_metadata: copied_metadata.is_some(),
            validation,
        })
    }

    pub fn export_tldr_pages(&self, destination: &Path) -> Result<TldrExportReport> {
        reject_export_directory(destination)?;
        let pages = self.list()?;
        let names = export_names(pages.iter().map(|page| page.topic.as_str()))?;
        let mut planned = Vec::new();
        let mut mappings = Vec::with_capacity(pages.len());

        for page in pages {
            let content = self.read(&page.topic)?.content;
            let validation = validate_tldr_page(&content, Some(&page.topic));
            if !validation.valid {
                return Err(Error::InvalidTldr {
                    path: page.path,
                    diagnostics: validation.diagnostics,
                });
            }

            let (stem, collision_resolved) = names
                .get(&page.topic)
                .expect("every listed topic receives an export name");
            let page_file = format!("{stem}{PAGE_SUFFIX}");
            let page_destination = destination.join(&page_file);
            planned.push(PlannedExport {
                destination: page_destination,
                content: content.into_bytes(),
            });

            let source_metadata = metadata_path(&page.path)?;
            let metadata_file = match fs::symlink_metadata(&source_metadata) {
                Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                    return Err(Error::UnsafeSymlink(source_metadata));
                }
                Ok(metadata) if !metadata.is_file() => {
                    return Err(Error::UnsafeFileType(source_metadata));
                }
                Ok(_) => {
                    let metadata =
                        read_external_bytes(&source_metadata, MAX_METADATA_BYTES, "metadata")?;
                    let filename = format!("{stem}{METADATA_SUFFIX}");
                    let target = destination.join(&filename);
                    planned.push(PlannedExport {
                        destination: target,
                        content: metadata,
                    });
                    Some(filename)
                }
                Err(error) if error.kind() == ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };

            mappings.push(TldrExportMapping {
                topic: page.topic,
                page_file,
                metadata_file,
                collision_resolved: *collision_resolved,
                validation,
            });
        }

        preflight_export_targets(destination, &planned)?;
        fs::create_dir_all(destination)?;
        reject_export_directory(destination)?;
        let prepared = planned
            .into_iter()
            .map(PreparedExport::new)
            .collect::<Result<Vec<_>>>()?;
        commit_exports(prepared)?;
        Ok(TldrExportReport {
            destination: destination.to_path_buf(),
            mappings,
        })
    }
}

struct PlannedExport {
    destination: PathBuf,
    content: Vec<u8>,
}

struct PreparedExport {
    destination: PathBuf,
    temporary: NamedTempFile,
    expected_sha256: String,
}

impl PreparedExport {
    fn new(planned: PlannedExport) -> Result<Self> {
        let parent = planned
            .destination
            .parent()
            .expect("an export destination always has a parent");
        reject_export_directory(parent)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(&planned.content)?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        Ok(Self {
            destination: planned.destination,
            temporary,
            expected_sha256: content_sha256(&planned.content),
        })
    }
}

fn preflight_export_targets(destination: &Path, planned: &[PlannedExport]) -> Result<()> {
    let mut planned_names = std::collections::BTreeSet::new();
    for item in planned {
        reject_export_target(&item.destination)?;
        let filename = item
            .destination
            .file_name()
            .and_then(|name| name.to_str())
            .expect("MyHelp export filenames are ASCII");
        if !planned_names.insert(filename.to_ascii_lowercase()) {
            return Err(Error::InvalidImportMetadata(
                "export mapping produced duplicate portable filenames".to_owned(),
            ));
        }
    }

    match fs::read_dir(destination) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| planned_names.contains(&name.to_ascii_lowercase()))
                {
                    return Err(Error::ExportDestinationOccupied(entry.path()));
                }
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn commit_exports(prepared: Vec<PreparedExport>) -> Result<()> {
    let mut committed = Vec::new();
    for item in prepared {
        reject_export_target(&item.destination)?;
        let temporary_path = item.temporary.path().to_path_buf();
        if let Err(error) = no_replace_rename(&temporary_path, &item.destination) {
            rollback_exports(&committed);
            return Err(if error.kind() == ErrorKind::AlreadyExists {
                Error::ExportDestinationOccupied(item.destination)
            } else {
                Error::Io(error)
            });
        }
        committed.push((item.destination, item.expected_sha256));
    }
    Ok(())
}

fn rollback_exports(committed: &[(PathBuf, String)]) {
    for (path, expected_sha256) in committed.iter().rev() {
        let unchanged = fs::read(path)
            .ok()
            .is_some_and(|content| content_sha256(&content) == *expected_sha256);
        if unchanged {
            let _ = fs::remove_file(path);
        }
    }
}

fn rollback_imported_page(path: &Path, expected: &str) {
    let unchanged = read_snapshot(path)
        .ok()
        .is_some_and(|(content, _)| content == expected);
    if unchanged {
        let _ = fs::remove_file(path);
    }
}

fn reject_export_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_symlink_or_reparse(&metadata) => {
            Err(Error::UnsafeSymlink(path.to_path_buf()))
        }
        Ok(metadata) if metadata.is_dir() => reject_existing_path_components(path),
        Ok(_) => Err(Error::UnsafeFileType(path.to_path_buf())),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                reject_existing_path_components(parent)?;
            }
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn reject_existing_path_components(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                return Err(Error::UnsafeSymlink(current));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn reject_export_target(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_symlink_or_reparse(&metadata) => {
            Err(Error::UnsafeSymlink(path.to_path_buf()))
        }
        Ok(_) => Err(Error::ExportDestinationOccupied(path.to_path_buf())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_new_file_atomic(path: &Path, content: &[u8]) -> Result<()> {
    reject_export_target(path)?;
    let parent = path.parent().expect("metadata path has a parent");
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    no_replace_rename(temporary.path(), path).map_err(|error| {
        if error.kind() == ErrorKind::AlreadyExists {
            Error::ExportDestinationOccupied(path.to_path_buf())
        } else {
            Error::Io(error)
        }
    })
}

fn read_external_utf8(path: &Path, max_bytes: usize, kind: &str) -> Result<String> {
    let mut file = checked_external_file(path, max_bytes, kind)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    reject_external_size(path, bytes.len() as u64, max_bytes, kind)?;
    let content = String::from_utf8(bytes).map_err(|error| {
        Error::Io(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("{kind} is not valid UTF-8: {error}"),
        ))
    })?;
    Ok(content)
}

fn read_external_bytes(path: &Path, max_bytes: usize, kind: &str) -> Result<Vec<u8>> {
    let mut file = checked_external_file(path, max_bytes, kind)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    reject_external_size(path, bytes.len() as u64, max_bytes, kind)?;
    Ok(bytes)
}

fn checked_external_file(path: &Path, max_bytes: usize, kind: &str) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if is_symlink_or_reparse(&metadata) {
        return Err(Error::UnsafeSymlink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(Error::UnsafeFileType(path.to_path_buf()));
    }
    reject_external_size(path, metadata.len(), max_bytes, kind)?;
    let file = open_read_nofollow(path)?;
    let metadata = file.metadata()?;
    if is_symlink_or_reparse(&metadata) {
        return Err(Error::UnsafeSymlink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(Error::UnsafeFileType(path.to_path_buf()));
    }
    reject_external_size(path, metadata.len(), max_bytes, kind)?;
    Ok(file)
}

fn reject_external_size(path: &Path, size: u64, max_bytes: usize, kind: &str) -> Result<()> {
    if size > max_bytes as u64 {
        if kind == "page" {
            reject_oversized_page(path, size)
        } else {
            Err(Error::InputTooLarge {
                field: "metadata file",
                max_bytes,
            })
        }
    } else {
        Ok(())
    }
}

fn topic_from_source(source: &Path) -> Result<String> {
    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::InvalidTopic(source.display().to_string()))?;
    let topic = filename
        .strip_suffix(PAGE_SUFFIX)
        .or_else(|| filename.strip_suffix(".md"))
        .filter(|topic| !topic.is_empty())
        .ok_or_else(|| Error::InvalidTopic(filename.to_owned()))?
        .to_lowercase();
    validate_topic(&topic)?;
    Ok(topic)
}

fn render_metadata(options: &TldrImportOptions) -> Result<String> {
    let mut output = format!("schema_version: 1\nid: {}\n", Uuid::new_v4());
    if let Some(license) = &options.page_license {
        validate_metadata_scalar(license, "page license")?;
        output.push_str("license: ");
        output.push_str(&yaml_string(license));
        output.push('\n');
    }
    if let Some(source) = &options.source {
        validate_absolute_url(&source.url)?;
        validate_metadata_scalar(&source.url, "source URL")?;
        output.push_str("sources:\n  - url: ");
        output.push_str(&yaml_string(&source.url));
        output.push('\n');
        for (key, value) in [
            ("title", source.title.as_deref()),
            ("license", source.license.as_deref()),
            ("attribution", source.attribution.as_deref()),
        ] {
            if let Some(value) = value {
                validate_metadata_scalar(value, "source metadata")?;
                output.push_str("    ");
                output.push_str(key);
                output.push_str(": ");
                output.push_str(&yaml_string(value));
                output.push('\n');
            }
        }
    }
    Ok(output)
}

fn validate_metadata_scalar(value: &str, field: &'static str) -> Result<()> {
    if value.is_empty() {
        return Err(Error::InvalidImportMetadata(format!(
            "{field} cannot be empty"
        )));
    }
    if value.len() > 4096 {
        return Err(Error::InputTooLarge {
            field,
            max_bytes: 4096,
        });
    }
    Ok(())
}

fn validate_absolute_url(value: &str) -> Result<()> {
    validate_metadata_scalar(value, "source URL")?;
    let valid_characters = !value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace());
    let valid_scheme = value.split_once(':').is_some_and(|(scheme, rest)| {
        let mut characters = scheme.chars();
        characters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && characters.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
            && !rest.is_empty()
    });
    if valid_characters && valid_scheme {
        Ok(())
    } else {
        Err(Error::InvalidImportMetadata(
            "source URL must be an absolute URL without whitespace".to_owned(),
        ))
    }
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a Rust string cannot fail")
}

fn export_names<'a>(
    topics: impl Iterator<Item = &'a str>,
) -> Result<BTreeMap<String, (String, bool)>> {
    let topics = topics.map(str::to_owned).collect::<Vec<_>>();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for topic in &topics {
        groups
            .entry(flatten_topic(topic).to_lowercase())
            .or_default()
            .push(topic.clone());
    }

    let mut names = BTreeMap::new();
    for group in groups.values_mut() {
        group.sort();
        let collision = group.len() > 1;
        for topic in group.iter() {
            let base = flatten_topic(topic);
            let stem = if collision {
                with_topic_hash(&base, topic)
            } else {
                base
            };
            names.insert(topic.clone(), (stem, collision));
        }
    }

    let mut final_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (topic, (stem, _)) in &names {
        final_groups
            .entry(stem.to_lowercase())
            .or_default()
            .push(topic.clone());
    }
    for group in final_groups.values().filter(|group| group.len() > 1) {
        for topic in group {
            let (stem, collision) = names
                .get_mut(topic)
                .expect("final collision refers to a mapped topic");
            *stem = with_topic_hash(stem, topic);
            *collision = true;
        }
    }

    let unique = names
        .values()
        .map(|(stem, _)| stem.to_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != names.len() {
        return Err(Error::InvalidImportMetadata(
            "could not derive unique export filenames".to_owned(),
        ));
    }
    Ok(names)
}

fn flatten_topic(topic: &str) -> String {
    let mut stem = String::new();
    for byte in topic.to_lowercase().bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'+' => {
                stem.push(char::from(byte));
            }
            b'/' | b' ' => stem.push('-'),
            _ => stem.push_str(&format!("~{byte:02x}")),
        }
    }
    if stem.is_empty() {
        stem.push_str("page");
    }
    if stem.starts_with('.') || is_windows_reserved_name(&stem) {
        stem.insert_str(0, "myhelp-");
    }
    if stem.ends_with('.') {
        stem.push_str("-page");
    }
    if stem.len() > MAX_EXPORT_STEM_BYTES {
        stem = with_topic_hash(&stem[..MAX_EXPORT_STEM_BYTES], topic);
    }
    stem
}

fn with_topic_hash(base: &str, topic: &str) -> String {
    let suffix = &content_sha256(topic.as_bytes())[..12];
    let max_base = MAX_EXPORT_STEM_BYTES.saturating_sub(22);
    let base = &base[..base.len().min(max_base)];
    format!("{base}--myhelp-{suffix}")
}

fn is_windows_reserved_name(stem: &str) -> bool {
    let basename = stem.split('.').next().unwrap_or(stem);
    matches!(basename, "con" | "prn" | "aux" | "nul")
        || (basename.len() == 4
            && (basename.starts_with("com") || basename.starts_with("lpt"))
            && matches!(basename.as_bytes()[3], b'1'..=b'9'))
}

fn is_command_line(line: &str) -> bool {
    command_body(line).is_some()
}

fn command_body(line: &str) -> Option<&str> {
    line.strip_prefix('`')
        .and_then(|line| line.strip_suffix('`'))
        .filter(|line| !line.is_empty() && !line.contains('`'))
}

fn validate_placeholders(command: &str, line: usize, diagnostics: &mut Vec<TldrDiagnostic>) {
    let bytes = command.as_bytes();
    let mut index = 0_usize;
    while index < bytes.len() {
        if escaped_brace_pair(bytes, index) {
            index += 4;
            continue;
        }
        if bytes[index..].starts_with(b"{{") {
            let start = index;
            index += 2;
            let content_start = index;
            let mut brace_depth = 0_usize;
            let mut closed = None;
            while index < bytes.len() {
                if bytes[index..].starts_with(b"{{") {
                    error(
                        diagnostics,
                        line,
                        "nested-placeholder",
                        "tldr placeholders cannot be nested",
                    );
                    index += 2;
                    continue;
                }
                match bytes[index] {
                    b'{' => {
                        brace_depth += 1;
                        index += 1;
                    }
                    b'}' if brace_depth > 0 => {
                        brace_depth -= 1;
                        index += 1;
                    }
                    b'}' if bytes[index..].starts_with(b"}}") => {
                        closed = Some(index);
                        index += 2;
                        break;
                    }
                    _ => index += 1,
                }
            }
            let Some(content_end) = closed else {
                error(
                    diagnostics,
                    line,
                    "unclosed-placeholder",
                    "placeholder opened with `{{` but has no matching `}}`",
                );
                break;
            };
            let content = &command[content_start..content_end];
            if content.is_empty() {
                error(
                    diagnostics,
                    line,
                    "empty-placeholder",
                    "placeholders cannot be empty",
                );
            }
            if content.starts_with('[') || content.ends_with(']') {
                let option = content
                    .strip_prefix('[')
                    .and_then(|value| value.strip_suffix(']'));
                let valid = option.is_some_and(|value| {
                    let choices = value.split('|').collect::<Vec<_>>();
                    choices.len() == 2 && choices.iter().all(|choice| !choice.is_empty())
                });
                if !valid {
                    error(
                        diagnostics,
                        line,
                        "invalid-option-placeholder",
                        "option placeholders must use `{{[short|long]}}`",
                    );
                }
            }
            if start == index {
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"}}") {
            error(
                diagnostics,
                line,
                "unexpected-placeholder-close",
                "`}}` appears without a matching `{{`",
            );
            index += 2;
            continue;
        }
        index += 1;
    }
}

fn escaped_brace_pair(bytes: &[u8], index: usize) -> bool {
    bytes
        .get(index..index.saturating_add(4))
        .is_some_and(|value| value == b"\\{\\{" || value == b"\\}\\}")
}

fn validation(diagnostics: Vec<TldrDiagnostic>) -> TldrValidation {
    TldrValidation {
        valid: !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.level == TldrDiagnosticLevel::Error),
        diagnostics,
    }
}

fn error(diagnostics: &mut Vec<TldrDiagnostic>, line: usize, code: &str, message: &str) {
    diagnostic(diagnostics, TldrDiagnosticLevel::Error, line, code, message);
}

fn warning(diagnostics: &mut Vec<TldrDiagnostic>, line: usize, code: &str, message: &str) {
    diagnostic(
        diagnostics,
        TldrDiagnosticLevel::Warning,
        line,
        code,
        message,
    );
}

fn diagnostic(
    diagnostics: &mut Vec<TldrDiagnostic>,
    level: TldrDiagnosticLevel,
    line: usize,
    code: &str,
    message: &str,
) {
    diagnostics.push(TldrDiagnostic {
        level,
        line,
        code: code.to_owned(),
        message: message.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTF8_PAGE: &str =
        "# 挨拶\n\n> 日本語の例です。\n\n- 名前を表示する:\n\n`printf '%s\\n' {{名前}}`\n";

    #[test]
    fn validates_utf8_and_reports_line_oriented_errors() {
        let valid = validate_tldr_page(UTF8_PAGE, Some("挨拶"));
        assert!(valid.valid, "{:?}", valid.diagnostics);

        let invalid = validate_tldr_page(
            "# Broken\n\n- Missing description\n\nnot a command\n",
            Some("broken"),
        );
        assert!(!invalid.valid);
        assert!(invalid.diagnostics.iter().all(|item| item.line > 0));
        assert!(
            invalid
                .diagnostics
                .iter()
                .any(|item| item.code == "missing-example-command")
        );
        assert!(
            invalid
                .diagnostics
                .iter()
                .any(|item| item.code == "missing-description")
        );
    }

    #[test]
    fn validation_rejects_non_utf8_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("invalid.md");
        fs::write(&source, [0xff, 0xfe]).expect("non-UTF-8 source");

        assert!(matches!(
            validate_tldr_file(&source, None),
            Err(Error::Io(error)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    #[test]
    fn validates_tldr_placeholder_edge_cases() {
        let page = "# Git\n\n> Work with repositories.\n\n- Inspect a stash:\n\n`git stash show {{stash@{0}}}`\n\n- Choose an option:\n\n`git add {{[-A|--all]}}`\n\n- Preserve template braces:\n\n`echo \\{\\{value\\}\\}`\n";
        let validation = validate_tldr_page(page, Some("git"));
        assert!(validation.valid, "{:?}", validation.diagnostics);
    }

    #[test]
    fn import_and_export_preserve_crlf_exactly() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("git.md");
        let content =
            "# Git\r\n\r\n> Work with repositories.\r\n\r\n- Show status:\r\n\r\n`git status`\r\n";
        fs::write(&source, content).expect("CRLF source");
        assert!(
            validate_tldr_file(&source, None)
                .expect("validate CRLF")
                .valid
        );

        let vault = Vault::new(directory.path().join("vault"));
        let imported = vault
            .import_tldr_page(&source, &TldrImportOptions::default())
            .expect("import CRLF");
        assert_eq!(
            fs::read(&imported.page_path).expect("imported bytes"),
            content.as_bytes()
        );

        let export = vault
            .export_tldr_pages(&directory.path().join("export"))
            .expect("export CRLF");
        assert_eq!(
            fs::read(
                directory
                    .path()
                    .join("export")
                    .join(&export.mappings[0].page_file)
            )
            .expect("exported bytes"),
            content.as_bytes()
        );
    }

    #[test]
    fn import_and_export_round_trip_utf8_and_metadata() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("hello.page.md");
        fs::write(&source, UTF8_PAGE).expect("source page");
        fs::write(
            directory.path().join("hello.page.meta.yaml"),
            "schema_version: 1\nid: 550e8400-e29b-41d4-a716-446655440000\nlicense: MIT\n",
        )
        .expect("source metadata");
        let vault = Vault::new(directory.path().join("vault"));

        let imported = vault
            .import_tldr_page(
                &source,
                &TldrImportOptions {
                    topic: Some("日本語/挨拶".to_owned()),
                    ..TldrImportOptions::default()
                },
            )
            .expect("import");
        assert_eq!(
            fs::read_to_string(&imported.page_path).expect("imported page"),
            UTF8_PAGE
        );
        assert!(imported.preserved_source_metadata);

        let export = vault
            .export_tldr_pages(&directory.path().join("export"))
            .expect("export");
        assert_eq!(export.mappings.len(), 1);
        let mapping = &export.mappings[0];
        assert_eq!(
            fs::read_to_string(directory.path().join("export").join(&mapping.page_file))
                .expect("exported page"),
            UTF8_PAGE
        );
        assert_eq!(
            fs::read_to_string(
                directory
                    .path()
                    .join("export")
                    .join(mapping.metadata_file.as_ref().expect("metadata mapping"))
            )
            .expect("exported metadata"),
            "schema_version: 1\nid: 550e8400-e29b-41d4-a716-446655440000\nlicense: MIT\n"
        );
    }

    #[test]
    fn nested_topic_collisions_get_stable_distinct_names() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        for topic in ["foo/bar", "foo-bar", "FOO-BAR"] {
            vault
                .create_with_content(
                    topic,
                    &format!("# {topic}\n\n> A page.\n\n- Show it:\n\n`echo {topic}`\n"),
                )
                .expect("create colliding topic");
        }

        let first = vault
            .export_tldr_pages(&directory.path().join("first"))
            .expect("first export");
        let second = vault
            .export_tldr_pages(&directory.path().join("second"))
            .expect("second export");
        let first_names = first
            .mappings
            .iter()
            .map(|mapping| (&mapping.topic, &mapping.page_file))
            .collect::<Vec<_>>();
        let second_names = second
            .mappings
            .iter()
            .map(|mapping| (&mapping.topic, &mapping.page_file))
            .collect::<Vec<_>>();
        assert_eq!(first_names, second_names);
        assert!(
            first
                .mappings
                .iter()
                .all(|mapping| mapping.collision_resolved)
        );
        let unique = first
            .mappings
            .iter()
            .map(|mapping| mapping.page_file.to_lowercase())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn export_preflights_every_target_without_overwriting() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        vault
            .create_with_content("alpha", "# Alpha\n\n> A page.\n\n- Run it:\n\n`alpha`\n")
            .expect("alpha");
        vault
            .create_with_content("beta", "# Beta\n\n> A page.\n\n- Run it:\n\n`beta`\n")
            .expect("beta");
        let destination = directory.path().join("export");
        fs::create_dir(&destination).expect("export directory");
        fs::write(destination.join("beta.page.md"), "unrelated\n").expect("occupied target");

        assert!(matches!(
            vault.export_tldr_pages(&destination),
            Err(Error::ExportDestinationOccupied(_))
        ));
        assert!(!destination.join("alpha.page.md").exists());
        assert_eq!(
            fs::read_to_string(destination.join("beta.page.md")).expect("unrelated file"),
            "unrelated\n"
        );
    }

    #[test]
    fn export_rejects_existing_portable_case_collision() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        vault
            .create_with_content("git", "# Git\n\n> A page.\n\n- Run it:\n\n`git`\n")
            .expect("git");
        let destination = directory.path().join("export");
        fs::create_dir(&destination).expect("export directory");
        fs::write(destination.join("GIT.page.md"), "unrelated\n").expect("case collision");

        assert!(matches!(
            vault.export_tldr_pages(&destination),
            Err(Error::ExportDestinationOccupied(_))
        ));
        assert!(!destination.join("git.page.md").exists());
        assert_eq!(
            fs::read_to_string(destination.join("GIT.page.md")).expect("unrelated file"),
            "unrelated\n"
        );
    }

    #[test]
    fn export_names_are_portable_bounded_and_ascii() {
        let topics = [
            "CON".to_owned(),
            "LPT9.txt".to_owned(),
            ".hidden".to_owned(),
            "café/工具".to_owned(),
            "a".repeat(220),
        ];
        let names = export_names(topics.iter().map(String::as_str)).expect("portable mappings");

        assert!(names["CON"].0.starts_with("myhelp-con"));
        assert!(names["LPT9.txt"].0.starts_with("myhelp-lpt9"));
        assert!(names[".hidden"].0.starts_with("myhelp-"));
        assert!(
            names
                .values()
                .all(|(stem, _)| stem.is_ascii() && stem.len() <= MAX_EXPORT_STEM_BYTES)
        );
        assert!(
            names[topics.last().expect("long topic")]
                .0
                .contains("--myhelp-")
        );
    }

    #[cfg(unix)]
    #[test]
    fn import_and_export_reject_symlink_endpoints() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("git.md");
        fs::write(
            &source,
            "# Git\n\n> Work with repositories.\n\n- Show status:\n\n`git status`\n",
        )
        .expect("source");
        let source_link = directory.path().join("linked.md");
        symlink(&source, &source_link).expect("source symlink");
        let vault = Vault::new(directory.path().join("vault"));

        assert!(matches!(
            vault.import_tldr_page(&source_link, &TldrImportOptions::default()),
            Err(Error::UnsafeSymlink(_))
        ));

        vault
            .import_tldr_page(&source, &TldrImportOptions::default())
            .expect("regular import");
        let real_destination = directory.path().join("real-export");
        fs::create_dir(&real_destination).expect("real export directory");
        let destination_link = directory.path().join("linked-export");
        symlink(&real_destination, &destination_link).expect("destination symlink");
        assert!(matches!(
            vault.export_tldr_pages(&destination_link),
            Err(Error::UnsafeSymlink(_))
        ));
        assert!(
            fs::read_dir(&real_destination)
                .expect("real destination")
                .next()
                .is_none()
        );
    }

    #[test]
    fn provenance_generates_an_adr_compatible_sidecar() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("git.md");
        fs::write(
            &source,
            "# Git\n\n> Work with repositories.\n\n- Show status:\n\n`git status`\n",
        )
        .expect("source");
        let vault = Vault::new(directory.path().join("vault"));
        let report = vault
            .import_tldr_page(
                &source,
                &TldrImportOptions {
                    topic: None,
                    page_license: Some("CC-BY-4.0".to_owned()),
                    source: Some(TldrSource {
                        url: "https://github.com/tldr-pages/tldr".to_owned(),
                        title: Some("tldr pages".to_owned()),
                        license: Some("CC-BY-4.0".to_owned()),
                        attribution: Some("tldr-pages contributors".to_owned()),
                    }),
                },
            )
            .expect("import with provenance");
        let metadata =
            fs::read_to_string(report.metadata_path.expect("metadata sidecar")).expect("metadata");
        assert!(metadata.starts_with("schema_version: 1\nid: "));
        assert!(metadata.contains("license: \"CC-BY-4.0\""));
        assert!(metadata.contains("url: \"https://github.com/tldr-pages/tldr\""));
        assert!(metadata.contains("attribution: \"tldr-pages contributors\""));
    }
}
