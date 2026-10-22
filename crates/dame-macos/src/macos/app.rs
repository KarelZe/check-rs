//! The controller: game state, move input and the engine session.

use std::fmt::Write as _;
use std::sync::mpsc::{self, Receiver};

use dame::hub::quote;
use dame::{Color, Game, Move, Position, Square, Termination};
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

use super::board_view::{BoardView, Scene};
use super::status_bar::{DEPTH_SYMBOL, Item, Status, StatusBar, TIME_SYMBOL, Tone};
use super::wake_app;
use crate::engine::{Engine, Event, Info, engine_command};
use crate::input::MoveInput;

/// Who the engine plays for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Players {
    /// Two people share the board.
    Nobody,
    White,
    Black,
    /// The engine plays itself.
    Both,
}

impl Players {
    pub(crate) const ALL: [Self; 4] = [Self::Black, Self::White, Self::Both, Self::Nobody];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Black => "Engine Plays Black",
            Self::White => "Engine Plays White",
            Self::Both => "Engine Plays Both",
            Self::Nobody => "Two Players",
        }
    }

    fn includes(self, color: Color) -> bool {
        match self {
            Self::Nobody => false,
            Self::White => color == Color::White,
            Self::Black => color == Color::Black,
            Self::Both => true,
        }
    }
}

/// Engine thinking times offered in the menu, in seconds.
pub(crate) const MOVE_TIMES: [u32; 4] = [1, 3, 10, 30];

/// The engine process and what it is doing.
#[derive(Debug)]
struct Session {
    engine: Engine,
    name: String,
    ready: bool,
    /// The position the running search is for.
    thinking: Option<Position>,
    /// Searches that were stopped and whose `done` must be ignored.
    stale: u32,
    info: Option<Info>,
}

#[derive(Debug)]
pub(crate) struct App {
    board: Retained<BoardView>,
    status: Retained<StatusBar>,
    game: Game,
    input: MoveInput,
    flipped: bool,
    players: Players,
    move_time: u32,
    events: Receiver<Event>,
    session: Option<Session>,
    /// Shown in the status bar until the next change.
    message: Option<Item>,
}

impl App {
    pub(crate) fn new(board: Retained<BoardView>, status: Retained<StatusBar>) -> Self {
        let (tx, events) = mpsc::channel();
        let wake = move |event| {
            // The receiver only goes away when the app quits.
            let _ = tx.send(event);
            DispatchQueue::main().exec_async(wake_app);
        };
        let (session, message) = match Engine::spawn(engine_command(), wake) {
            Ok(engine) => {
                let session = Session {
                    engine,
                    name: "engine".into(),
                    ready: false,
                    thinking: None,
                    stale: 0,
                    info: None,
                };
                (Some(session), None)
            }
            Err(err) => (
                None,
                Some(danger(format!(
                    "Cannot start the engine ({err}): build `dame` or set DAME_ENGINE"
                ))),
            ),
        };
        let mut app = Self {
            board,
            status,
            game: Game::new(),
            input: MoveInput::default(),
            flipped: false,
            players: if session.is_some() {
                Players::Black
            } else {
                Players::Nobody
            },
            move_time: MOVE_TIMES[0],
            events,
            session,
            message,
        };
        app.refresh();
        app
    }

    pub(crate) fn players(&self) -> Players {
        self.players
    }

    pub(crate) fn move_time(&self) -> u32 {
        self.move_time
    }

