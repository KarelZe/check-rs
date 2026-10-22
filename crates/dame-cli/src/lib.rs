//! Command-line front end for the dame draughts engine.
//!
//! The `dame` binary is a thin wrapper around [`main`]; the Python package
//! calls the same function for its `dame` console script.

#![allow(clippy::print_stdout)]

mod hub;
mod play;
mod style;

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use dame::{Color, Game, Info, Limits, OpeningBook, Position, Searcher, divide, perft};

/// The environment variable that names the opening book for `hub` and `play`.
const BOOK_VAR: &str = "DAME_BOOK";
/// The environment variable that sets how many plies of each book game count.
const BOOK_PLIES_VAR: &str = "DAME_BOOK_PLIES";

/// A fast international draughts engine.
#[derive(Debug, Parser)]
#[command(
    version,
    about,
    after_help = "Environment:\n  \
        DAME_BOOK        opening book (PDN games) for the hub and play commands\n  \
        DAME_BOOK_PLIES  plies of each book game the engine follows [default: 20]\n  \
        NO_COLOR         turn colored output off\n  \
        FORCE_COLOR      color output even when it is not a terminal"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Speak the Hub protocol on standard input and output (for GUIs).
    Hub,
    /// Play a game against the engine in the terminal.
    Play {
        /// Your color.
        #[arg(long, value_enum, default_value_t = Side::White)]
        color: Side,
        /// Engine thinking time per move, in seconds.
        #[arg(long, default_value_t = 2.0)]
        time: f64,
        /// Maximum engine search depth.
        #[arg(long)]
        depth: Option<u32>,
        /// Start position in FEN notation.
        #[arg(long)]
        fen: Option<String>,
        /// Transposition table size in MiB.
        #[arg(long, default_value_t = 64)]
        hash: usize,
    },
    /// Count the leaf nodes of the move tree (move generator test).
    Perft {
        /// Depth in plies.
        depth: u32,
        /// Start position in FEN notation.
        #[arg(long)]
        fen: Option<String>,
        /// Print the count for each root move.
        #[arg(long)]
        divide: bool,
    },
    /// Search a position and print each iteration.
    Search {
        /// Start position in FEN notation.
        #[arg(long)]
        fen: Option<String>,
        /// Search time in seconds.
        #[arg(long, default_value_t = 5.0)]
        time: f64,
        /// Maximum search depth.
        #[arg(long)]
        depth: Option<u32>,
        /// Transposition table size in MiB.
        #[arg(long, default_value_t = 64)]
        hash: usize,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Side {
    White,
    Black,
}

fn position(fen: Option<&str>) -> Result<Position, String> {
    fen.map_or_else(
        || Ok(Position::initial()),
        |fen| Position::from_fen(fen).map_err(|e| e.to_string()),
    )
}

fn limits(time: f64, depth: Option<u32>) -> Result<Limits, String> {
    let time = Duration::try_from_secs_f64(time).map_err(|e| format!("invalid time: {e}"))?;
    Ok(Limits {
        depth,
        move_time: Some(time),
        ..Limits::default()
    })
}

/// The opening book named by `DAME_BOOK`; empty if unset.
fn book_file() -> String {
    env::var(BOOK_VAR).unwrap_or_default()
}

/// The book depth set by `DAME_BOOK_PLIES`, or the default if unset.
fn book_plies() -> Result<usize, String> {
    let value = env::var(BOOK_PLIES_VAR).unwrap_or_default();
    if value.is_empty() {
        return Ok(OpeningBook::DEFAULT_PLIES);
    }
    value
        .parse()
        .ok()
        .filter(|plies| (1..=hub::MAX_BOOK_PLIES).contains(plies))
        .ok_or_else(|| {
            format!(
                "{BOOK_PLIES_VAR} must be a number of plies from 1 to {}, not `{value}`",
                hub::MAX_BOOK_PLIES
            )
        })
}

/// One line of search progress, with the labels dimmed and the score
/// colored.
fn search_info(info: &Info, paint: style::Painter) -> String {
    let pv: Vec<String> = info.pv.iter().map(ToString::to_string).collect();
    let label = |name| paint.paint(style::DIM, name);
    format!(
        "{} {:>2}  {} {:>2}  {} {}  {} {:>11}  {} {:>7.3}s  {} {}",
        label("depth"),
        info.depth,
        label("seldepth"),
        info.seldepth,
        label("score"),
        paint.score(info.score, 7),
        label("nodes"),
        info.nodes,
        label("time"),
        info.elapsed.as_secs_f64(),
        label("pv"),
        pv.join(" ")
    )
}

#[allow(clippy::cast_precision_loss)]
fn run(cli: Cli) -> Result<(), String> {
    let paint = style::Painter::stdout();
    match cli.command {
        Command::Hub => {
            let output: hub::Output = Arc::new(Mutex::new(io::stdout()));
            hub::Hub::new(output, book_file(), book_plies()?).run(io::stdin().lock());
        }
        Command::Play {
            color,
            time,
            depth,
            fen,
            hash,
        } => {
            let human = match color {
                Side::White => Color::White,
                Side::Black => Color::Black,
            };
            let game = Game::from_position(position(fen.as_deref())?);
            let book_file = book_file();
            let book = if book_file.is_empty() {
                None
            } else {
                Some(Arc::new(hub::load_book(&book_file, book_plies()?)?))
            };
            play::play(game, human, &limits(time, depth)?, hash, book)
                .map_err(|e| e.to_string())?;
        }
        Command::Perft {
            depth,
            fen,
            divide: split,
        } => {
            let pos = position(fen.as_deref())?;
            let start = Instant::now();
            let nodes = if split {
                let moves = divide(&pos, depth);
                for (mv, count) in &moves {
                    let hub = format!("{:<12}", mv.to_hub());
                    println!("{} {count}", paint.paint(style::MOVE, hub));
                }
                moves.iter().map(|(_, count)| count).sum()
            } else {
                perft(&pos, depth)
            };
            let seconds = start.elapsed().as_secs_f64();
            println!(
                "{} {} {}",
                paint.paint(style::DIM, format!("perft({depth}) =")),
                paint.paint(style::BOLD, nodes),
                paint.paint(
                    style::DIM,
                    format!(
                        "in {seconds:.3}s ({:.1} Mnps)",
                        nodes as f64 / seconds.max(1e-9) / 1e6
                    )
                ),
            );
        }
        Command::Search {
            fen,
            time,
            depth,
            hash,
        } => {
            let game = Game::from_position(position(fen.as_deref())?);
            let mut searcher = Searcher::new(dame::Classical::default(), hash);
            let result = searcher.search(&game, &limits(time, depth)?, |info| {
                println!("{}", search_info(info, paint));
            });
            match result.best_move {
                Some(mv) => println!(
                    "{} {} {}",
                    paint.paint(style::BOLD, "bestmove"),
                    paint.paint(style::MOVE, mv),
                    paint.paint(
                        style::DIM,
                        format!(
                            "({:.2} Mnps)",
                            result.nodes as f64 / result.elapsed.as_secs_f64().max(1e-9) / 1e6
                        )
                    ),
                ),
                None => println!("the game is over"),
            }
        }
    }
    Ok(())
}

/// Runs the command line `args`, the program name first, and returns the
/// process exit code: 0 on success, 1 on an error and 2 on a usage error.
pub fn main<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    // Help and usage errors follow the same color settings as the rest of
    // the output.
    let color = if style::Painter::stdout().is_colored() {
        ColorChoice::Always
    } else {
        ColorChoice::Never
    };
    let cli = Cli::command()
        .color(color)
        .try_get_matches_from(args)
        .and_then(|matches| Cli::from_arg_matches(&matches));
    let code = match cli {
        Ok(cli) => match run(cli) {
            Ok(()) => 0,
            Err(err) => {
                let paint = style::Painter::stderr();
                #[allow(clippy::print_stderr)]
                {
                    eprintln!("{} {err}", paint.paint(style::ERROR, "error:"));
                }
                1
            }
        },
        // Help and version requests land here too, with exit code 0.
        Err(err) => {
            let _ = err.print();
            u8::try_from(err.exit_code()).unwrap_or(1)
        }
    };
    // Inside Python no runtime flushes standard output at exit.
    let _ = io::stdout().flush();
    code
}
