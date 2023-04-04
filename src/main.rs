//! Tool to annotate [GitHub Actions](https://docs.github.com/en/actions) from the output of Cargo commands
#![warn(
	// Restriction
	missing_copy_implementations,
	missing_debug_implementations,
	missing_docs,
	unreachable_pub,
	unused,
	unused_crate_dependencies,
	unused_lifetimes,
	unused_tuple_struct_fields,
	clippy::dbg_macro,
	clippy::empty_structs_with_brackets,
	clippy::enum_glob_use,
	clippy::float_cmp_const,
	clippy::format_push_string,
	clippy::match_on_vec_items,
	clippy::missing_docs_in_private_items,
	clippy::mod_module_files,
	clippy::option_option,
	clippy::rest_pat_in_fully_bound_structs,
	clippy::str_to_string,
	clippy::verbose_file_reads,
	// Suspicious
	noop_method_call,
	meta_variable_misuse,
	// Pedantic
	unused_qualifications,
	clippy::doc_link_with_quotes,
	clippy::doc_markdown,
	clippy::filter_map_next,
	clippy::float_cmp,
	clippy::inefficient_to_string,
	clippy::macro_use_imports,
	clippy::manual_let_else,
	clippy::match_wildcard_for_single_variants,
	clippy::mem_forget,
	clippy::missing_errors_doc,
	clippy::missing_panics_doc,
	clippy::needless_continue,
	clippy::semicolon_if_nothing_returned,
	clippy::unnested_or_patterns,
	clippy::unused_self,
	// Style
	unused_import_braces,
	// Nursery
	clippy::empty_line_after_outer_attr,
	clippy::imprecise_flops,
	clippy::missing_const_for_fn,
	clippy::suboptimal_flops,
)]
#![deny(
	// Correctness
	pointer_structural_match,
	// Restriction
	keyword_idents,
	non_ascii_idents,
	missing_abi,
	unsafe_op_in_unsafe_fn,
	unused_must_use,
	clippy::exit,
	clippy::lossy_float_literal,
	clippy::undocumented_unsafe_blocks,
)]
#![forbid(unsafe_code)]

use clap::{Args, Parser, Subcommand, ValueHint};
use std::{
	collections::{BTreeSet, HashMap},
	ffi::OsString,
	fmt::Write as FmtWrite,
	fs::File,
	io::{self, BufRead, Cursor, Write as IoWrite},
	process::{Command, ExitCode, Output, Stdio},
};

mod cargo;
mod github;

use cargo::{CargoMessage, Diagnostic, DiagnosticLevel};
use github::{Annotation, AnnotationKind};

fn main() -> ExitCode {
	let cli = Cli::parse_from(std::env::args_os().filter(|arg| arg != "ghannotate"));

	let annotation_threshold = if cli.allow_warnings {
		AnnotationKind::Error
	} else {
		AnnotationKind::Warning
	};
	let mut max_annotation = AnnotationKind::Notice;

	let cargo = cli.invoke_cargo().expect("Cargo invocation failed");
	let mut summaries = Vec::new();
	let mut annotations = BTreeSet::new();
	let mut stdout = io::stdout().lock();
	for line in Cursor::new(cargo.stdout).lines() {
		if let Ok(message) = serde_json::from_str::<CargoMessage>(&line.unwrap()) {
			let summary = Summary::from(&message);
			if let Ok(annotation) = Annotation::try_from(message) {
				if annotations.insert(annotation.to_owned()) {
					writeln!(stdout, "{annotation}").unwrap();
					max_annotation = max_annotation.max(annotation.kind);
					summaries.push(summary);
				}
			}
		}
	}
	write_summaries(summaries).unwrap();

	if max_annotation >= annotation_threshold {
		ExitCode::FAILURE
	} else {
		ExitCode::SUCCESS
	}
}

/// Annotates GitHub Actions from the output of Cargo subcommands
#[derive(Debug, Clone, Parser)]
#[command(author, version, about, long_about = None)]
#[command(override_usage = "cargo ghannotate check [OPTIONS] [ARGS]...\n       \
	cargo ghannotate clippy [OPTIONS] [ARGS]...\n       \
	cargo ghannotate build [OPTIONS] [ARGS]...")]
struct Cli {
	/// Path to the `cargo` executable
	#[arg(long, env = "CARGO", value_name = "PATH", value_hint = ValueHint::ExecutablePath)]
	cargo: OsString,
	/// Should warnings be raised, they would not cause the job to fail
	#[arg(long)]
	allow_warnings: bool,
	/// Cargo subcommand
	#[command(subcommand)]
	command: CliCommand,
}
impl Cli {
	/// Invokes Cargo with the passed arguments and returns its output
	#[inline]
	fn invoke_cargo(&self) -> io::Result<Output> {
		#[allow(clippy::enum_glob_use)]
		use CliCommand::*;

		Command::new(&self.cargo)
			.arg(match self.command {
				Check(_) => "check",
				Clippy(_) => "clippy",
				Build(_) => "build",
			})
			.args(self.command.as_ref().as_ref())
			.arg("--message-format=json")
			.stdin(Stdio::null())
			.stderr(Stdio::inherit())
			.output()
	}
}

