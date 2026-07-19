mod navi;

pub use navi::{NaviAdapter, inspect_navi_file};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::Result;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ForeignFormat {
    Navi,
    Cheat,
    Pet,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AdapterCompatibility {
    LossyImportPreview,
    ReadOnlyIndex,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterDiagnosticLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AdapterDisposition {
    Mapped,
    ReportedOnly,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDiagnostic {
    pub level: AdapterDiagnosticLevel,
    pub line: Option<usize>,
    pub code: String,
    pub source_field: String,
    pub disposition: AdapterDisposition,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterConversionReport {
    pub format: ForeignFormat,
    pub compatibility: AdapterCompatibility,
    pub dry_run: bool,
    pub source_path: PathBuf,
    pub topic: String,
    pub convertible: bool,
    pub lossless: bool,
    pub source_tags: Vec<String>,
    pub generated_page: Option<String>,
    pub diagnostics: Vec<AdapterDiagnostic>,
}

pub trait ForeignAdapter {
    fn format(&self) -> ForeignFormat;

    fn inspect(
        &self,
        source_path: &Path,
        source: &str,
        topic: &str,
    ) -> Result<AdapterConversionReport>;
}
