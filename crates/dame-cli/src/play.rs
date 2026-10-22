//! Playing against the engine in the terminal.

use std::fmt::Write as _;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::thread;

use dame::{Color, Game, Limits, OpeningBook, Piece, Position, SearchResult, Searcher, Square};

use crate::hub::SEARCH_STACK_SIZE;

/// Renders the board next to a key of square numbers.
fn diagram(pos: &Position) -> String {
    let mut squares = Square::all();
    let mut text = String::new();
    for row in 0..10 {
        let (mut board, mut numbers) = (String::new(), String::new());
        for file in 0..10 {
            if (row + file) % 2 == 1 {
                let sq = squares.next().expect("50 playable squares");
                board.push(' ');
                board.push(pos.piece_at(sq).map_or('.', Piece::to_char));
                write!(numbers, "{:>3}", sq.number()).expect("writing to a String cannot fail");
            } else {
                board += "  ";
                numbers += "   ";
            }
        }
        text += &board;
        text += "    ";
        text += &numbers;
        text.push('\n');
    }
    text
}

/// Runs a search on a thread with a large stack.
fn think(searcher: &mut Searcher, game: &Game, limits: &Limits) -> SearchResult {
    thread::scope(|scope| {
        thread::Builder::new()
            .stack_size(SEARCH_STACK_SIZE)
            .spawn_scoped(scope, || searcher.search(game, limits, |_| {}))
            .expect("failed to spawn the search thread")
            .join()
            .expect("the search thread panicked")
    })
}

/// Plays a game between a human on standard input and the engine, which
/// opens from `book` if it has one.
pub(crate) fn play(
    mut game: Game,
    human: Color,
    limits: &Limits,
    hash_mb: usize,
    book: Option<Arc<OpeningBook>>,
) -> io::Result<()> {
    let mut searcher = Searcher::new(dame::Classical::default(), hash_mb);
    searcher.set_book(book);
    let stdin = io::stdin();
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "You play {human}. Enter moves like 32-28 or 28x19; `undo`, `hint`, `moves`, `quit`.\n"
    )?;

    loop {
        let pos = *game.position();
        writeln!(out, "{}", diagram(&pos))?;
        if let Some(outcome) = game.outcome() {
            let winner = outcome
                .winner
                .map_or_else(|| "draw".to_owned(), |c| format!("{c} wins"));
            writeln!(
                out,
                "Game over: {} ({winner}, {:?})",
                outcome.result(),
                outcome.termination
            )?;
            return Ok(());
        }

        if pos.side_to_move() != human {
            write!(out, "dame is thinking... ")?;
            out.flush()?;
            let result = think(&mut searcher, &game, limits);
            let mv = result.best_move.expect("the game is not over");
            if result.from_book {
                writeln!(out, "{mv}  (book)\n")?;
            } else {
                writeln!(
                    out,
                    "{mv}  (depth {}, score {:+.2}, {} nodes)\n",
                    result.depth,
                    f64::from(result.score) / 100.0,
                    result.nodes
                )?;
            }
            game.play_unchecked(mv);
            continue;
        }

        loop {
            write!(out, "{} to move> ", pos.side_to_move())?;
            out.flush()?;
            let mut line = String::new();
            if stdin.lock().read_line(&mut line)? == 0 {
                return Ok(());
            }
            match line.trim() {
                "" => {}
                "quit" | "q" => return Ok(()),
                "undo" | "u" => {
                    // Take back the engine's reply and the human move.
                    game.undo();
                    if game.position().side_to_move() != human {
                        game.undo();
                    }
                    break;
                }
                "moves" | "m" => {
                    let moves: Vec<String> =
                        game.legal_moves().iter().map(ToString::to_string).collect();
                    writeln!(out, "{}", moves.join(" "))?;
                }
                "hint" | "h" => {
                    let result = think(&mut searcher, &game, limits);
                    if let Some(mv) = result.best_move {
                        writeln!(out, "hint: {mv}")?;
                    }
                }
                text => match game.play_str(text) {
                    Ok(_) => {
                        writeln!(out)?;
                        break;
                    }
                    Err(err) => writeln!(out, "{err}")?,
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_has_a_square_key() {
        let text = diagram(&Position::initial());
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("   b   b"), "{first}");
        assert!(first.ends_with("  1     2     3     4     5"), "{first:?}");
    }
}
