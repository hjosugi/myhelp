use super::{
    AdapterCompatibility, AdapterConversionReport, AdapterDiagnostic, AdapterDiagnosticLevel,
    AdapterDisposition, ForeignAdapter, ForeignFormat,
};
use crate::external::{ExternalFileKind, read_external_utf8};
use crate::{
    Error, MAX_PAGE_BYTES, Result, TldrDiagnosticLevel, validate_tldr_page, validate_topic,
};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
pub struct NaviAdapter;

pub fn inspect_navi_file(
    source_path: &Path,
    topic: Option<&str>,
) -> Result<AdapterConversionReport> {
    validate_source_filename(source_path)?;
    let source = read_external_utf8(
        source_path,
        MAX_PAGE_BYTES,
        ExternalFileKind::Input("navi cheatsheet"),
    )?;
    let topic = match topic {
        Some(topic) => topic.to_owned(),
        None => topic_from_source(source_path)?,
    };
    NaviAdapter.inspect(source_path, &source, &topic)
}

impl ForeignAdapter for NaviAdapter {
    fn format(&self) -> ForeignFormat {
        ForeignFormat::Navi
    }

    fn inspect(
        &self,
        source_path: &Path,
        source: &str,
        topic: &str,
    ) -> Result<AdapterConversionReport> {
        validate_topic(topic)?;

        let mut state = ParseState::default();
        for (index, line) in source.lines().enumerate() {
            state.read_line(index + 1, line);
        }
        state.finish(source.lines().count().max(1));

        let generated_page = state.render_page(topic);
        if let Some(page) = &generated_page {
            if page.len() > MAX_PAGE_BYTES {
                state.error(
                    None,
                    "generated-page-too-large",
                    "generatedPage",
                    AdapterDisposition::Unsupported,
                    format!("the generated page exceeds MyHelp's {MAX_PAGE_BYTES}-byte page limit"),
                );
            } else {
                let validation = validate_tldr_page(page, Some(topic));
                for diagnostic in validation.diagnostics {
                    let level = match diagnostic.level {
                        TldrDiagnosticLevel::Error => AdapterDiagnosticLevel::Error,
                        TldrDiagnosticLevel::Warning => AdapterDiagnosticLevel::Warning,
                    };
                    state.diagnostics.push(AdapterDiagnostic {
                        level,
                        line: None,
                        code: format!("generated-{}", diagnostic.code),
                        source_field: "generatedPage".to_owned(),
                        disposition: AdapterDisposition::Mapped,
                        message: format!(
                            "generated tldr line {}: {}",
                            diagnostic.line, diagnostic.message
                        ),
                    });
                }
            }
        }

        state.info(
            None,
            "layout-normalized",
            "layout",
            AdapterDisposition::ReportedOnly,
            "navi layout and ordering are normalized into a tldr-style preview; reverse conversion is not lossless",
        );

        let convertible = generated_page.is_some()
            && !state
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.level == AdapterDiagnosticLevel::Error);

        Ok(AdapterConversionReport {
            format: self.format(),
            compatibility: AdapterCompatibility::LossyImportPreview,
            dry_run: true,
            source_path: source_path.to_path_buf(),
            topic: topic.to_owned(),
            convertible,
            lossless: false,
            source_tags: state.tags,
            generated_page,
            diagnostics: state.diagnostics,
        })
    }
}

#[derive(Default)]
struct ParseState {
    context_count: usize,
    tags: Vec<String>,
    description: Option<(usize, String)>,
    snippet: Vec<(usize, String)>,
    examples: Vec<Example>,
    diagnostics: Vec<AdapterDiagnostic>,
    inside_code_fence: bool,
    variable_continuation: bool,
}

struct Example {
    description: String,
    command: String,
}

