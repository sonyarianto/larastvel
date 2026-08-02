//! Process helpers — Laravel's `Process` facade.

use std::io;
use std::path::Path;
use std::time::Duration;

/// The result of a finished process run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessResult {
    /// Exit code, or `None` if the process was terminated by a signal.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessResult {
    pub fn successful(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// A configurable process invocation — Laravel's `Process::run()` / builder.
pub struct ProcessBuilder {
    command: String,
    path: Option<String>,
    timeout: Option<Duration>,
    envs: Vec<(String, String)>,
}

impl ProcessBuilder {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            path: None,
            timeout: None,
            envs: Vec::new(),
        }
    }

    /// Working directory for the process — Laravel's `Process::path()`.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Kill the process after the given duration — Laravel's `Process::timeout()`.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set an environment variable — Laravel's `Process::env()`.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    fn base_command(&self) -> tokio::process::Command {
        let mut cmd = if cfg!(windows) {
            let mut c = tokio::process::Command::new("cmd");
            c.arg("/C").arg(&self.command);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.arg("-c").arg(&self.command);
            c
        };
        if let Some(p) = &self.path {
            cmd.current_dir(Path::new(p));
        }
        for (k, v) in &self.envs {
            cmd.env(k, v);
        }
        cmd
    }

    /// Run the process and wait for it to finish, capturing output.
    pub async fn run(&self) -> io::Result<ProcessResult> {
        let mut child = self
            .base_command()
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let mut stdout = child.stdout.take().expect("stdout is piped");
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let reader = tokio::task::spawn(async move {
            let mut out = Vec::new();
            let mut err = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut out).await;
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut err).await;
            (out, err)
        });

        let exit_code = match self.timeout {
            Some(t) => match tokio::time::timeout(t, child.wait()).await {
                Ok(status) => status?.code(),
                Err(_) => {
                    let _ = child.kill().await;
                    None
                }
            },
            None => child.wait().await?.code(),
        };

        let (stdout_bytes, stderr_bytes) = reader.await.unwrap_or_default();
        Ok(ProcessResult {
            exit_code,
            stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
            stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        })
    }

    /// Start the process in the background and return a handle to it.
    pub async fn background(&self) -> io::Result<tokio::process::Child> {
        self.base_command()
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    }
}

/// Run a command and wait for it — Laravel's `Process::run()`.
///
/// ```rust,ignore
/// let result = Process::run("echo hello").await?;
/// assert!(result.successful());
/// ```
pub async fn run(command: impl Into<String>) -> io::Result<ProcessResult> {
    ProcessBuilder::new(command).run().await
}

/// Run a command in the foreground, streaming its output to stdout/stderr —
/// Laravel's `Process::foreground()`.
pub async fn foreground(command: impl Into<String>) -> io::Result<ProcessResult> {
    let builder = ProcessBuilder::new(command);
    let mut child = builder
        .base_command()
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;
    let status = child.wait().await?;
    Ok(ProcessResult {
        exit_code: status.code(),
        stdout: String::new(),
        stderr: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_captures_stdout() {
        let result = run("echo hello").await.unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("hello"));
        assert!(result.successful());
    }

    #[tokio::test]
    async fn test_run_reports_nonzero_exit() {
        let result = run("exit 3").await.unwrap();
        assert_eq!(result.exit_code, Some(3));
        assert!(!result.successful());
    }

    #[tokio::test]
    async fn test_run_captures_stderr() {
        let result = run("echo oops 1>&2").await.unwrap();
        assert!(result.stderr.contains("oops"));
    }

    #[tokio::test]
    async fn test_run_with_env() {
        let result = ProcessBuilder::new("echo $GREETING")
            .env("GREETING", "hola")
            .run()
            .await
            .unwrap();
        assert!(result.stdout.contains("hola"));
    }

    #[tokio::test]
    async fn test_run_timeout_kills_process() {
        let result = ProcessBuilder::new("sleep 5")
            .timeout(Duration::from_millis(100))
            .run()
            .await
            .unwrap();
        assert_eq!(result.exit_code, None);
    }

    #[tokio::test]
    async fn test_foreground_streams_and_exits() {
        let result = foreground("echo streamed").await.unwrap();
        assert_eq!(result.exit_code, Some(0));
    }
}
