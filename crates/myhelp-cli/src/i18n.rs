//! Locale selection and localized human-facing CLI messages.
//!
//! Only text meant for a person is localized: error lines on stderr and the
//! interactive picker prompt. Anything a script may read keeps its bytes in
//! every locale: page output, `--raw`, `--json`, `path`, list/search columns,
//! reports, completion scripts, and `--help` (completion scripts embed it).
//! Exit codes never depend on the locale. See `docs/localization.md`.

use crate::CliFailure;
use myhelp_core::Error as CoreError;

/// Explicit override: `en`, `ja`, or `auto` (the default).
pub const LANGUAGE_VARIABLE: &str = "MYHELP_LANG";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Locale {
    #[default]
    En,
    Ja,
}

impl Locale {
    /// Reads `MYHELP_LANG`, then the POSIX `LC_ALL`, `LC_MESSAGES`, and
    /// `LANG`, then the operating-system preference (the Windows user UI
    /// language, the macOS preferred languages, or the same POSIX variables
    /// on Linux).
    pub fn detect() -> Self {
        let variable = |name: &str| std::env::var(name).ok();
        Self::from_sources(
            variable(LANGUAGE_VARIABLE).as_deref(),
            [
                variable("LC_ALL").as_deref(),
                variable("LC_MESSAGES").as_deref(),
                variable("LANG").as_deref(),
            ],
            sys_locale::get_locale().as_deref(),
        )
    }

    /// Resolves the locale from already-read sources.
    ///
    /// An explicit value other than `auto` always decides, and an unsupported
    /// one selects English. The first non-empty POSIX variable decides next,
    /// as in `setlocale`: `C`, `POSIX`, and unsupported languages select
    /// English rather than falling through to the operating system. Only
    /// when no variable is set is the operating-system preference used.
    pub fn from_sources(
        explicit: Option<&str>,
        posix: [Option<&str>; 3],
        system: Option<&str>,
    ) -> Self {
        let explicit = explicit
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("auto"));
        if let Some(value) = explicit {
            return Self::parse(value).unwrap_or_default();
        }
        if let Some(value) = posix
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|value| !value.is_empty())
        {
            return Self::parse(value).unwrap_or_default();
        }
        system.and_then(Self::parse).unwrap_or_default()
    }

    /// Parses BCP 47 (`ja-JP`) and POSIX (`ja_JP.UTF-8@modifier`) tags by
    /// their primary language subtag.
    pub fn parse(tag: &str) -> Option<Self> {
        let primary = tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Self::En),
            "ja" => Some(Self::Ja),
            _ => None,
        }
    }

    pub fn picker_prompt(self) -> &'static str {
        match self {
            Self::En => "Page",
            Self::Ja => "ページ",
        }
    }
}

/// Formats the final stderr line for a failed command.
///
/// English keeps the historical `error: <cause chain>` text. Other locales
/// translate the failure MyHelp recognizes and append the untranslated cause
/// chain, so paths and operating-system details are never lost. An error
/// MyHelp does not recognize keeps the English chain after a translated
/// prefix.
pub fn error_line(error: &anyhow::Error, locale: Locale) -> String {
    match locale {
        Locale::En => format!("error: {error:#}"),
        Locale::Ja => match japanese_message(error) {
            Some(message) => format!("エラー: {message}（{error:#}）"),
            None => format!("エラー: {error:#}"),
        },
    }
}

