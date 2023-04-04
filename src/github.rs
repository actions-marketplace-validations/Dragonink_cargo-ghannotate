//! Provides structures and functions to annotate GitHub Actions

use crate::cargo::{CargoMessage, DiagnosticLevel};
use serde::Serialize;
use std::{
	borrow::Cow,
	cmp::Ordering,
	fmt::{self, Display, Formatter},
	path::Path,
};

/// An annotation command
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Annotation<'s> {
	/// Kind of annotation
	pub(crate) kind: AnnotationKind,
	/// File to annotate
	file: Cow<'s, str>,
	/// Start of the lines to annotate (1-based, inclusive)
	line: usize,
	/// End of the lines to annotate (1-based)
	end_line: Option<usize>,
	/// Start of the columns to annotate (1-based, inclusive)
	col: Option<usize>,
	/// End of the lines to annotate (1-based)
	end_column: Option<usize>,
	/// Annotation title
	title: Option<Cow<'s, str>>,
	/// Annotation message
	message: Cow<'s, str>,
}
impl<'c> TryFrom<CargoMessage<'c>> for Annotation<'c> {
	type Error = &'static str;

	fn try_from(message: CargoMessage<'c>) -> Result<Self, Self::Error> {
		match message {
			CargoMessage::CompilerMessage(message) => {
				let primary_span = message
					.spans
					.iter()
					.find(|span| span.is_primary)
					.ok_or("Missing primary span")?;

				Ok(Self {
					kind: message.level.into(),
					file: Cow::Borrowed(primary_span.file_name),
					line: primary_span.line_start,
					end_line: Some(primary_span.line_end),
					col: Some(primary_span.column_start),
					end_column: Some(primary_span.column_end),
					title: message
						.rendered
						.as_ref()
						.map(|_rendered| Cow::Borrowed(message.message)),
					message: message.rendered.unwrap_or(Cow::Borrowed(message.message)),
				})
			}
		}
	}
}
impl<'s> Annotation<'s> {
	/// Clones `self` such that all strings are owned
	#[inline]
	pub(crate) fn to_owned(&self) -> Annotation<'static> {
		Annotation {
			kind: self.kind,
			file: Cow::Owned(self.file.clone().into_owned()),
			line: self.line,
			end_line: self.end_line,
			col: self.col,
			end_column: self.end_column,
			title: self
				.title
				.clone()
				.map(|title| Cow::Owned(title.into_owned())),
			message: Cow::Owned(self.message.clone().into_owned()),
		}
	}
}
impl<'s> PartialOrd for Annotation<'s> {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}
impl<'s> Ord for Annotation<'s> {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering {
		Path::new(self.file.as_ref())
			.cmp(Path::new(other.file.as_ref()))
			.then_with(|| self.line.cmp(&other.line))
			.then_with(|| self.col.cmp(&other.col))
			.then_with(|| self.kind.cmp(&other.kind).reverse())
	}
}
impl<'s> Display for Annotation<'s> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "::")?;
		self.kind.serialize(&mut *f)?;
		write!(f, " file={},line={}", self.file, self.line)?;
		if let Some(end_line) = self.end_line {
			write!(f, ",endLine={end_line}")?;
		}
		if let Some(col) = self.col {
			write!(f, ",col={col}")?;
			if let Some(end_column) = self.end_column {
				write!(f, ",endColumn={end_column}")?;
			}
		}
		if let Some(title) = &self.title {
			write!(f, ",title={title}")?;
		}
		write!(
			f,
			"::{}",
			self.message
				.trim()
				.replace('%', "%25")
				.replace('\n', "%0A")
				.replace('\r', "%0D")
		)
	}
}

#[allow(clippy::missing_docs_in_private_items)]
/// Kind of annotation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AnnotationKind {
	Notice,
	Warning,
	Error,
}
impl From<DiagnosticLevel> for AnnotationKind {
	#[inline]
	fn from(level: DiagnosticLevel) -> Self {
		match level {
			DiagnosticLevel::Error | DiagnosticLevel::InternalCompilerError => Self::Error,
			DiagnosticLevel::Warning => Self::Warning,
			_ => Self::Notice,
		}
	}
}
impl AnnotationKind {
	/// Returns the emoji associated with the annotation kind
	#[inline]
	pub(crate) const fn emoji(&self) -> &'static str {
		match self {
			Self::Notice => ":information_source:",
			Self::Warning => ":warning:",
			Self::Error => ":x:",
		}
	}
}
impl Display for AnnotationKind {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{} {self:?}", self.emoji())
	}
}