impl ParseState {
    fn read_line(&mut self, line_number: usize, line: &str) {
        if self.variable_continuation {
            self.variable_continuation = line.ends_with('\\');
            return;
        }

        if line.starts_with('%') {
            self.flush_example();
            self.description = None;
            self.context_count += 1;
            if self.context_count > 1 {
                self.error(
                    Some(line_number),
                    "multiple-contexts",
                    "context",
                    AdapterDisposition::Unsupported,
                    "one navi file contains multiple `%` contexts; the prototype will not invent multiple MyHelp topic names",
                );
            }
            self.read_tags(line_number, line);
            return;
        }

        if line.starts_with('#') {
            self.flush_example();
            let description = value_after_prefix(line_number, line, '#', self);
            if description.is_empty() {
                self.error(
                    Some(line_number),
                    "empty-description",
                    "description",
                    AdapterDisposition::Unsupported,
                    "a navi `#` description cannot be empty",
                );
            } else if description.chars().any(char::is_control) {
                self.error(
                    Some(line_number),
                    "control-character",
                    "description",
                    AdapterDisposition::Unsupported,
                    "description contains a control character that cannot be emitted safely",
                );
            } else {
                self.description = Some((line_number, description));
            }
            return;
        }

        if line.starts_with('@') {
            let dependency = value_after_prefix(line_number, line, '@', self);
            self.warning(
                Some(line_number),
                "context-extension-unsupported",
                "context.extension",
                AdapterDisposition::Unsupported,
                format!(
                    "navi context extension `{dependency}` is reported but not resolved; MyHelp never evaluates its variable sources"
                ),
            );
            return;
        }

        if line.starts_with(';') {
            self.info(
                Some(line_number),
                "metacomment-ignored",
                "metacomment",
                AdapterDisposition::Unsupported,
                "navi ignores this metacomment and the generated page does not retain it",
            );
            return;
        }

        if !self.inside_code_fence && line.starts_with('$') && line.contains(':') {
            self.flush_example();
            let definition = line.trim_start_matches('$');
            let variable = definition
                .split_once(':')
                .map_or("", |(name, _)| name)
                .trim();
            let field = if variable.is_empty() {
                "variable.source".to_owned()
            } else {
                format!("variable.{variable}.source")
            };
            self.warning(
                Some(line_number),
                "dynamic-variable-source-omitted",
                &field,
                AdapterDisposition::Unsupported,
                "the command-backed suggestion source is not executed and is omitted; its `<variable>` placeholder can still be mapped",
            );
            self.variable_continuation = line.ends_with('\\');
            return;
        }

        if line.starts_with("```") {
            self.inside_code_fence = !self.inside_code_fence;
            return;
        }

        if line.is_empty() {
            if !self.snippet.is_empty() {
                self.snippet.push((line_number, String::new()));
            }
            return;
        }

        self.snippet.push((line_number, line.to_owned()));
    }

    fn finish(&mut self, last_line: usize) {
        if self.variable_continuation {
            self.warning(
                Some(last_line),
                "unterminated-variable-continuation",
                "variable.source",
                AdapterDisposition::Unsupported,
                "the final dynamic variable source ends with a continuation; it remains omitted",
            );
        }
        if self.inside_code_fence {
            self.error(
                Some(last_line),
                "unclosed-code-fence",
                "snippet",
                AdapterDisposition::Unsupported,
                "the navi Markdown code fence is not closed",
            );
        }
        self.flush_example();
        if self.context_count == 0 {
            self.error(
                None,
                "missing-context",
                "context",
                AdapterDisposition::Unsupported,
                "a navi cheatsheet must declare one `%` context",
            );
        }
        if self.examples.is_empty() {
            self.error(
                None,
                "missing-example",
                "snippet",
                AdapterDisposition::Unsupported,
                "no single-line, described command can be converted",
            );
        }
    }

    fn read_tags(&mut self, line_number: usize, line: &str) {
        let value = value_after_prefix(line_number, line, '%', self);
        let tags = value
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();
        if tags.is_empty() {
            self.error(
                Some(line_number),
                "empty-context",
                "context.tags",
                AdapterDisposition::Unsupported,
                "a navi `%` context must contain at least one tag",
            );
            return;
        }

        for tag in tags {
            if !self.tags.iter().any(|existing| existing == tag) {
                self.tags.push(tag.to_owned());
            }
        }
        self.warning(
            Some(line_number),
            "tags-reported-only",
            "context.tags",
            AdapterDisposition::ReportedOnly,
            "navi context tags are included in this report but are not written because general MyHelp metadata parsing is still pending",
        );
    }

