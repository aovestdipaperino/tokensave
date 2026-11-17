//! `tokensave grep` (#688): runs a search command and adds index facts to
//! its hits.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitStatus;
use std::process::Stdio;

use tokensave::grep_annotate::classify_search;
use tokensave::grep_annotate::split_terminator;
use tokensave::grep_annotate::AnnotatePlan;
use tokensave::grep_annotate::Index;
use tokensave::grep_annotate::IndexState;
use tokensave::grep_annotate::SearchMode;
use tokensave::grep_annotate::FACTS_MARKER;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Exit code for a command that `tokensave grep` does not accept. `grep`
/// uses the same code for an error.
const USAGE_EXIT: i32 = 2;

/// Exit code when the search command cannot start, as a shell gives it.
const NOT_STARTED_EXIT: i32 = 127;

/// Exit code when the reader of the output closes it before the end, for
/// example `head -1`. On Unix this is the code of a process that `SIGPIPE`
/// stops.
#[cfg(unix)]
const CLOSED_OUTPUT_EXIT: i32 = 141;
#[cfg(not(unix))]
const CLOSED_OUTPUT_EXIT: i32 = 0;

/// Maximum number of bytes of one output line that are held in memory. A
/// longer line is copied in parts and gets no facts.
const MAX_LINE_BYTES: u64 = 1024 * 1024;

/// Runs the search command and returns the exit code for the process.
pub(crate) async fn run(
    graph: Option<&str>,
    command: &[OsString],
) -> tokensave::errors::Result<i32> {
    let with_facts = match graph {
        None => true,
        Some("none") => false,
        Some(other) => {
            eprintln!(
                "tokensave grep: --graph={other} is not available yet. \
                 This version accepts --graph=none only."
            );
            return Ok(USAGE_EXIT);
        }
    };
    let plan = match classify_search(command) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("tokensave grep: {error}");
            return Ok(USAGE_EXIT);
        }
    };
    let SearchMode::Annotate(plan) = plan.mode else {
        return run_unchanged(command).await;
    };
    if !with_facts {
        return run_unchanged(command).await;
    }
    let cwd = std::env::current_dir()?;
    // The index opens before the search starts. With no usable index the
    // command runs unchanged, also without the added `-n`.
    match Index::open(&cwd, &plan.git_dirs).await {
        IndexState::Ready(index) => run_with_facts(command, &plan, *index, &cwd).await,
        IndexState::Missing => run_unchanged(command).await,
        IndexState::Unusable(reason) => {
            // The search still runs. One line says why its hits have no facts.
            eprintln!("tokensave grep: no index facts: {reason}");
            run_unchanged(command).await
        }
    }
}

fn search_command(command: &[OsString]) -> Option<Command> {
    let (program, arguments) = command.split_first()?;
    let mut search = Command::new(program);
    search.args(arguments);
    Some(search)
}

fn not_started(command: &[OsString], error: &std::io::Error) -> i32 {
    let program = command
        .first()
        .map(|program| program.to_string_lossy().into_owned())
        .unwrap_or_default();
    eprintln!("tokensave grep: cannot run `{program}`: {error}");
    NOT_STARTED_EXIT
}

/// Returns the exit code of the search command. A command that a signal
/// stops gives 128 plus the signal number, as a shell reports it.
fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

/// Runs the command with the standard streams of this process.
async fn run_unchanged(command: &[OsString]) -> tokensave::errors::Result<i32> {
    let Some(mut search) = search_command(command) else {
        return Ok(USAGE_EXIT);
    };
    match search.status().await {
        Ok(status) => Ok(exit_code(status)),
        Err(error) => Ok(not_started(command, &error)),
    }
}

/// Runs the command and adds the index facts to each hit of its output.
async fn run_with_facts(
    command: &[OsString],
    plan: &AnnotatePlan,
    mut index: Index,
    cwd: &std::path::Path,
) -> tokensave::errors::Result<i32> {
    let mut numbered = command.to_vec();
    if let Some(position) = plan.line_number_flag_at {
        numbered.insert(position.min(numbered.len()), OsString::from("-n"));
    }
    // A search of one file prints `line:` hits with no path.
    let single_file = plan
        .single_operand
        .as_deref()
        .filter(|operand| cwd.join(operand).is_file());

    let Some(mut search) = search_command(&numbered) else {
        return Ok(USAGE_EXIT);
    };
    let mut child = match search.stdout(Stdio::piped()).kill_on_drop(true).spawn() {
        Ok(child) => child,
        Err(error) => return Ok(not_started(command, &error)),
    };
    let Some(output) = child.stdout.take() else {
        return Ok(exit_code(child.wait().await?));
    };
    let mut reader = tokio::io::BufReader::new(output);
    let mut out = std::io::BufWriter::new(std::io::stdout());
    let mut line = Vec::new();
    // True while the parts of a line longer than `MAX_LINE_BYTES` go by.
    let mut in_long_line = false;
    let mut output_closed = false;
    loop {
        line.clear();
        let read = (&mut reader)
            .take(MAX_LINE_BYTES)
            .read_until(b'\n', &mut line)
            .await?;
        if read == 0 {
            break;
        }
        let complete = line.ends_with(b"\n") || (read as u64) < MAX_LINE_BYTES;
        let (text, terminator) = split_terminator(&line);
        let facts = if complete && !in_long_line {
            index.facts_for_line(text, plan, single_file).await
        } else {
            None
        };
        in_long_line = !complete;
        if write_line(&mut out, text, facts.as_deref(), terminator).is_err() {
            output_closed = true;
            break;
        }
    }
    if !output_closed && out.flush().is_err() {
        output_closed = true;
    }
    if output_closed {
        // The reader stopped, so the search has no more use.
        let _ = child.kill().await;
        return Ok(CLOSED_OUTPUT_EXIT);
    }
    Ok(exit_code(child.wait().await?))
}

fn write_line(
    out: &mut impl Write,
    text: &[u8],
    facts: Option<&str>,
    terminator: &[u8],
) -> std::io::Result<()> {
    out.write_all(text)?;
    if let Some(facts) = facts {
        out.write_all(FACTS_MARKER.as_bytes())?;
        out.write_all(facts.as_bytes())?;
    }
    out.write_all(terminator)
}
