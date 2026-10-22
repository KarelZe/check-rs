//! Playing against the engine in the terminal.

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::thread;

use dame::{Color, Game, Limits, Move, OpeningBook, Position, SearchResult, Searcher, Square};

use crate::hub::SEARCH_STACK_SIZE;
use crate::style::{self, Painter};

/// Renders the board next to a key of square numbers, with the squares of
/// `last`, the last move, highlighted.
fn diagram(pos: &Position, last: Option<Move>, paint: Painter) -> String {
    let mut squares = Square::all();
    let mut text = String::new();
    for row in 0..10 {
        let (mut board, mut numbers) = (String::new(), String::new());
        for file in 0..10 {
            if (row + file) % 2 == 1 {
                let sq = squares.next().expect("50 playable squares");
                let moved = last.is_some_and(|mv| mv.from() == sq || mv.to() == sq);
                let cell = match pos.piece_at(sq) {
                    Some(piece) if moved => paint.paint(style::LAST_MOVE, piece.to_char()),
                    Some(piece) if piece.color == Color::White => {
                        paint.paint(style::WHITE, piece.to_char())
                    }
                    Some(piece) => paint.paint(style::BLACK, piece.to_char()),
                    None if moved => paint.paint(style::LAST_MOVE, '.'),
                    None => paint.paint(style::DIM, '.'),
                };
                board.push(' ');
                board += &cell;
                let number = format!("{:>3}", sq.number());
                numbers += &paint.paint(if moved { style::LAST_MOVE } else { style::DIM }, number);
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

/// The greeting: the side the person plays and the commands. Commands are
/// bold in color and quoted without.
fn intro(human: Color, paint: Painter) -> String {
    let command = |name| {
        if paint.is_colored() {
            paint.paint(style::BOLD, name)
        } else {
            format!("`{name}`")
        }
    };
    format!(
        "You play {human}. Enter moves like {} or {}; {}, {}, {}, {}.",
        paint.paint(style::MOVE, "32-28"),
        paint.paint(style::MOVE, "28x19"),
        command("undo"),
        command("hint"),
        command("moves"),
        command("quit"),
    )
}

/// The engine's move with its search statistics, or `(book)`.
fn engine_move(result: &SearchResult, mv: Move, paint: Painter) -> String {
    let details = if result.from_book {
        "(book)".to_owned()
    } else {
        format!(
            "(depth {}, score {:+.2}, {} nodes)",
            result.depth,
            f64::from(result.score) / 100.0,
            result.nodes
        )
    };
    format!(
        "{}  {}",
        paint.paint(style::MOVE, mv),
        paint.paint(style::DIM, details)
    )
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
    let paint = Painter::stdout();
    writeln!(out, "{}\n", intro(human, paint))?;

    loop {
        let pos = *game.position();
        let last = game.moves().last().copied();
        writeln!(out, "{}", diagram(&pos, last, paint))?;
        if let Some(outcome) = game.outcome() {
            let winner = outcome
                .winner
                .map_or_else(|| "draw".to_owned(), |c| format!("{c} wins"));
            let over = format!(
                "Game over: {} ({winner}, {:?})",
                outcome.result(),
                outcome.termination
            );
            writeln!(out, "{}", paint.paint(style::BOLD, over))?;
            return Ok(());
        }

        if pos.side_to_move() != human {
            write!(out, "{}", paint.paint(style::DIM, "dame is thinking... "))?;
            out.flush()?;
            let result = think(&mut searcher, &game, limits);
            let mv = result.best_move.expect("the game is not over");
            writeln!(out, "{}\n", engine_move(&result, mv, paint))?;
            game.play_unchecked(mv);
            continue;
        }

        loop {
            write!(
                out,
                "{}",
                paint.paint(style::BOLD, format!("{} to move> ", pos.side_to_move()))
            )?;
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
                        writeln!(
                            out,
                            "{} {}",
                            paint.paint(style::DIM, "hint:"),
                            paint.paint(style::MOVE, mv)
                        )?;
                    }
                }
                text => match game.play_str(text) {
                    Ok(_) => {
                        writeln!(out)?;
                        break;
                    }
                    Err(err) => writeln!(out, "{}", paint.paint(style::ERROR, err))?,
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
        let text = diagram(&Position::initial(), None, Painter::PLAIN);
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("   b   b"), "{first}");
        assert!(first.ends_with("  1     2     3     4     5"), "{first:?}");
    }

    #[test]
    fn colors_only_add_escape_codes() {
        let mut game = Game::new();
        let mv = game.play_str("32-28").unwrap();
        let (pos, last) = (*game.position(), Some(mv));
        let colored = diagram(&pos, last, Painter::COLOR);
        assert_ne!(colored, diagram(&pos, last, Painter::PLAIN));
        assert_eq!(
            style::tests::strip(&colored),
            diagram(&pos, last, Painter::PLAIN)
        );
        // The man that moved to 28 and the square it left are highlighted.
        let highlighted = Painter::COLOR.paint(style::LAST_MOVE, 'w');
        assert!(colored.contains(&highlighted), "{colored:?}");
    }
}
