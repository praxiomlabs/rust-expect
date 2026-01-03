//! Session builder for constructing sessions with custom configuration.
//!
//! This module provides a builder pattern for creating sessions with
//! customized configuration options.

use crate::config::{
    BufferConfig, EncodingConfig, LineEnding, LoggingConfig, SessionConfig, TimeoutConfig,
};
use std::path::PathBuf;
use std::time::Duration;

/// Builder for creating session configurations.
#[derive(Debug, Clone)]
pub struct SessionBuilder {
    config: SessionConfig,
}

impl SessionBuilder {
    /// Create a new session builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SessionConfig::default(),
        }
    }

    /// Set the command to execute.
    #[must_use]
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.config.command = command.into();
        self
    }

    /// Set the command arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Add a single argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.config.args.push(arg.into());
        self
    }

    /// Set environment variables.
    #[must_use]
    pub fn envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.config.env = envs
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        self
    }

    /// Set a single environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.env.insert(key.into(), value.into());
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.working_dir = Some(path.into());
        self
    }

    /// Set the terminal dimensions (width, height).
    #[must_use]
    pub const fn dimensions(mut self, cols: u16, rows: u16) -> Self {
        self.config.dimensions = (cols, rows);
        self
    }

    /// Set the default timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout.default = timeout;
        self
    }

    /// Set the timeout configuration.
    #[must_use]
    pub const fn timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.config.timeout = config;
        self
    }

    /// Set the buffer max size.
    #[must_use]
    pub const fn buffer_max_size(mut self, max_size: usize) -> Self {
        self.config.buffer.max_size = max_size;
        self
    }

    /// Set the buffer configuration.
    #[must_use]
    pub const fn buffer_config(mut self, config: BufferConfig) -> Self {
        self.config.buffer = config;
        self
    }

    /// Set the line ending style.
    #[must_use]
    pub const fn line_ending(mut self, line_ending: LineEnding) -> Self {
        self.config.line_ending = line_ending;
        self
    }

    /// Use Unix line endings (LF).
    #[must_use]
    pub const fn unix_line_endings(self) -> Self {
        self.line_ending(LineEnding::Lf)
    }

    /// Use Windows line endings (CRLF).
    #[must_use]
    pub const fn windows_line_endings(self) -> Self {
        self.line_ending(LineEnding::CrLf)
    }

    /// Set the encoding configuration.
    #[must_use]
    pub const fn encoding(mut self, config: EncodingConfig) -> Self {
        self.config.encoding = config;
        self
    }

    /// Set the logging configuration.
    #[must_use]
    pub fn logging(mut self, config: LoggingConfig) -> Self {
        self.config.logging = config;
        self
    }

    /// Enable logging to a file.
    #[must_use]
    pub fn log_to_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.logging.log_file = Some(path.into());
        self
    }

    /// Build the session configuration.
    #[must_use]
    pub fn build(self) -> SessionConfig {
        self.config
    }
}

impl Default for SessionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SessionBuilder> for SessionConfig {
    fn from(builder: SessionBuilder) -> Self {
        builder.build()
    }
}

/// Quick session configuration for common use cases.
pub struct QuickSession;

impl QuickSession {
    /// Create a session config for a shell command.
    #[must_use]
    pub fn shell() -> SessionConfig {
        SessionBuilder::new()
            .command(Self::default_shell())
            .build()
    }

    /// Create a session config for bash.
    #[must_use]
    pub fn bash() -> SessionConfig {
        SessionBuilder::new()
            .command("/bin/bash")
            .arg("--norc")
            .arg("--noprofile")
            .build()
    }

    /// Create a session config for a custom command.
    #[must_use]
    pub fn command(cmd: impl Into<String>) -> SessionConfig {
        SessionBuilder::new().command(cmd).build()
    }

