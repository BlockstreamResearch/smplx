use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, ExprLit, Lit, Token};

pub struct SynFilePath {
    _span_file: String,

    root_simf_path: String,
    deps_json_path: String,
}

impl Parse for SynFilePath {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parsing the first argument ("simf/main.simf")
        let expr_root = input.parse::<Expr>()?;
        let span_file = expr_root.span().file();

        let root_simf_path = match expr_root {
            Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Ok(s.value()),
            _ => Err(syn::Error::new(expr_root.span(), "Expected string literal")),
        }?;

        input.parse::<Token![,]>()?;

        // Parsing the second argument ("dependency.json")
        let expr_deps = input.parse::<Expr>()?;

        let deps_json_path = match expr_deps {
            Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Ok(s.value()),
            _ => Err(syn::Error::new(
                expr_deps.span(),
                "Expected string for dependency.json path",
            )),
        }?;

        Ok(Self {
            _span_file: span_file,
            root_simf_path,
            deps_json_path,
        })
    }
}

impl SynFilePath {
    #[inline]
    pub fn validate_and_load_deps_json(&self) -> syn::Result<String> {
        let valid_path = Self::resolve_and_validate(&self.deps_json_path)?;

        let json_str = std::fs::read_to_string(&valid_path).map_err(|e| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Unable to load JSON string. IO Error: '{}'. Looked at path: '{}', is_file: {}",
                    e, // Actual OS error
                    valid_path.display(),
                    valid_path.is_file()
                ),
            )
        })?;

        Ok(json_str)
    }

    #[inline]
    fn validate_root_simf_path(&self) -> syn::Result<PathBuf> {
        Self::resolve_and_validate(&self.root_simf_path)
    }

    #[inline]
    fn resolve_and_validate(raw_path: &str) -> syn::Result<PathBuf> {
        let mut path = PathBuf::from_str(raw_path).unwrap();

        if !path.is_absolute() {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "CARGO_MANIFEST_DIR not set - macro must be used within a Cargo workspace",
                )
            })?;

            let mut path_local = PathBuf::from(manifest_dir);
            path_local.push(raw_path);

            path = path_local;
        }

        if is_not_a_file(&path) {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "File not found, look path: '{}', is file: '{}', canonical: '{:?}'",
                    path.display(),
                    path.is_file(),
                    path.canonicalize()
                ),
            ));
        }
        Ok(path)
    }
}

pub struct SimfContent {
    pub content: String,
    pub contract_name: String,
}

impl SimfContent {
    /// Prepares a contract name for use as a Rust module/identifier.
    ///
    /// Converts the input to a valid lowercase Rust identifier by:
    /// - Trimming whitespace
    /// - Converting to lowercase
    /// - Replacing invalid characters with underscores
    /// - Ensuring it starts with a letter or underscore (not a digit)
    /// - Validating it's not a reserved keyword
    ///
    /// # Errors
    /// Returns an `std::io::Error` if:
    /// - The contract name is empty after trimming.
    /// - The contract name is a reserved Rust keyword.
    /// - The contract name is not a valid Rust identifier.
    ///
    /// # Examples
    /// - `"MyContract"` → `"mycontract"`
    /// - `"My-Contract-V2"` → `"my_contract_v2"`
    /// - `"123Invalid"` → Error (starts with digit)
    /// - `"valid_name"` → `"valid_name"`
    pub fn prepare_contract_name(name: &str) -> std::io::Result<String> {
        let trimmed = name.trim_matches(|c: char| c.is_whitespace());
        if trimmed.is_empty() {
            return Err(std::io::Error::other("Contract name cannot be empty"));
        }

        let mut result = trimmed.to_lowercase();

        result = result
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect();

        while result.contains("__") {
            result = result.replace("__", "_");
        }

        result = result.trim_matches('_').to_string();

        if result.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            result = format!("_{result}");
        }

        if Self::is_reserved_keyword(&result) {
            return Err(std::io::Error::other(format!(
                "Contract name '{result}' is a reserved Rust keyword"
            )));
        }

        if !Self::is_valid_rust_identifier(&result) {
            return Err(std::io::Error::other(format!(
                "Contract name '{result}' is not a valid Rust identifier"
            )));
        }

        Ok(result)
    }

    /// Checks if a string is a valid Rust identifier
    #[inline]
    fn is_valid_rust_identifier(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        let first = s.chars().next().unwrap();
        // First char must be letter or underscore
        if !first.is_alphabetic() && first != '_' {
            return false;
        }

        s.chars().all(|c| c.is_alphanumeric() || c == '_')
    }

    /// Checks if a string is a Rust reserved keyword (only checks keywords, not format)
    ///
    /// This function validates against Rust's actual reserved keywords.
    /// Valid identifiers like "hello" will return false (not a keyword).#[inline]
    fn is_reserved_keyword(s: &str) -> bool {
        syn::parse_str::<syn::Ident>(s).is_err()
    }

    pub fn extract_content_from_path(path: &PathBuf) -> std::io::Result<SimfContent> {
        let contract_name = {
            let name = path
                .file_prefix()
                .ok_or(std::io::Error::other(format!(
                    "No file prefix in file: '{}'",
                    path.display()
                )))?
                .to_string_lossy();
            Self::prepare_contract_name(name.as_ref())?
        };

        let mut content = String::new();
        let mut x = File::open(path)?;
        x.read_to_string(&mut content)?;
        Ok(SimfContent { content, contract_name })
    }

    /// Evaluates the path expression and extracts Simf content.
    ///
    /// # Errors
    /// Returns a `syn::Error` if the path is invalid or the file cannot be read.
    pub fn eval_path_expr(syn_file_path: &SynFilePath) -> syn::Result<(Self, PathBuf)> {
        let path = syn_file_path.validate_root_simf_path()?;
        let res = Self::extract_content_from_path(&path).map_err(|e| syn::Error::new(Span::call_site(), e))?;
        Ok((res, path))
    }
}

#[inline]
fn is_not_a_file(path: &Path) -> bool {
    !path.is_file()
}