    fn flush_example(&mut self) {
        while self.snippet.last().is_some_and(|(_, line)| line.is_empty()) {
            self.snippet.pop();
        }

        let Some((description_line, description)) = self.description.take() else {
            if let Some((line, _)) = self.snippet.first() {
                self.error(
                    Some(*line),
                    "missing-description",
                    "snippet",
                    AdapterDisposition::Unsupported,
                    "navi accepts this snippet, but the tldr subset requires a preceding `#` description",
                );
            }
            self.snippet.clear();
            return;
        };

        if self.snippet.is_empty() {
            self.warning(
                Some(description_line),
                "description-without-snippet",
                "description",
                AdapterDisposition::Unsupported,
                "the description has no command and is omitted from the generated page",
            );
            return;
        }

        if self.snippet.len() != 1 {
            let line = self.snippet.first().map(|(line, _)| *line);
            self.error(
                line,
                "multiline-snippet-unsupported",
                "snippet",
                AdapterDisposition::Unsupported,
                "navi multiline snippets cannot be represented by the supported single-line tldr command block",
            );
            self.snippet.clear();
            return;
        }

        let (command_line, command) = self.snippet.pop().expect("one snippet line was checked");
        if command.contains('`') {
            self.error(
                Some(command_line),
                "backtick-in-snippet",
                "snippet",
                AdapterDisposition::Unsupported,
                "a command containing a backtick cannot be wrapped in the supported tldr command block",
            );
            return;
        }
        if command
            .chars()
            .any(|character| character.is_control() && character != '\t')
        {
            self.error(
                Some(command_line),
                "control-character",
                "snippet",
                AdapterDisposition::Unsupported,
                "snippet contains a control character that cannot be emitted safely",
            );
            return;
        }

        let (command, mapped, invalid) = map_placeholders(&command);
        if mapped > 0 {
            self.info(
                Some(command_line),
                "placeholder-mapped",
                "snippet.placeholder",
                AdapterDisposition::Mapped,
                format!(
                    "mapped {mapped} navi `<name>` placeholder(s) to tldr `{{{{name}}}}` syntax"
                ),
            );
        }
        for placeholder in invalid {
            self.warning(
                Some(command_line),
                "invalid-placeholder-reported",
                "snippet.placeholder",
                AdapterDisposition::ReportedOnly,
                format!(
                    "left `<{placeholder}>` unchanged because navi documents only alphanumeric and underscore variable names"
                ),
            );
        }

        self.examples.push(Example {
            description,
            command,
        });
    }

    fn render_page(&self, topic: &str) -> Option<String> {
        if self.examples.is_empty() {
            return None;
        }

        let mut output = format!(
            "# {}\n\n> Converted from a navi cheatsheet for display only; MyHelp never executes saved commands.\n",
            title_from_topic(topic)
        );
        for example in &self.examples {
            output.push_str("\n- ");
            output.push_str(
                example
                    .description
                    .strip_suffix(':')
                    .unwrap_or(&example.description),
            );
            output.push_str(":\n\n`");
            output.push_str(&example.command);
            output.push_str("`\n");
        }
        Some(output)
    }

    fn info(
        &mut self,
        line: Option<usize>,
        code: &str,
        source_field: &str,
        disposition: AdapterDisposition,
        message: impl Into<String>,
    ) {
        self.diagnostic(
            AdapterDiagnosticLevel::Info,
            line,
            code,
            source_field,
            disposition,
            message,
        );
    }

    fn warning(
        &mut self,
        line: Option<usize>,
        code: &str,
        source_field: &str,
        disposition: AdapterDisposition,
        message: impl Into<String>,
    ) {
        self.diagnostic(
            AdapterDiagnosticLevel::Warning,
            line,
            code,
            source_field,
            disposition,
            message,
        );
    }

    fn error(
        &mut self,
        line: Option<usize>,
        code: &str,
        source_field: &str,
        disposition: AdapterDisposition,
        message: impl Into<String>,
    ) {
        self.diagnostic(
            AdapterDiagnosticLevel::Error,
            line,
            code,
            source_field,
            disposition,
            message,
        );
    }

    fn diagnostic(
        &mut self,
        level: AdapterDiagnosticLevel,
        line: Option<usize>,
        code: &str,
        source_field: &str,
        disposition: AdapterDisposition,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(AdapterDiagnostic {
            level,
            line,
            code: code.to_owned(),
            source_field: source_field.to_owned(),
            disposition,
            message: message.into(),
        });
    }
}