/// Cargo subcommand
#[derive(Debug, Clone, Subcommand)]
enum CliCommand {
	/// Runs `cargo check` and annotates from its output
	Check(CliCommandArgs),
	/// Runs `cargo clippy` and annotates from its output
	Clippy(CliCommandArgs),
	/// Runs `cargo build` and annotates from its output
	Build(CliCommandArgs),
}
impl AsRef<CliCommandArgs> for CliCommand {
	#[inline]
	fn as_ref(&self) -> &CliCommandArgs {
		match self {
			Self::Check(args) | Self::Clippy(args) | Self::Build(args) => args,
		}
	}
}

/// Arguments to be passed down to Cargo
#[derive(Debug, Clone, Args)]
#[repr(transparent)]
struct CliCommandArgs {
	/// Arguments to be passed down to Cargo
	#[arg(
		num_args = 0..,
		trailing_var_arg = true,
		allow_hyphen_values = true,
	)]
	args: Vec<OsString>,
}
impl AsRef<[OsString]> for CliCommandArgs {
	#[inline]
	fn as_ref(&self) -> &[OsString] {
		self.args.as_ref()
	}
}

/// Summary of [`CargoMessage`]
#[derive(Debug, Clone)]
enum Summary {
	/// Summary of [`Diagnostic`]
	Diagnostic {
		/// [`Diagnostic.level`](Diagnostic#structfield.level)
		level: DiagnosticLevel,
		/// [`Diagnostic.message`](Diagnostic#structfield.message)
		message: String,
		/// Location of the diagnostic (primary [span](cargo::DiagnosticSpan))
		location: Option<(String, usize)>,
	},
}
impl<'c> From<&'c Diagnostic<'c>> for Summary {
	#[inline]
	fn from(message: &'c Diagnostic<'c>) -> Self {
		Self::Diagnostic {
			level: message.level,
			message: message.message.to_owned(),
			location: message.spans.iter().find_map(|span| {
				span.is_primary
					.then(|| (span.file_name.to_owned(), span.line_start))
			}),
		}
	}
}
impl<'c> From<&'c CargoMessage<'c>> for Summary {
	#[inline]
	fn from(message: &'c CargoMessage<'c>) -> Self {
		match message {
			CargoMessage::CompilerMessage(message) => Self::from(message),
		}
	}
}

/// Writes a summary of the job in the special summary file
fn write_summaries(summaries: Vec<Summary>) -> io::Result<()> {
	/// Environment variable containing the path to the special summary file
	const SUMMARY_PATH_VAR: &str = "GITHUB_STEP_SUMMARY";
	let Some(path) = std::env::var_os(SUMMARY_PATH_VAR)
		.or(cfg!(debug_assertions).then(|| OsString::from("SUMMARY.md")))
		else {
			return Ok(());
		};
	let mut file = File::create(path)?;

	let diagnostics = summaries
		.iter()
		.filter(|summary| matches!(summary, Summary::Diagnostic { .. }))
		.collect::<Vec<_>>();
	if !diagnostics.is_empty() {
		write_diagnostic_summary(diagnostics, &mut file)?;
	}

	Ok(())
}

/// Write a summary of the [`Diagnostic`](Summary::Diagnostic) items
fn write_diagnostic_summary<'s>(
	diagnostics: impl IntoIterator<Item = &'s Summary>,
	file: &mut File,
) -> io::Result<()> {
	writeln!(file, "# Diagnostics")?;

	let mut kind_count: HashMap<AnnotationKind, usize> = HashMap::new();
	let mut table = String::new();
	writeln!(table, "|Level|Message|Location|").unwrap();
	writeln!(table, "|:--|:--|--:|").unwrap();
	for summary in diagnostics {
		let Summary::Diagnostic { level, message, location } = summary else {
			unreachable!()
		};
		let kind = AnnotationKind::from(*level);
		*kind_count.entry(kind).or_default() += 1;
		let location = location
			.as_ref()
			.map(|location| format!("`{}:{}`", location.0, location.1))
			.unwrap_or_default();
		writeln!(table, "|{kind}|{message}|{location}|").unwrap();
	}

	writeln!(
		file,
		"> **TOTAL:** {} {}s, {} {}s, {} {}s",
		kind_count
			.get(&AnnotationKind::Error)
			.copied()
			.unwrap_or_default(),
		AnnotationKind::Error,
		kind_count
			.get(&AnnotationKind::Warning)
			.copied()
			.unwrap_or_default(),
		AnnotationKind::Warning,
		kind_count
			.get(&AnnotationKind::Notice)
			.copied()
			.unwrap_or_default(),
		AnnotationKind::Notice,
	)?;
	writeln!(file)?;
	file.write_all(table.as_bytes())
}

#[cfg(test)]
mod tests {
	use super::*;
	use clap::CommandFactory;

	#[test]
	fn cli() {
		Cli::command().debug_assert();
	}
}