fn japanese_message(error: &anyhow::Error) -> Option<String> {
    if let Some(failure) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CliFailure>())
    {
        return Some(
            match failure {
                CliFailure::EditorConfig(_) => "VISUAL/EDITOR の設定が正しくありません",
                CliFailure::EditorExited(_) => {
                    "エディターが失敗で終了しました。ページは変更していません"
                }
                CliFailure::EditorExitedWithDraft { .. } => {
                    "エディターが失敗で終了しました。ページは変更せず、下書きを保存しました"
                }
                CliFailure::InteractiveRequired => {
                    "pick には対話的な標準入力と標準エラー出力の端末が必要です"
                }
                CliFailure::NoPages => "選択できるページが保管庫にありません",
                CliFailure::Cancelled => "ページの選択をキャンセルしました",
                CliFailure::TldrValidationFailed(_) => "tldr ページの検証に失敗しました",
                CliFailure::AdapterConversionFailed(_) => {
                    "このアダプター変換はインポートに使えません"
                }
            }
            .to_owned(),
        );
    }

    let core = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CoreError>())?;
    Some(
        match core {
            CoreError::MissingDataDirectory => "OS のデータディレクトリを特定できませんでした",
            CoreError::InvalidTopic(_) => "トピック名が正しくありません",
            CoreError::AlreadyExists(_) => "ページはすでに存在します",
            CoreError::NotFound(_) => "ページが見つかりません",
            CoreError::Conflict { .. } => "読み込んだ後にページがディスク上で変更されました",
            CoreError::UnsafeSymlink(_) => {
                "保管庫内のシンボリックリンクまたはリパースポイントはたどりません"
            }
            CoreError::UnsafeFileType(_) => "保管庫のページが通常のファイルではありません",
            CoreError::PageTooLarge { .. } => "ページがサイズの上限を超えています",
            CoreError::InvalidTldr { .. } => "tldr ページの検証に失敗しました",
            CoreError::InvalidImportMetadata(_) => "インポートのメタデータが正しくありません",
            CoreError::ExportDestinationOccupied(_) => "エクスポート先がすでに使われています",
            CoreError::InputTooLarge { .. } => "入力がサイズの上限を超えています",
            CoreError::Io(_) | CoreError::WalkDir(_) => "ファイルを読み書きできませんでした",
        }
        .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::{Locale, error_line};
    use crate::CliFailure;
    use anyhow::{Context, Error};
    use myhelp_core::Error as CoreError;

    #[test]
    fn parses_bcp47_and_posix_tags() {
        assert_eq!(Locale::parse("ja"), Some(Locale::Ja));
        assert_eq!(Locale::parse("ja-JP"), Some(Locale::Ja));
        assert_eq!(Locale::parse("ja_JP.UTF-8"), Some(Locale::Ja));
        assert_eq!(Locale::parse("JA_jp.eucJP@cjknarrow"), Some(Locale::Ja));
        assert_eq!(Locale::parse("en_US.UTF-8"), Some(Locale::En));
        assert_eq!(Locale::parse("C.UTF-8"), None);
        assert_eq!(Locale::parse("fr_FR"), None);
    }

    #[test]
    fn explicit_override_wins_and_unsupported_values_select_english() {
        let posix = [None, None, Some("en_US.UTF-8")];
        assert_eq!(Locale::from_sources(Some("ja"), posix, None), Locale::Ja);
        assert_eq!(
            Locale::from_sources(Some("en"), [Some("ja_JP.UTF-8"), None, None], Some("ja-JP")),
            Locale::En
        );
        assert_eq!(
            Locale::from_sources(Some("fr"), [None, None, Some("ja_JP")], None),
            Locale::En
        );
    }

    #[test]
    fn auto_follows_posix_precedence() {
        assert_eq!(
            Locale::from_sources(
                Some("auto"),
                [Some("ja_JP.UTF-8"), Some("en_US"), Some("en_US")],
                None
            ),
            Locale::Ja
        );
        assert_eq!(
            Locale::from_sources(None, [None, Some("ja_JP.UTF-8"), Some("en_US")], None),
            Locale::Ja
        );
        assert_eq!(
            Locale::from_sources(None, [Some(""), None, Some("ja_JP.UTF-8")], None),
            Locale::Ja
        );
    }

    #[test]
    fn c_locale_and_unsupported_posix_values_do_not_fall_through() {
        assert_eq!(
            Locale::from_sources(None, [Some("C"), None, None], Some("ja-JP")),
            Locale::En
        );
        assert_eq!(
            Locale::from_sources(None, [None, None, Some("de_DE.UTF-8")], Some("ja-JP")),
            Locale::En
        );
    }

    #[test]
    fn operating_system_preference_applies_without_posix_variables() {
        assert_eq!(
            Locale::from_sources(None, [None, None, None], Some("ja-JP")),
            Locale::Ja
        );
        assert_eq!(
            Locale::from_sources(None, [None, None, None], None),
            Locale::En
        );
    }

    #[test]
    fn english_error_line_is_unchanged() {
        let error = Error::new(CoreError::NotFound("git".to_owned()));
        assert_eq!(
            error_line(&error, Locale::En),
            "error: page does not exist: git"
        );
    }

    #[test]
    fn japanese_error_line_keeps_the_original_detail() {
        let error = Error::new(CoreError::NotFound("git".to_owned()));
        assert_eq!(
            error_line(&error, Locale::Ja),
            "エラー: ページが見つかりません（page does not exist: git）"
        );

        let failure = Error::new(CliFailure::NoPages);
        assert_eq!(
            error_line(&failure, Locale::Ja),
            "エラー: 選択できるページが保管庫にありません（the vault contains no pages to select）"
        );
    }

    #[test]
    fn japanese_error_line_finds_failures_behind_context() {
        let error = Err::<(), _>(CoreError::InvalidTopic("../escape".to_owned()))
            .context("could not create the page")
            .unwrap_err();
        assert_eq!(
            error_line(&error, Locale::Ja),
            "エラー: トピック名が正しくありません（could not create the page: invalid topic: ../escape）"
        );
    }

    #[test]
    fn unrecognized_errors_keep_the_english_chain() {
        let error = anyhow::anyhow!("could not write command output");
        assert_eq!(
            error_line(&error, Locale::Ja),
            "エラー: could not write command output"
        );
    }
}
