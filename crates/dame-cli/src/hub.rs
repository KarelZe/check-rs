//! The Hub protocol (version 2), as used by Scan 3.1 and the Hub GUI.
//!
//! Hub is a line-based protocol over standard input and output, similar to
//! UCI in chess. Every line is a command followed by `name=value` arguments;
//! values containing spaces or `=` are double-quoted, and unknown commands and
//! arguments are ignored. A search is set up with `pos` and `level` and
//! started with `go`; the engine reports `info` lines and finally
//! `done move=... ponder=...`. While searching, only `stop`, `ping` and
//! `ponder-hit` are accepted.
//!
//! Specification: <https://github.com/rhalbersma/scan/blob/master/protocol.txt>.

use std::fmt::Write as _;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use dame::hub::{Line, quote};
use dame::search::is_decisive;
use dame::{Game, Limits, OpeningBook, Position, SearchResult, Searcher};

/// Stack size of the search thread; the recursion keeps move lists on the stack.
pub(crate) const SEARCH_STACK_SIZE: usize = 64 << 20;

/// Where engine output goes; shared with the search thread.
pub(crate) type Output = Arc<Mutex<dyn Write + Send>>;

/// The search currently running on the worker thread.
#[derive(Debug)]
struct Running {
    handle: JoinHandle<Searcher>,
    stop: Arc<AtomicBool>,
    /// The clock limits to apply on `ponder-hit`, if pondering.
    ponder_limits: Option<Limits>,
}

/// The engine side of a Hub session.
pub(crate) struct Hub {
    output: Output,
    game: Game,
    level: Limits,
    hash_mb: usize,
    book_file: String,
    book: Option<Arc<OpeningBook>>,
    searcher: Option<Searcher>,
    running: Option<Running>,
}

impl Hub {
    pub(crate) fn new(output: Output) -> Self {
        Self {
            output,
            game: Game::new(),
            level: Limits::default(),
            hash_mb: 64,
            book_file: String::new(),
            book: None,
            searcher: None,
            running: None,
        }
    }

    /// Processes commands from `input` until `quit` or end of input, then
    /// stops any running search. End of input means the GUI is gone.
    pub(crate) fn run(&mut self, input: impl BufRead) {
        self.process(input);
        self.stop_search();
    }

    fn process(&mut self, input: impl BufRead) {
        for line in input.lines() {
            let Ok(line) = line else { break };
            let Some(line) = Line::parse(&line) else {
                continue;
            };
            if !self.handle(&line) {
                break;
            }
        }
    }

    fn send(&self, line: &str) {
        send(&self.output, line);
    }

    fn error(&self, message: &str) {
        self.send(&format!("error message={}", quote(message)));
    }

    /// Handles one command; returns `false` on `quit`.
    fn handle(&mut self, line: &Line) -> bool {
        // Commands accepted while searching.
        match line.command.as_str() {
            "ping" => {
                self.send("pong");
                return true;
            }
            "stop" => {
                self.stop_search();
                return true;
            }
            "ponder-hit" => {
                self.ponder_hit();
                return true;
            }
            "quit" => return false,
            _ => {}
        }
        // Anything else first waits for a search that is finishing.
        self.stop_search();
        match line.command.as_str() {
            "hub" => {
                self.send(&format!(
                    "id name=dame version={} author={}",
                    dame::VERSION,
                    quote("Markus Bilz")
                ));
                self.send(&format!(
                    "param name=hash value={} type=int min=1 max=65536",
                    self.hash_mb
                ));
                self.send(&format!(
                    "param name=book-file value={} type=string",
                    quote(&self.book_file)
                ));
                self.send("wait");
            }
            "set-param" => match (line.get("name"), line.get("value")) {
                (Some("hash"), Some(value)) => match value.parse::<usize>() {
                    Ok(mb) if mb > 0 => self.hash_mb = mb,
                    _ => self.error(&format!("invalid hash size {value}")),
                },
                (Some("book-file"), Some(value)) => value.clone_into(&mut self.book_file),
                _ => {}
            },
            "init" => {
                self.init();
                self.send("ready");
            }
            "new-game" => {
                if let Some(searcher) = &mut self.searcher {
                    searcher.clear();
                }
            }
            "pos" => self.set_position(line),
            "level" => self.set_level(line),
            "go" => self.go(line),
            _ => {}
        }
        true
    }

