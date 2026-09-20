//! Runner of the shell command of an intent.
//!
//! The runner starts one command through the shell, streams every output
//! line while the command runs, and stops the command when it takes
//! longer than the limit. The command of an intent comes from the intent
//! configuration of the user, so the daemon runs it as it is written.

use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use alice_core::dto::OutputStreamDto;

/// The shell the daemon runs the commands with.
const SHELL: &str = "sh";

/// The argument that hands the shell one command.
const SHELL_FLAG: &str = "-c";

/// The number of output lines that may wait for the reader.
const LINE_BUFFER: usize = 64;

/// Failure of one command run.
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// The daemon could not start the command.
    #[error("the command did not start: {0}")]
    StartFailed(String),
    /// The command ran longer than the limit.
    #[error("the command ran longer than {0:?}")]
    Timeout(Duration),
}

/// What one command did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutcome {
    /// The exit code of the command.
    pub exit_code: i32,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// The time the command ran, in milliseconds.
    pub duration_ms: u64,
}

/// Runner of the shell commands of the intents.
#[derive(Clone, Debug)]
pub struct CommandRunner {
    timeout: Duration,
    max_output_bytes: usize,
}

impl CommandRunner {
    /// Create a new runner.
    pub fn new(timeout: Duration, max_output_bytes: usize) -> Self {
        Self {
            timeout,
            max_output_bytes,
        }
    }

    /// Run one command and stream every output line to `on_line`.
    ///
    /// The runner kills the command when the timeout expires: the child
    /// drops with the run future, and a dropped child is killed.
    pub async fn run<F>(
        &self,
        command: &str,
        mut on_line: F,
    ) -> Result<CommandOutcome, ExecutionError>
    where
        F: FnMut(OutputStreamDto, &str) + Send,
    {
        // 1. Start the command.
        let started = Instant::now();
        let mut child = Command::new(SHELL)
            .arg(SHELL_FLAG)
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| ExecutionError::StartFailed(err.to_string()))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // 2. Read both streams on their own task and collect the lines.
        let (line_tx, mut line_rx) = mpsc::channel(LINE_BUFFER);
        if let Some(pipe) = stdout {
            tokio::spawn(read_lines(pipe, OutputStreamDto::Stdout, line_tx.clone()));
        }
        if let Some(pipe) = stderr {
            tokio::spawn(read_lines(pipe, OutputStreamDto::Stderr, line_tx.clone()));
        }
        drop(line_tx);

        let mut collected = Collected::default();
        let run = async {
            while let Some(line) = line_rx.recv().await {
                collected.push(line.stream, &line.text);
                on_line(line.stream, &line.text);
            }
            child
                .wait()
                .await
                .map_err(|err| ExecutionError::StartFailed(err.to_string()))
        };

        // 3. Stop the command when it takes too long.
        let status: ExitStatus = match tokio::time::timeout(self.timeout, run).await {
            Ok(result) => result?,
            Err(_) => return Err(ExecutionError::Timeout(self.timeout)),
        };

        // 4. Return what the command did.
        Ok(collected.into_outcome(
            status.code().unwrap_or(-1),
            started.elapsed(),
            self.max_output_bytes,
        ))
    }
}

/// One line the command wrote.
struct OutputLine {
    /// The stream the line belongs to.
    stream: OutputStreamDto,
    /// The text of the line.
    text: String,
}

/// The output of one command.
#[derive(Default)]
struct Collected {
    /// The standard output.
    stdout: String,
    /// The standard error.
    stderr: String,
}

impl Collected {
    /// Add one output line to the stream it belongs to.
    fn push(&mut self, stream: OutputStreamDto, line: &str) {
        let target = match stream {
            OutputStreamDto::Stdout => &mut self.stdout,
            OutputStreamDto::Stderr => &mut self.stderr,
        };
        if !target.is_empty() {
            target.push('\n');
        }
        target.push_str(line);
    }

    /// Build the outcome and keep the output inside the size limit.
    fn into_outcome(self, exit_code: i32, elapsed: Duration, max_bytes: usize) -> CommandOutcome {
        CommandOutcome {
            exit_code,
            stdout: cut(self.stdout, max_bytes),
            stderr: cut(self.stderr, max_bytes),
            duration_ms: elapsed.as_millis() as u64,
        }
    }
}

/// Read one output stream and send every line to the collector.
async fn read_lines<P>(pipe: P, stream: OutputStreamDto, line_tx: mpsc::Sender<OutputLine>)
where
    P: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(pipe).lines();
    while let Ok(Some(text)) = lines.next_line().await {
        if line_tx.send(OutputLine { stream, text }).await.is_err() {
            break;
        }
    }
}

/// Cut a text to the given number of bytes and mark the cut.
fn cut(text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut kept = text;
    kept.truncate(end);
    format!("{kept}\n… the output is longer than {max} bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collected_separates_the_two_streams() {
        let mut collected = Collected::default();
        collected.push(OutputStreamDto::Stdout, "one");
        collected.push(OutputStreamDto::Stdout, "two");
        collected.push(OutputStreamDto::Stderr, "bad");

        assert_eq!(collected.stdout, "one\ntwo");
        assert_eq!(collected.stderr, "bad");
    }

    #[test]
    fn cut_keeps_a_short_text_as_it_is() {
        assert_eq!(cut("hello".to_string(), 10), "hello");
    }

    #[test]
    fn cut_marks_a_text_it_cut() {
        let cut_text = cut("a".repeat(40), 10);
        assert!(cut_text.starts_with(&"a".repeat(10)));
        assert!(cut_text.contains("longer than 10 bytes"));
    }

    #[test]
    fn cut_keeps_a_character_boundary() {
        let cut_text = cut("é".repeat(10), 5);
        assert!(cut_text.starts_with('é'));
    }

    #[tokio::test]
    async fn run_returns_the_output_and_the_exit_code() {
        let runner = CommandRunner::new(Duration::from_secs(5), 4096);
        let mut seen: Vec<String> = Vec::new();

        let outcome = runner
            .run("echo hello", |_, line| seen.push(line.to_string()))
            .await
            .expect("the command runs");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "hello");
        assert_eq!(seen, vec!["hello"]);
    }

    #[tokio::test]
    async fn run_reports_the_exit_code_of_a_failing_command() {
        let runner = CommandRunner::new(Duration::from_secs(5), 4096);
        let outcome = runner
            .run("exit 3", |_, _| {})
            .await
            .expect("the command runs");

        assert_eq!(outcome.exit_code, 3);
    }

    #[tokio::test]
    async fn run_streams_the_error_stream() {
        let runner = CommandRunner::new(Duration::from_secs(5), 4096);
        let mut streams: Vec<OutputStreamDto> = Vec::new();

        let outcome = runner
            .run("echo bad 1>&2", |stream, _| streams.push(stream))
            .await
            .expect("the command runs");

        assert_eq!(outcome.stderr, "bad");
        assert_eq!(streams, vec![OutputStreamDto::Stderr]);
    }

    #[tokio::test]
    async fn run_stops_a_command_that_takes_too_long() {
        let runner = CommandRunner::new(Duration::from_millis(200), 4096);
        let outcome = runner.run("sleep 5", |_, _| {}).await;

        assert!(matches!(outcome, Err(ExecutionError::Timeout(_))));
    }
}
