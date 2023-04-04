//! Provides structures to parse Cargo JSON data

use serde::Deserialize;
use std::borrow::Cow;

/// Message outputted by Cargo
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "reason", content = "message", rename_all = "kebab-case")]
pub(crate) enum CargoMessage<'c> {
	/// Message outputted by rustc
	#[serde(borrow)]
	CompilerMessage(Diagnostic<'c>),
}

/// rustc's diagnostic message
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Diagnostic<'c> {
	/// Primary message
	pub(crate) message: &'c str,
	/// Severity of the diagnostic
	pub(crate) level: DiagnosticLevel,
	/// Locations in the source code of this diagnostic
	#[serde(borrow)]
	pub(crate) spans: Vec<DiagnosticSpan<'c>>,
	/// Diagnostic as rendered by rustc
	#[serde(borrow)]
	pub(crate) rendered: Option<Cow<'c, str>>,
}

/// Severity of a [`Diagnostic`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DiagnosticLevel {
	/// A fatal error that prevents compilation
	Error,
	/// A possible error of concern
	Warning,
	/// Additional information or context about the diagnostic
	Note,
	/// A suggestion on how to resolve the diagnostic
	Help,
	/// A note attached to the message for further information
	FailureNote,
	/// Indicates a bug within the compiler
	#[serde(rename = "error: internal compiler error")]
	InternalCompilerError,
}

/// The location of a diagnostic in the source code
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct DiagnosticSpan<'c> {
	/// The file where the span is located
	///
	/// This path may not exist or may point to the source of an external crate.
	pub(crate) file_name: &'c str,
	/// The first line number of the span (1-based, inclusive)
	pub(crate) line_start: usize,
	/// The last line number of the span (1-based, inclusive)
	pub(crate) line_end: usize,
	/// The first column number of the span (1-based, inclusive)
	pub(crate) column_start: usize,
	/// The last column number of the span (1-based, exclusive)
	pub(crate) column_end: usize,
	/// This span is the "primary" span
	pub(crate) is_primary: bool,
}