    fn init(&mut self) {
        self.book = None;
        if !self.book_file.is_empty() {
            match std::fs::read_to_string(&self.book_file)
                .map_err(|err| err.to_string())
                .and_then(|text| OpeningBook::from_text(&text).map_err(|err| err.to_string()))
            {
                Ok(book) => self.book = Some(Arc::new(book)),
                Err(err) => self.error(&format!("cannot load book {}: {err}", self.book_file)),
            }
        }
        let mut searcher = Searcher::new(dame::Classical::default(), self.hash_mb);
        searcher.set_book(self.book.clone());
        self.searcher = Some(searcher);
    }

    fn set_position(&mut self, line: &Line) {
        let position = match line.get("pos") {
            None | Some("start") => Ok(Position::initial()),
            Some(pos) => Position::from_hub(pos).or_else(|_| Position::from_fen(pos)),
        };
        let mut game = match position {
            Ok(position) => Game::from_position(position),
            Err(err) => return self.error(&err.to_string()),
        };
        for mv in line.get("moves").unwrap_or_default().split_whitespace() {
            if let Err(err) = game.play_str(mv) {
                return self.error(&err.to_string());
            }
        }
        self.game = game;
    }

    fn set_level(&mut self, line: &Line) {
        let seconds = |name| {
            line.get(name)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|s| s.is_finite() && *s >= 0.0)
                .map(Duration::from_secs_f64)
        };
        let number = |name| line.get(name).and_then(|v| v.parse::<u64>().ok());
        self.level = if line.has("infinite") {
            Limits::default()
        } else {
            Limits {
                depth: number("depth").map(|d| d.min(u64::from(u32::MAX)) as u32),
                nodes: number("nodes"),
                move_time: seconds("move-time"),
                time: seconds("time"),
                increment: seconds("inc").unwrap_or_default(),
                moves_to_go: number("moves").map(|m| m.min(u64::from(u32::MAX)) as u32),
                stop: None,
            }
        };
    }

    fn go(&mut self, line: &Line) {
        if self.searcher.is_none() {
            self.init();
        }
        let mut searcher = self.searcher.take().expect("initialized above");
        let stop = Arc::new(AtomicBool::new(false));
        let ponder = line.has("ponder");
        let analyze = line.has("analyze");

        let mut limits = self.level.clone();
        limits.stop = Some(Arc::clone(&stop));
        let ponder_limits = ponder.then(|| {
            // The clock is not running while pondering: search without time
            // limits until `ponder-hit` starts the clock.
            let clock = limits.clone();
            limits.move_time = None;
            limits.time = None;
            clock
        });
        searcher.set_book(if analyze { None } else { self.book.clone() });

        let game = self.game.clone();
        let output = Arc::clone(&self.output);
        let handle = thread::Builder::new()
            .name("search".into())
            .stack_size(SEARCH_STACK_SIZE)
            .spawn(move || {
                let result = searcher.search(&game, &limits, |info| {
                    let seconds = info.elapsed.as_secs_f64();
                    let pv: Vec<String> = info.pv.iter().map(|mv| mv.to_hub()).collect();
                    send(
                        &output,
                        &format!(
                            "info depth={} score={:.2} nodes={} time={seconds:.3} nps={:.2} pv={}",
                            info.depth,
                            hub_score(info.score),
                            info.nodes,
                            mega_nps(info.nodes, seconds),
                            quote(&pv.join(" ")),
                        ),
                    );
                });
                send(&output, &done_line(&result));
                searcher
            })
            .expect("failed to spawn the search thread");
        self.running = Some(Running {
            handle,
            stop,
            ponder_limits,
        });
    }

    /// Starts the clock of a pondering search: it now stops after the time
    /// budget of the stored level.
    fn ponder_hit(&mut self) {
        let Some(running) = &mut self.running else {
            return;
        };
        let Some(limits) = running.ponder_limits.take() else {
            return;
        };
        if let (Some(budget), _) = limits.time_budgets() {
            let stop = Arc::clone(&running.stop);
            thread::spawn(move || {
                thread::sleep(budget);
                stop.store(true, Ordering::Relaxed);
            });
        }
    }

    /// Stops the running search, if any, and waits for its `done` line.
    fn stop_search(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop.store(true, Ordering::Relaxed);
            match running.handle.join() {
                Ok(searcher) => self.searcher = Some(searcher),
                Err(_) => self.error("the search thread panicked"),
            }
        }
    }
}

