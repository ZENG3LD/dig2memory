use super::language::LanguageSupport;
use std::path::Path;

/// File extensions handled by the Rust language support (always enabled).
const RUST_EXTENSIONS: &[&str] = &["rs"];

/// File extensions handled by the TypeScript language support (feature-gated).
#[cfg(feature = "lang-typescript")]
const TYPESCRIPT_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx"];

/// File extensions handled by the Python language support (feature-gated).
#[cfg(feature = "lang-python")]
const PYTHON_EXTENSIONS: &[&str] = &["py"];

/// File extensions handled by the Go language support (feature-gated).
#[cfg(feature = "lang-go")]
const GO_EXTENSIONS: &[&str] = &["go"];

/// Returns `true` if the given file extension is supported by any enabled language.
pub fn is_supported_extension(ext: &str) -> bool {
    if RUST_EXTENSIONS.contains(&ext) {
        return true;
    }
    #[cfg(feature = "lang-typescript")]
    if TYPESCRIPT_EXTENSIONS.contains(&ext) {
        return true;
    }
    #[cfg(feature = "lang-python")]
    if PYTHON_EXTENSIONS.contains(&ext) {
        return true;
    }
    #[cfg(feature = "lang-go")]
    if GO_EXTENSIONS.contains(&ext) {
        return true;
    }
    false
}

/// Returns all file extensions supported by enabled language features.
pub fn supported_extensions() -> Vec<&'static str> {
    let slices: &[&[&str]] = &[
        RUST_EXTENSIONS,
        #[cfg(feature = "lang-typescript")]
        TYPESCRIPT_EXTENSIONS,
        #[cfg(feature = "lang-python")]
        PYTHON_EXTENSIONS,
        #[cfg(feature = "lang-go")]
        GO_EXTENSIONS,
    ];
    slices.iter().flat_map(|s| s.iter().copied()).collect()
}

/// Create a `LanguageSupport` implementation for a file, based on its extension.
///
/// Returns `None` if the extension is unsupported or the language feature is disabled.
pub fn language_for_file(path: &str) -> Option<Box<dyn LanguageSupport>> {
    let ext = Path::new(path).extension()?.to_str()?;
    match ext {
        "rs" => Some(Box::new(
            super::languages::rust::RustLanguage::new().ok()?,
        )),
        #[cfg(feature = "lang-typescript")]
        "ts" | "tsx" | "js" | "jsx" => Some(Box::new(
            super::languages::typescript::TypeScriptLanguage::new(ext).ok()?,
        )),
        #[cfg(feature = "lang-python")]
        "py" => Some(Box::new(
            super::languages::python::PythonLanguage::new().ok()?,
        )),
        #[cfg(feature = "lang-go")]
        "go" => Some(Box::new(
            super::languages::go::GoLanguage::new().ok()?,
        )),
        _ => None,
    }
}
