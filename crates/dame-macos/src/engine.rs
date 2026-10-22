//! The GUI side of the Hub protocol: runs an engine as a child process.
//!
//! The app talks to engines only through Hub, the protocol the `dame hub`
//! command implements, so any Hub engine can be plugged in. A reader thread
//! parses the engine's output into [`Event`]s and hands them to a callback;
//! commands go out through [`Engine::send`].

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use dame::hub::Line;

/// A message from the engine.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Event {
    /// `id`: the engine introduces itself.
    Id { name: String, version: String },
    /// `param`: the engine declares a parameter the GUI may set.
    Param { name: String },
    /// `wait`: the engine has listed its parameters and waits for `init`.
    Wait,
    /// `ready`: the engine is initialised and accepts searches.
    Ready,
    /// `info`: search progress.
    Info(Info),
    /// `done`: the search finished with this best move, if any.
    Done { best: Option<String> },
    /// `error`: the engine reports a problem.
    Error(String),
    /// The engine closed its output: it exited or crashed.
    Exited,
}

/// Search progress from an `info` line. Every field is optional in Hub.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Info {
    pub(crate) depth: Option<u32>,
    /// The score in men from the side to move's point of view.
    pub(crate) score: Option<f64>,
    pub(crate) pv: String,
}

impl Event {
    /// Parses one line of engine output; unknown commands give `None`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let line = Line::parse(text)?;
        let text = |name| line.get(name).unwrap_or_default().to_owned();
        Some(match line.command.as_str() {
            "id" => Self::Id {
                name: text("name"),
                version: text("version"),
            },
            "param" => Self::Param {
                name: line.get("name")?.to_owned(),
            },
            "wait" => Self::Wait,
            "ready" => Self::Ready,
            "info" => Self::Info(Info {
                depth: line.get("depth").and_then(|v| v.parse().ok()),
                score: line.get("score").and_then(|v| v.parse().ok()),
                pv: text("pv"),
            }),
            "done" => Self::Done {
                best: line.get("move").map(str::to_owned),
            },
            "error" => Self::Error(text("message")),
            _ => return None,
        })
    }
}

/// A running engine process.
#[derive(Debug)]
pub(crate) struct Engine {
    child: Child,
    stdin: ChildStdin,
}

impl Engine {
    /// Starts `command` with piped standard streams and opens the session
    /// with `hub`. Every event, ending with [`Event::Exited`], is passed to
    /// `on_event` on a background thread.
    pub(crate) fn spawn(
        mut command: Command,
        mut on_event: impl FnMut(Event) + Send + 'static,
    ) -> io::Result<Self> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        thread::Builder::new()
            .name("hub-reader".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Some(event) = Event::parse(&line) {
                        on_event(event);
                    }
                }
                on_event(Event::Exited);
            })?;
        let mut engine = Self { child, stdin };
        engine.send("hub")?;
        Ok(engine)
    }

    /// Sends one command line.
    pub(crate) fn send(&mut self, line: &str) -> io::Result<()> {
        writeln!(self.stdin, "{line}")?;
        self.stdin.flush()
    }
}

impl Drop for Engine {
    /// Asks the engine to quit and kills it if it has not after a moment.
    fn drop(&mut self) {
        let _ = self.send("quit");
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The command that starts the engine in Hub mode.
///
/// `DAME_ENGINE` names any Hub engine; otherwise the `dame` binary next to
/// the app's executable is used, and finally `dame` on the `PATH`.
pub(crate) fn engine_command() -> Command {
    let program = std::env::var_os("DAME_ENGINE")
        .map(PathBuf::from)
        .or_else(|| {
            let sibling = std::env::current_exe().ok()?.with_file_name("dame");
            sibling.is_file().then_some(sibling)
        })
        .unwrap_or_else(|| PathBuf::from("dame"));
    let mut command = Command::new(program);
    command.arg("hub");
    command
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn parse_events() {
        assert_eq!(
            Event::parse("id name=dame version=0.1.0 author=\"Markus Bilz\""),
            Some(Event::Id {
                name: "dame".into(),
                version: "0.1.0".into()
            })
        );
        assert_eq!(
            Event::parse("info depth=7 score=-0.25 nodes=1000 pv=\"32-28 19-23\""),
            Some(Event::Info(Info {
                depth: Some(7),
                score: Some(-0.25),
                pv: "32-28 19-23".into(),
            }))
        );
        assert_eq!(
            Event::parse("done move=28x19x23 ponder=14x23"),
            Some(Event::Done {
                best: Some("28x19x23".into())
            })
        );
        assert_eq!(Event::parse("done"), Some(Event::Done { best: None }));
        assert_eq!(
            Event::parse("error message=\"bad pos\""),
            Some(Event::Error("bad pos".into()))
        );
        assert_eq!(
            Event::parse("param name=book-file value=\"\" type=string"),
            Some(Event::Param {
                name: "book-file".into()
            })
        );
        assert_eq!(Event::parse("param value=64"), None);
        assert_eq!(Event::parse(""), None);
    }

    /// Runs a scripted fake engine through a real pipe.
    #[cfg(unix)]
    #[test]
    fn talks_to_a_child_process() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "read l; echo 'id name=fake version=1'; echo wait; \
             read l; echo ready; read l; echo \"done move=$l\"",
        ]);
        let (tx, rx) = mpsc::channel();
        let mut engine = Engine::spawn(command, move |event| {
            let _ = tx.send(event);
        })
        .unwrap();
        engine.send("init").unwrap();
        engine.send("32-28").unwrap();
        let events: Vec<Event> = rx.iter().take(5).collect();
        assert_eq!(
            events,
            [
                Event::Id {
                    name: "fake".into(),
                    version: "1".into()
                },
                Event::Wait,
                Event::Ready,
                Event::Done {
                    best: Some("32-28".into())
                },
                Event::Exited,
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn missing_engine_is_an_error() {
        let command = Command::new("/nonexistent/dame-engine");
        assert!(Engine::spawn(command, |_| {}).is_err());
    }
}