fn send(output: &Output, line: &str) {
    let mut output = output
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // A closed pipe means the GUI is gone; there is nobody left to tell.
    let _ = writeln!(output, "{line}").and_then(|()| output.flush());
}

/// Scores are reported in men; wins are capped so GUIs show them as large.
#[allow(clippy::cast_precision_loss)]
fn hub_score(score: i32) -> f64 {
    if is_decisive(score) {
        f64::from(score.signum() * 100)
    } else {
        f64::from(score) / 100.0
    }
}

#[allow(clippy::cast_precision_loss)]
fn mega_nps(nodes: u64, seconds: f64) -> f64 {
    if seconds > 0.0 {
        nodes as f64 / seconds / 1e6
    } else {
        0.0
    }
}

fn done_line(result: &SearchResult) -> String {
    let mut line = String::from("done");
    for (name, mv) in [("move", result.best_move), ("ponder", result.ponder)] {
        if let Some(mv) = mv {
            write!(line, " {name}={}", mv.to_hub()).expect("writing to a String cannot fail");
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs a scripted session, lets the last search finish and returns the
    /// engine output.
    fn session(script: &str) -> String {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let output: Output = buffer.clone();
        let mut hub = Hub::new(output);
        hub.process(script.as_bytes());
        if let Some(running) = hub.running.take() {
            hub.searcher = Some(running.handle.join().unwrap());
        }
        let bytes = buffer.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn handshake() {
        let out = session("hub\nset-param name=hash value=8\ninit\nping\nquit\n");
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("id name=dame version="));
        assert!(lines.contains(&"wait"));
        assert_eq!(&lines[lines.len() - 2..], ["ready", "pong"]);
    }

    #[test]
    fn think_reports_and_plays_a_legal_move() {
        let start = Position::initial().hub();
        let out = session(&format!(
            "hub\ninit\npos pos={start}\nlevel depth=4\ngo think\n"
        ));
        assert!(out.contains("info depth=4 "), "{out}");
        let done = out.lines().find(|l| l.starts_with("done")).unwrap();
        let mv = Line::parse(done).unwrap().get("move").unwrap().to_owned();
        assert!(Position::initial().parse_move(&mv).is_ok(), "{done}");
    }

    #[test]
    fn moves_are_applied_to_the_position() {
        let start = Position::initial().hub();
        let out = session(&format!(
            "init\npos pos={start} moves=\"32-28 19-23\"\nlevel depth=2\ngo think\n"
        ));
        // White must recapture: 28x19.
        assert!(out.contains("done move=28x19x23"), "{out}");
    }

    #[test]
    fn stop_ends_an_infinite_search() {
        let start = Position::initial().hub();
        let out = session(&format!(
            "init\npos pos={start}\nlevel infinite\ngo analyze\nstop\n"
        ));
        assert!(out.lines().any(|l| l.starts_with("done move=")), "{out}");
    }

    #[test]
    fn errors_are_reported() {
        let out = session("init\npos pos=Wxyz\npos pos=start moves=\"32-29\"\n");
        assert_eq!(out.matches("error message=").count(), 2, "{out}");
    }
}