    /// Create a session config for SSH.
    #[must_use]
    pub fn ssh(host: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("ssh")
            .arg(host)
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for SSH with user.
    #[must_use]
    pub fn ssh_user(user: &str, host: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("ssh")
            .arg(format!("{user}@{host}"))
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for telnet.
    #[must_use]
    pub fn telnet(host: &str, port: u16) -> SessionConfig {
        SessionBuilder::new()
            .command("telnet")
            .arg(host)
            .arg(port.to_string())
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for Python.
    #[must_use]
    pub fn python() -> SessionConfig {
        SessionBuilder::new()
            .command(if cfg!(windows) { "python" } else { "python3" })
            .arg("-i")
            .build()
    }

    /// Create a session config for Windows Command Prompt.
    ///
    /// This configures a cmd.exe session with Windows-style line endings.
    #[must_use]
    pub fn cmd() -> SessionConfig {
        SessionBuilder::new()
            .command("cmd.exe")
            .windows_line_endings()
            .build()
    }

    /// Create a session config for PowerShell.
    ///
    /// Works with both Windows PowerShell (powershell.exe) and
    /// PowerShell Core (pwsh.exe). Defaults to powershell.exe on Windows,
    /// pwsh on other platforms.
    #[must_use]
    pub fn powershell() -> SessionConfig {
        let command = if cfg!(windows) {
            "powershell.exe"
        } else {
            "pwsh"
        };
        SessionBuilder::new()
            .command(command)
            .arg("-NoLogo")
            .arg("-NoProfile")
            .build()
    }

    /// Create a session config for zsh.
    #[must_use]
    pub fn zsh() -> SessionConfig {
        SessionBuilder::new()
            .command("/bin/zsh")
            .arg("--no-rcs")
            .build()
    }

    /// Create a session config for fish shell.
    #[must_use]
    pub fn fish() -> SessionConfig {
        SessionBuilder::new()
            .command("fish")
            .arg("--no-config")
            .build()
    }

    /// Create a session config for a REPL.
    #[must_use]
    pub fn repl(cmd: impl Into<String>) -> SessionConfig {
        SessionBuilder::new()
            .command(cmd)
            .build()
    }

    /// Create a session config for Node.js REPL.
    #[must_use]
    pub fn node() -> SessionConfig {
        SessionBuilder::new()
            .command("node")
            .build()
    }

    /// Create a session config for Ruby IRB.
    #[must_use]
    pub fn ruby() -> SessionConfig {
        SessionBuilder::new()
            .command("irb")
            .arg("--simple-prompt")
            .build()
    }

    /// Create a session config for MySQL client.
    #[must_use]
    pub fn mysql(host: &str, user: &str, database: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("mysql")
            .arg("-h")
            .arg(host)
            .arg("-u")
            .arg(user)
            .arg(database)
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for MySQL client with password prompt.
    #[must_use]
    pub fn mysql_password(host: &str, user: &str, database: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("mysql")
            .arg("-h")
            .arg(host)
            .arg("-u")
            .arg(user)
            .arg("-p")
            .arg(database)
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for PostgreSQL client.
    #[must_use]
    pub fn psql(host: &str, user: &str, database: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("psql")
            .arg("-h")
            .arg(host)
            .arg("-U")
            .arg(user)
            .arg(database)
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for Docker exec into a container.
    #[must_use]
    pub fn docker_exec(container: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("docker")
            .arg("exec")
            .arg("-it")
            .arg(container)
            .arg("/bin/sh")
            .build()
    }

    /// Create a session config for Docker exec with a specific shell.
    #[must_use]
    pub fn docker_exec_shell(container: &str, shell: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("docker")
            .arg("exec")
            .arg("-it")
            .arg(container)
            .arg(shell)
            .build()
    }

    /// Create a session config for Docker run with interactive shell.
    #[must_use]
    pub fn docker_run(image: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("docker")
            .arg("run")
            .arg("-it")
            .arg("--rm")
            .arg(image)
            .build()
    }

    /// Create a session config for Redis CLI.
    #[must_use]
    pub fn redis_cli(host: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("redis-cli")
            .arg("-h")
            .arg(host)
            .build()
    }

    /// Create a session config for MongoDB shell.
    #[must_use]
    pub fn mongosh(uri: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("mongosh")
            .arg(uri)
            .timeout(Duration::from_secs(30))
            .build()
    }

    /// Create a session config for SQLite.
    #[must_use]
    pub fn sqlite(database: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("sqlite3")
            .arg(database)
            .build()
    }

    /// Create a session config for GDB debugger.
    #[must_use]
    pub fn gdb(program: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("gdb")
            .arg(program)
            .build()
    }

    /// Create a session config for LLDB debugger.
    #[must_use]
    pub fn lldb(program: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("lldb")
            .arg(program)
            .build()
    }

    /// Create a session config for Lua REPL.
    #[must_use]
    pub fn lua() -> SessionConfig {
        SessionBuilder::new()
            .command("lua")
            .arg("-i")
            .build()
    }

    /// Create a session config for Perl debugger.
    #[must_use]
    pub fn perl() -> SessionConfig {
        SessionBuilder::new()
            .command("perl")
            .arg("-de0")
            .build()
    }

    /// Create a session config for R REPL.
    #[must_use]
    pub fn r() -> SessionConfig {
        SessionBuilder::new()
            .command("R")
            .arg("--no-save")
            .arg("--no-restore")
            .build()
    }

    /// Create a session config for Julia REPL.
    #[must_use]
    pub fn julia() -> SessionConfig {
        SessionBuilder::new()
            .command("julia")
            .build()
    }

    /// Create a session config for Scala REPL.
    #[must_use]
    pub fn scala() -> SessionConfig {
        SessionBuilder::new()
            .command("scala")
            .build()
    }

    /// Create a session config for Elixir IEx.
    #[must_use]
    pub fn iex() -> SessionConfig {
        SessionBuilder::new()
            .command("iex")
            .build()
    }

    /// Create a session config for Clojure REPL.
    #[must_use]
    pub fn clojure() -> SessionConfig {
        SessionBuilder::new()
            .command("clj")
            .build()
    }

    /// Create a session config for Haskell GHCi.
    #[must_use]
    pub fn ghci() -> SessionConfig {
        SessionBuilder::new()
            .command("ghci")
            .build()
    }

    /// Create a session config for OCaml REPL.
    #[must_use]
    pub fn ocaml() -> SessionConfig {
        SessionBuilder::new()
            .command("ocaml")
            .build()
    }

    /// Create a session config for kubectl exec into a pod.
    #[must_use]
    pub fn kubectl_exec(pod: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("kubectl")
            .arg("exec")
            .arg("-it")
            .arg(pod)
            .arg("--")
            .arg("/bin/sh")
            .build()
    }

    /// Create a session config for kubectl exec with namespace.
    #[must_use]
    pub fn kubectl_exec_ns(namespace: &str, pod: &str, shell: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("kubectl")
            .arg("exec")
            .arg("-it")
            .arg("-n")
            .arg(namespace)
            .arg(pod)
            .arg("--")
            .arg(shell)
            .build()
    }

    /// Create a session config for screen attach.
    #[must_use]
    pub fn screen_attach(session_name: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("screen")
            .arg("-r")
            .arg(session_name)
            .build()
    }

    /// Create a session config for tmux attach.
    #[must_use]
    pub fn tmux_attach(session_name: &str) -> SessionConfig {
        SessionBuilder::new()
            .command("tmux")
            .arg("attach")
            .arg("-t")
            .arg(session_name)
            .build()
    }

    /// Get the default shell for the current platform.
    #[must_use]
    pub fn default_shell() -> String {
        std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(windows) {
                "cmd.exe".to_string()
            } else {
                "/bin/sh".to_string()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_basic() {
        let config = SessionBuilder::new()
            .command("/bin/bash")
            .arg("-c")
            .arg("echo hello")
            .build();

        assert_eq!(config.command, "/bin/bash");
        assert_eq!(config.args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn builder_env() {
        let config = SessionBuilder::new()
            .command("test")
            .env("FOO", "bar")
            .env("BAZ", "qux")
            .build();

        assert_eq!(config.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(config.env.get("BAZ"), Some(&"qux".to_string()));
    }

    #[test]
    fn builder_timeout() {
        let config = SessionBuilder::new()
            .command("test")
            .timeout(Duration::from_secs(60))
            .build();

        assert_eq!(config.timeout.default, Duration::from_secs(60));
    }

    #[test]
    fn quick_session_bash() {
        let config = QuickSession::bash();
        assert_eq!(config.command, "/bin/bash");
        assert!(config.args.contains(&"--norc".to_string()));
    }

    #[test]
    fn quick_session_ssh() {
        let config = QuickSession::ssh_user("admin", "example.com");
        assert_eq!(config.command, "ssh");
        assert!(config.args.contains(&"admin@example.com".to_string()));
    }

    #[test]
    fn quick_session_cmd() {
        let config = QuickSession::cmd();
        assert_eq!(config.command, "cmd.exe");
        assert_eq!(config.line_ending, LineEnding::CrLf);
    }

    #[test]
    fn quick_session_powershell() {
        let config = QuickSession::powershell();
        #[cfg(windows)]
        assert_eq!(config.command, "powershell.exe");
        #[cfg(not(windows))]
        assert_eq!(config.command, "pwsh");
        assert!(config.args.contains(&"-NoLogo".to_string()));
        assert!(config.args.contains(&"-NoProfile".to_string()));
    }

    #[test]
    fn quick_session_zsh() {
        let config = QuickSession::zsh();
        assert_eq!(config.command, "/bin/zsh");
        assert!(config.args.contains(&"--no-rcs".to_string()));
    }

    #[test]
    fn quick_session_fish() {
        let config = QuickSession::fish();
        assert_eq!(config.command, "fish");
        assert!(config.args.contains(&"--no-config".to_string()));
    }

    #[test]
    fn quick_session_python() {
        let config = QuickSession::python();
        #[cfg(windows)]
        assert_eq!(config.command, "python");
        #[cfg(not(windows))]
        assert_eq!(config.command, "python3");
        assert!(config.args.contains(&"-i".to_string()));
    }

    #[test]
    fn quick_session_node() {
        let config = QuickSession::node();
        assert_eq!(config.command, "node");
    }

    #[test]
    fn quick_session_ruby() {
        let config = QuickSession::ruby();
        assert_eq!(config.command, "irb");
        assert!(config.args.contains(&"--simple-prompt".to_string()));
    }

    #[test]
    fn quick_session_mysql() {
        let config = QuickSession::mysql("localhost", "root", "testdb");
        assert_eq!(config.command, "mysql");
        assert!(config.args.contains(&"-h".to_string()));
        assert!(config.args.contains(&"localhost".to_string()));
        assert!(config.args.contains(&"-u".to_string()));
        assert!(config.args.contains(&"root".to_string()));
        assert!(config.args.contains(&"testdb".to_string()));
    }

    #[test]
    fn quick_session_psql() {
        let config = QuickSession::psql("localhost", "postgres", "mydb");
        assert_eq!(config.command, "psql");
        assert!(config.args.contains(&"-h".to_string()));
        assert!(config.args.contains(&"-U".to_string()));
        assert!(config.args.contains(&"postgres".to_string()));
    }

    #[test]
    fn quick_session_docker_exec() {
        let config = QuickSession::docker_exec("my-container");
        assert_eq!(config.command, "docker");
        assert!(config.args.contains(&"exec".to_string()));
        assert!(config.args.contains(&"-it".to_string()));
        assert!(config.args.contains(&"my-container".to_string()));
        assert!(config.args.contains(&"/bin/sh".to_string()));
    }

    #[test]
    fn quick_session_docker_run() {
        let config = QuickSession::docker_run("ubuntu:latest");
        assert_eq!(config.command, "docker");
        assert!(config.args.contains(&"run".to_string()));
        assert!(config.args.contains(&"-it".to_string()));
        assert!(config.args.contains(&"--rm".to_string()));
        assert!(config.args.contains(&"ubuntu:latest".to_string()));
    }

    #[test]
    fn quick_session_redis() {
        let config = QuickSession::redis_cli("redis.example.com");
        assert_eq!(config.command, "redis-cli");
        assert!(config.args.contains(&"-h".to_string()));
        assert!(config.args.contains(&"redis.example.com".to_string()));
    }

    #[test]
    fn quick_session_sqlite() {
        let config = QuickSession::sqlite("test.db");
        assert_eq!(config.command, "sqlite3");
        assert!(config.args.contains(&"test.db".to_string()));
    }

    #[test]
    fn quick_session_gdb() {
        let config = QuickSession::gdb("./my_program");
        assert_eq!(config.command, "gdb");
        assert!(config.args.contains(&"./my_program".to_string()));
    }

    #[test]
    fn quick_session_kubectl() {
        let config = QuickSession::kubectl_exec("my-pod");
        assert_eq!(config.command, "kubectl");
        assert!(config.args.contains(&"exec".to_string()));
        assert!(config.args.contains(&"-it".to_string()));
        assert!(config.args.contains(&"my-pod".to_string()));
        assert!(config.args.contains(&"--".to_string()));
        assert!(config.args.contains(&"/bin/sh".to_string()));
    }

    #[test]
    fn quick_session_kubectl_ns() {
        let config = QuickSession::kubectl_exec_ns("production", "api-pod", "/bin/bash");
        assert_eq!(config.command, "kubectl");
        assert!(config.args.contains(&"-n".to_string()));
        assert!(config.args.contains(&"production".to_string()));
        assert!(config.args.contains(&"api-pod".to_string()));
        assert!(config.args.contains(&"/bin/bash".to_string()));
    }

    #[test]
    fn quick_session_repls() {
        // Test various REPL helpers
        assert_eq!(QuickSession::lua().command, "lua");
        assert_eq!(QuickSession::julia().command, "julia");
        assert_eq!(QuickSession::scala().command, "scala");
        assert_eq!(QuickSession::iex().command, "iex");
        assert_eq!(QuickSession::clojure().command, "clj");
        assert_eq!(QuickSession::ghci().command, "ghci");
        assert_eq!(QuickSession::ocaml().command, "ocaml");
        assert_eq!(QuickSession::r().command, "R");
    }

    #[test]
    fn quick_session_tmux_screen() {
        let config = QuickSession::tmux_attach("mysession");
        assert_eq!(config.command, "tmux");
        assert!(config.args.contains(&"attach".to_string()));
        assert!(config.args.contains(&"-t".to_string()));
        assert!(config.args.contains(&"mysession".to_string()));

        let config = QuickSession::screen_attach("myscreen");
        assert_eq!(config.command, "screen");
        assert!(config.args.contains(&"-r".to_string()));
        assert!(config.args.contains(&"myscreen".to_string()));
    }
}