    pub(crate) fn has_engine(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.ready)
    }

    pub(crate) fn can_take_back(&self) -> bool {
        !self.game.moves().is_empty()
    }

    /// Whether a person may move now.
    fn human_to_move(&self) -> bool {
        !self.game.is_over()
            && !self.players.includes(self.game.position().side_to_move())
            && self.session.as_ref().is_none_or(|s| s.thinking.is_none())
    }

    pub(crate) fn click(&mut self, square: Option<Square>) {
        if !self.human_to_move() {
            return;
        }
        let moves = self.game.legal_moves();
        if let Some(mv) = self.input.click(square, &moves) {
            self.play(mv);
        } else {
            self.refresh();
        }
    }

    fn play(&mut self, mv: Move) {
        self.input.clear();
        self.message = None;
        self.game
            .play(mv)
            .expect("moves come from the legal move list");
        self.refresh();
        self.think_if_engine_to_move();
    }

    pub(crate) fn new_game(&mut self) {
        self.set_game(Game::new());
        if let Some(session) = &mut self.session {
            let _ = session.engine.send("new-game");
        }
        self.think_if_engine_to_move();
    }

    fn set_game(&mut self, game: Game) {
        self.cancel_search();
        self.game = game;
        self.input.clear();
        self.message = None;
        self.refresh();
    }

    /// Takes back the last move, and the engine's reply before it, so that
    /// a person is to move again.
    pub(crate) fn take_back(&mut self) {
        self.cancel_search();
        self.input.clear();
        self.message = None;
        if self.game.undo().is_some() && self.players != Players::Both {
            while self.players.includes(self.game.position().side_to_move())
                && self.game.undo().is_some()
            {}
        }
        self.refresh();
        self.think_if_engine_to_move();
    }

    pub(crate) fn flip(&mut self) {
        self.flipped = !self.flipped;
        self.refresh();
    }

    pub(crate) fn set_players(&mut self, players: Players) {
        self.players = players;
        if let Some(searched) = self.session.as_ref().and_then(|s| s.thinking)
            && !players.includes(searched.side_to_move())
        {
            self.cancel_search();
        }
        self.input.clear();
        self.refresh();
        self.think_if_engine_to_move();
    }

    pub(crate) fn set_move_time(&mut self, seconds: u32) {
        self.move_time = seconds;
    }

    /// Stops the engine's search so it plays its best move so far, or lets
    /// it move for the side to move.
    pub(crate) fn move_now(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        if session.thinking.is_some() {
            let _ = session.engine.send("stop");
        } else {
            self.think();
        }
    }

    pub(crate) fn copy_position(&self) {
        copy(&self.game.position().fen());
    }

    /// Copies the moves played so far as numbered PDN.
    pub(crate) fn copy_moves(&self) {
        let mut text = String::new();
        let mut color = self.game.start().side_to_move();
        for (ply, mv) in self.game.moves().iter().enumerate() {
            if color == Color::White {
                write!(text, "{}. ", ply / 2 + 1).expect("writing to a String cannot fail");
            } else if ply == 0 {
                text.push_str("1... ");
            }
            write!(text, "{mv} ").expect("writing to a String cannot fail");
            color = !color;
        }
        if let Some(outcome) = self.game.outcome() {
            text.push_str(outcome.result());
        }
        copy(text.trim_end());
    }

    /// Sets up the position on the clipboard, in FEN or Hub notation.
    pub(crate) fn paste_position(&mut self) {
        let text = NSPasteboard::generalPasteboard()
            // SAFETY: the pasteboard type is an immutable framework constant.
            .stringForType(unsafe { NSPasteboardTypeString })
            .map(|s| s.to_string())
            .unwrap_or_default();
        let text = text.trim();
        match Game::from_fen(text).or_else(|_| Position::from_hub(text).map(Game::from_position)) {
            Ok(game) => {
                self.set_game(game);
                self.think_if_engine_to_move();
            }
            Err(err) => {
                self.message = Some(danger(format!("Cannot paste position: {err}")));
                self.refresh();
            }
        }
    }

    /// Handles everything the engine has sent so far.
    pub(super) fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.handle(event);
        }
        self.refresh();
    }

    fn handle(&mut self, event: Event) {
        let Some(session) = &mut self.session else {
            return;
        };
        match event {
            Event::Id { name, version } => {
                format!("{name} {version}")
                    .trim()
                    .clone_into(&mut session.name);
            }
            Event::Wait => {
                let _ = session.engine.send("init");
            }
            Event::Ready => {
                session.ready = true;
                self.think_if_engine_to_move();
            }
            Event::Info(mut info) if session.stale == 0 => {
                if let Some(searched) = session.thinking {
                    info.pv = pdn_line(searched, &info.pv);
                }
                session.info = Some(info);
            }
            Event::Info(_) => {}
            Event::Done { best } => {
                session.thinking = None;
                if session.stale > 0 {
                    session.stale -= 1;
                    return;
                }
                let mv = best.and_then(|best| self.game.position().parse_move(&best).ok());
                match mv {
                    Some(mv) => {
                        // Show the line as it continues after this move.
                        if let Some(info) = &mut session.info {
                            let (first, rest) = info.pv.split_once(' ').unwrap_or((&info.pv, ""));
                            if first == mv.to_string() {
                                info.pv = rest.to_owned();
                            }
                        }
                        self.play(mv);
                    }
                    None => self.message = Some(danger("The engine returned no legal move")),
                }
            }
            Event::Error(message) => {
                self.message = Some(danger(format!("Engine error: {message}")));
            }
            Event::Exited => {
                let name = session.name.clone();
                self.session = None;
                self.players = Players::Nobody;
                self.message = Some(danger(format!("{name} exited; two players now")));
            }
        }
    }

    fn think_if_engine_to_move(&mut self) {
        if self.players.includes(self.game.position().side_to_move()) {
            self.think();
        }
    }

    /// Starts a search for the side to move.
    fn think(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        if !session.ready || session.thinking.is_some() || self.game.is_over() {
            return;
        }
        let moves: Vec<String> = self.game.moves().iter().map(|mv| mv.to_hub()).collect();
        let commands = [
            format!(
                "pos pos={} moves={}",
                self.game.start().hub(),
                quote(&moves.join(" "))
            ),
            format!("level move-time={}", self.move_time),
            "go think".to_owned(),
        ];
        if commands
            .iter()
            .all(|command| session.engine.send(command).is_ok())
        {
            session.thinking = Some(*self.game.position());
            session.info = None;
            self.input.clear();
        }
        self.refresh();
    }

    /// Abandons the running search; its result will be ignored.
    fn cancel_search(&mut self) {
        if let Some(session) = &mut self.session
            && session.thinking.take().is_some()
        {
            session.stale += 1;
            let _ = session.engine.send("stop");
        }
    }

    /// Pushes the current state to the board view and the status line.
    fn refresh(&mut self) {
        let moves = self.game.legal_moves();
        self.board.set_scene(Scene {
            position: *self.game.position(),
            flipped: self.flipped,
            selected: self.input.from(),
            targets: self.input.targets(&moves),
            via: self.input.via(),
            last_move: self.game.moves().last().copied(),
        });
        self.status.set_status(self.status());
    }

    fn status(&self) -> Status {
        let mut status = Status::default();
        let side = self.game.position().side_to_move();
        status.left.push(match self.game.outcome() {
            Some(outcome) => {
                let reason = match outcome.termination {
                    Termination::NoMoves => "no moves left",
                    Termination::Repetition => "threefold repetition",
                    Termination::TwentyFiveMoves => "25-move rule",
                    Termination::SixteenMoves => "16-move rule",
                    Termination::FiveMoves => "5-move rule",
                };
                let result = match outcome.winner {
                    Some(color) => format!("{} wins", name(color)),
                    None => "Draw".to_owned(),
                };
                Item::new(
                    format!("{result} {} · {reason}", outcome.result()),
                    Tone::Success,
                )
            }
            None => Item::new(format!("{} to move", name(side)), Tone::Muted),
        });
        if let Some(message) = &self.message {
            status.left.push(message.clone());
        } else if self.input.is_ambiguous() {
            status.left.push(Item::new(
                "Several captures end there: click a piece to take",
                Tone::Attention,
            ));
        }

        let Some(session) = &self.session else {
            status.right.push(
                Item::new("no engine", Tone::Muted)
                    .with_dot(Tone::Danger)
                    .with_tooltip(
                        "Build `dame` next to the app or set DAME_ENGINE to a Hub engine",
                    ),
            );
            return status;
        };
        if let Some(info) = &session.info {
            if !info.pv.is_empty() {
                status.left.push(
                    Item::new(info.pv.clone(), Tone::Muted)
                        .with_tooltip(format!("Expected line: {}", info.pv)),
                );
            }
            if let Some(depth) = info.depth {
                status.right.push(
                    Item::new(depth.to_string(), Tone::Muted)
                        .with_symbol(DEPTH_SYMBOL)
                        .with_tooltip("Search depth in plies"),
                );
            }
            if let Some(score) = info.score {
                // Scores are from the searching side; show them for White.
                let searched = session.thinking.map_or(!side, |p| p.side_to_move());
                let score = if searched == Color::White {
                    score
                } else {
                    -score
                };
                status.right.push(
                    Item::new(format!("{score:+.2}"), Tone::Muted)
                        .with_tooltip("Evaluation in men, from White's point of view"),
                );
            }
        }
        status.right.push(
            Item::new(format!("{} s", self.move_time), Tone::Muted)
                .with_symbol(TIME_SYMBOL)
                .with_tooltip("Engine thinking time per move; change it in the Game menu"),
        );
        let (dot, state) = match (session.ready, session.thinking) {
            (false, _) => (Tone::Muted, "starting"),
            (true, Some(_)) => (Tone::Attention, "thinking"),
            (true, None) => (Tone::Success, "ready"),
        };
        status.right.push(
            Item::new(state, Tone::Muted)
                .with_dot(dot)
                .with_tooltip(format!("Engine: {}", session.name)),
        );
        status
    }
}

fn danger(text: impl Into<String>) -> Item {
    Item::new(text, Tone::Danger)
}

fn name(color: Color) -> &'static str {
    match color {
        Color::White => "White",
        Color::Black => "Black",
    }
}

/// Rewrites a variation from Hub notation (`28x19x23`) to short PDN
/// (`28x19`), stopping at the first move that does not parse.
fn pdn_line(mut position: Position, hub: &str) -> String {
    let mut line = Vec::new();
    for text in hub.split_whitespace() {
        let Ok(mv) = position.parse_move(text) else {
            break;
        };
        line.push(mv.to_string());
        position.play(mv);
    }
    line.join(" ")
}

fn copy(text: &str) {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    // SAFETY: the pasteboard type is an immutable framework constant.
    pasteboard.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variations_use_short_notation() {
        let position = Position::initial();
        assert_eq!(
            pdn_line(position, "32-28 19-23 28x19x23 14x23x19"),
            "32-28 19-23 28x19 14x23"
        );
        assert_eq!(pdn_line(position, "32-28 99-98 19-23"), "32-28");
    }
}