fn value_after_prefix(
    line_number: usize,
    line: &str,
    prefix: char,
    state: &mut ParseState,
) -> String {
    let canonical_prefix = format!("{prefix} ");
    if !line.starts_with(&canonical_prefix) {
        state.warning(
            Some(line_number),
            "noncanonical-prefix-spacing",
            "syntax",
            AdapterDisposition::ReportedOnly,
            format!("expected `{prefix} ` according to the documented navi syntax"),
        );
    }
    line.strip_prefix(prefix).unwrap_or(line).trim().to_owned()
}

fn map_placeholders(command: &str) -> (String, usize, Vec<String>) {
    let mut output = String::with_capacity(command.len());
    let mut mapped = 0;
    let mut invalid = Vec::new();
    let mut rest = command;

    while let Some(open) = rest.find('<') {
        output.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('>') else {
            output.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let name = &after_open[..close];
        if !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            output.push_str("{{");
            output.push_str(name);
            output.push_str("}}");
            mapped += 1;
        } else {
            output.push('<');
            output.push_str(name);
            output.push('>');
            if !invalid.iter().any(|existing| existing == name) {
                invalid.push(name.to_owned());
            }
        }
        rest = &after_open[close + 1..];
    }
    output.push_str(rest);
    (output, mapped, invalid)
}

fn topic_from_source(source: &Path) -> Result<String> {
    let topic = navi_source_stem(source)?.to_lowercase();
    validate_topic(&topic)?;
    Ok(topic)
}

fn validate_source_filename(source: &Path) -> Result<()> {
    navi_source_stem(source).map(|_| ())
}

fn navi_source_stem(source: &Path) -> Result<&str> {
    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::InvalidTopic(source.display().to_string()))?;
    filename
        .strip_suffix(".cheat.md")
        .or_else(|| filename.strip_suffix(".cheat"))
        .filter(|topic| !topic.is_empty())
        .ok_or_else(|| {
            Error::InvalidImportMetadata(
                "navi source filename must end in `.cheat` or `.cheat.md`".to_owned(),
            )
        })
}

fn title_from_topic(topic: &str) -> String {
    let leaf = topic.rsplit('/').next().unwrap_or(topic);
    let mut title = String::with_capacity(leaf.len());
    let mut capitalize = true;
    for character in leaf.chars() {
        if character == '-' || character.is_control() {
            title.push(' ');
            capitalize = true;
        } else if capitalize && character.is_ascii_alphabetic() {
            title.push(character.to_ascii_uppercase());
            capitalize = false;
        } else {
            title.push(character);
            capitalize = false;
        }
    }
    title
}

#[cfg(test)]
mod tests {
    use super::{NaviAdapter, map_placeholders};
    use crate::{AdapterDiagnosticLevel, ForeignAdapter};
    use std::path::Path;

    #[test]
    fn placeholders_map_only_documented_navi_names() {
        assert_eq!(
            map_placeholders("git checkout <branch_2> && cat <not-valid>"),
            (
                "git checkout {{branch_2}} && cat <not-valid>".to_owned(),
                1,
                vec!["not-valid".to_owned()]
            )
        );
    }

    #[test]
    fn supported_fixture_generates_a_safe_lossy_preview() {
        let source = include_str!("../../tests/fixtures/adapters/navi/supported.cheat");
        let report = NaviAdapter
            .inspect(Path::new("supported.cheat"), source, "git-branch")
            .expect("inspect fixture");

        assert!(report.dry_run);
        assert!(report.convertible);
        assert!(!report.lossless);
        assert_eq!(report.source_tags, ["git", "workflow"]);
        assert_eq!(
            report.generated_page.as_deref(),
            Some(
                "# Git Branch\n\n> Converted from a navi cheatsheet for display only; MyHelp never executes saved commands.\n\n- Change branch:\n\n`git checkout {{branch}}`\n"
            )
        );
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "dynamic-variable-source-omitted"
                && diagnostic.level == AdapterDiagnosticLevel::Warning
        }));
    }

    #[test]
    fn unsupported_fixture_reports_every_material_loss() {
        let source = include_str!("../../tests/fixtures/adapters/navi/unsupported.cheat");
        let report = NaviAdapter
            .inspect(Path::new("unsupported.cheat"), source, "deploy")
            .expect("inspect fixture");

        assert!(!report.convertible);
        for code in [
            "context-extension-unsupported",
            "multiline-snippet-unsupported",
            "multiple-contexts",
        ] {
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == code),
                "missing {code}"
            );
        }
    }
}
