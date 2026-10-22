//! Reading games in Portable Draughts Notation (PDN).
//!
//! PDN is the exchange format of draughts game collections, modelled on
//! chess's PGN. A game is a list of tag pairs followed by its moves:
//!
//! ```text
//! [Event "Sample"]
//! [White "Sijbrands"]
//! [Black "Gantwarg"]
//! [Result "1-1"]
//!
//! 1. 32-28 18-23 2. 37-32 {a comment} 12-18 (2... 17-21) 3. 33-29 1-1
//! ```
//!
//! The reader keeps the tags and the main line. Move numbers, comments,
//! side lines (variations), numeric annotations (`$1`) and move marks (`!`,
//! `?`) are skipped. A result (`2-0`, `0-2`, `1-1`, `0-0`, `*`) or the tags
//! of the next game end a game. The moves are returned as written; matching
//! them to legal moves is up to the caller, e.g. [`Game::play_str`].
//!
//! [`Game::play_str`]: crate::Game::play_str

/// One game read from PDN text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PdnGame {
    /// The tag pairs in order, e.g. `("Event", "Sample")`.
    pub tags: Vec<(String, String)>,
    /// The main-line moves as written, e.g. `32-28`, each with the 1-based
    /// line of the text it is on.
    pub moves: Vec<(String, usize)>,
}

impl PdnGame {
    /// The value of the first tag called `name`.
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.moves.is_empty()
    }
}

/// Game results; they end a game.
const RESULTS: [&str; 5] = ["2-0", "0-2", "1-1", "0-0", "*"];

/// Reads all games in `text`. The reader is lenient: anything it cannot
/// make sense of is either skipped or returned as a move, which then fails
/// to match a legal move.
pub fn read_games(text: &str) -> Vec<PdnGame> {
    let mut games = Vec::new();
    let mut game = PdnGame::default();
    let mut line = 1;
    // Nesting depth of side lines; only depth 0 is the main line.
    let mut depth = 0_usize;
    let mut chars = text.chars().peekable();
    let finish = |game: &mut PdnGame, games: &mut Vec<PdnGame>| {
        let game = std::mem::take(game);
        if !game.is_empty() {
            games.push(game);
        }
    };

    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '{' => {
                for c in chars.by_ref() {
                    match c {
                        '\n' => line += 1,
                        '}' => break,
                        _ => {}
                    }
                }
            }
            ';' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '[' => {
                let mut tag = String::new();
                let mut quoted = false;
                for c in chars.by_ref() {
                    match c {
                        '"' => quoted = !quoted,
                        ']' if !quoted => break,
                        '\n' => line += 1,
                        _ => {}
                    }
                    tag.push(c);
                }
                if !game.moves.is_empty() {
                    finish(&mut game, &mut games);
                }
                let (name, value) = tag.split_once(char::is_whitespace).unwrap_or((&tag, ""));
                game.tags
                    .push((name.to_owned(), value.trim().trim_matches('"').to_owned()));
            }
            c => {
                let mut token = String::from(c);
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || "{}()[];".contains(c) {
                        break;
                    }
                    token.push(c);
                    chars.next();
                }
                if depth > 0 {
                    continue;
                }
                if RESULTS.contains(&token.as_str()) {
                    finish(&mut game, &mut games);
                    continue;
                }
                if token.starts_with('$') {
                    continue;
                }
                // A move number, possibly glued to the move: `1.`, `12...`,
                // `1.32-28`.
                let digits =
                    token.len() - token.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                let mv = if token[digits..].starts_with('.') {
                    token[digits..].trim_start_matches('.')
                } else {
                    &token
                };
                let mv = mv.trim_end_matches(['!', '?']);
                if !mv.is_empty() {
                    game.moves.push((mv.to_owned(), line));
                }
            }
        }
    }
    finish(&mut game, &mut games);
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moves(game: &PdnGame) -> Vec<&str> {
        game.moves.iter().map(|(mv, _)| mv.as_str()).collect()
    }

    #[test]
    fn reads_tags_and_the_main_line() {
        let games = read_games(
            "[Event \"Test [1]\"]\n[Result \"2-0\"]\n\n\
             1. 32-28 18-23 {a {comment}\n2. 37-32 12-18 (2... 17-21 (3. 41-37)) \
             3.33-29! 7-12?! $1 4. 28x19 2-0\n",
        );
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].tag("Event"), Some("Test [1]"));
        assert_eq!(games[0].tag("Result"), Some("2-0"));
        assert_eq!(
            moves(&games[0]),
            ["32-28", "18-23", "37-32", "12-18", "33-29", "7-12", "28x19"]
        );
        let lines: Vec<usize> = games[0].moves.iter().map(|&(_, line)| line).collect();
        assert_eq!(lines, [4, 4, 5, 5, 5, 5, 5]);
    }

    #[test]
    fn results_and_tags_separate_games() {
        let games = read_games("32-28 *\n33-28 1-1 [Event \"x\"] 34-29\n[Event \"y\"]\n31-26");
        let all: Vec<Vec<&str>> = games.iter().map(moves).collect();
        assert_eq!(
            all,
            [vec!["32-28"], vec!["33-28"], vec!["34-29"], vec!["31-26"]]
        );
        assert_eq!(games[2].tag("Event"), Some("x"));
        assert_eq!(games[3].tag("Event"), Some("y"));
    }

    #[test]
    fn black_to_move_numbers_and_empty_text() {
        let games = read_games("1... 19-23 2. 32-28");
        assert_eq!(moves(&games[0]), ["19-23", "32-28"]);
        assert_eq!(read_games(" \n{only a comment}\n"), []);
    }
}
