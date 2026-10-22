//! Command-line front end for the dame draughts engine.

#![allow(clippy::print_stdout)]

mod hub;
mod play;

use std::env;
use std::io;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand, ValueEnum};
use dame::{Color, Game, Limits, OpeningBook, Position, Searcher, divide, perft};

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
        DAME_BOOK_PLIES  plies of each book game the engine follows [default: 20]"
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

#[allow(clippy::cast_precision_loss)]
fn run(cli: Cli) -> Result<(), String> {
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
                    println!("{:<12} {count}", mv.to_hub());
                }
                moves.iter().map(|(_, count)| count).sum()
            } else {
                perft(&pos, depth)
            };
            let seconds = start.elapsed().as_secs_f64();
            println!(
                "perft({depth}) = {nodes} in {seconds:.3}s ({:.1} Mnps)",
                nodes as f64 / seconds.max(1e-9) / 1e6
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
                let pv: Vec<String> = info.pv.iter().map(ToString::to_string).collect();
                println!(
                    "depth {:>2}  seldepth {:>2}  score {:+7.2}  nodes {:>11}  time {:>7.3}s  pv {}",
                    info.depth,
                    info.seldepth,
                    f64::from(info.score) / 100.0,
                    info.nodes,
                    info.elapsed.as_secs_f64(),
                    pv.join(" ")
                );
            });
            match result.best_move {
                Some(mv) => println!(
                    "bestmove {mv} ({:.2} Mnps)",
                    result.nodes as f64 / result.elapsed.as_secs_f64().max(1e-9) / 1e6
                ),
                None => println!("the game is over"),
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("error: {err}");
            }
            ExitCode::FAILURE
        }
    }
}
