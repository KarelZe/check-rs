//! The controller: game state, move input and the engine session.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

use dame::hub::quote;
use dame::{Bitboard, Color, Game, Move, OpeningBook, Position, Square, Termination};
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{NSString, NSUserDefaults};

use super::board_view::{BoardView, Scene};
use super::status_bar::{
    BOOK_SYMBOL, DEPTH_SYMBOL, FORWARD_SYMBOL, Item, SCORE_SYMBOL, Status, StatusBar, TIME_SYMBOL,
    Tone,
};
use super::wake_app;
use crate::engine::{Engine, Event, Info, engine_command};
use crate::input::MoveInput;
use crate::line::{Line, Preview};

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

/// Moves of the expected line shown in the status bar; the tooltip has all.
const SHOWN_PLIES: usize = 6;

/// Seconds the engine looks for the expected line to fast forward along.
const ANALYSIS_SECONDS: u32 = 1;

/// The user default that remembers the opening book's path.
const BOOK_KEY: &str = "BookFile";
/// The user default that remembers the book depth.
const BOOK_PLIES_KEY: &str = "BookPlies";

/// Book depths offered in the menu, in plies.
pub(crate) const BOOK_PLIES: [usize; 6] = [8, 12, 16, 20, 30, 40];

/// What a search is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Purpose {
    /// The engine plays the move it finds.
    Move,
    /// Only the expected line is wanted, to fast forward along it; the
    /// engine's move is not played.
    Analysis,
}

/// A running search.
#[derive(Copy, Clone, Debug)]
struct Search {
    /// The position searched.
    position: Position,
    purpose: Purpose,
}

/// The engine process and what it is doing.
#[derive(Debug)]
struct Session {
    engine: Engine,
    name: String,
    ready: bool,
    thinking: Option<Search>,
    /// Searches that were stopped and whose `done` must be ignored.
    stale: u32,
    info: Option<Info>,
    /// The expected line of the latest search, from the position searched.
    line: Option<Line>,
    /// Events from this engine process only, so a replaced engine's last
    /// words cannot reach its successor.
    events: Receiver<Event>,
}

impl Session {
    /// Starts the engine; the error is ready for the status bar.
    fn start() -> Result<Self, Item> {
        let (tx, events) = mpsc::channel();
        let wake = move |event| {
            // The receiver goes away when the engine is replaced or the app
            // quits; the event is moot then.
            let _ = tx.send(event);
            DispatchQueue::main().exec_async(wake_app);
        };
        match Engine::spawn(engine_command(), wake) {
            Ok(engine) => Ok(Self {
                engine,
                name: "engine".into(),
                ready: false,
                thinking: None,
                stale: 0,
                info: None,
                line: None,
                events,
            }),
            Err(err) => Err(danger(format!(
                "Cannot start the engine ({err}): build `dame` or set DAME_ENGINE"
            ))),
        }
    }
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
    /// The opening book file the engine plays from, if any.
    book: Option<String>,
    /// Plies of each book game the engine follows.
    book_plies: usize,
    session: Option<Session>,
    /// Shown in the status bar until the next change.
    message: Option<Item>,
    /// The expected line played through on the board, instead of the game.
    preview: Option<Preview>,
}

impl App {
    pub(crate) fn new(board: Retained<BoardView>, status: Retained<StatusBar>) -> Self {
        let (session, message) = match Session::start() {
            Ok(session) => (Some(session), None),
            Err(message) => (None, Some(message)),
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
            book: NSUserDefaults::standardUserDefaults()
                .stringForKey(&NSString::from_str(BOOK_KEY))
                .map(|path| path.to_string()),
            book_plies: usize::try_from(
                NSUserDefaults::standardUserDefaults()
                    .integerForKey(&NSString::from_str(BOOK_PLIES_KEY)),
            )
            .ok()
            .filter(|&plies| plies > 0)
            .unwrap_or(OpeningBook::DEFAULT_PLIES),
            session,
            message,
            preview: None,
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

    pub(crate) fn book(&self) -> Option<&str> {
        self.book.as_deref()
    }

    pub(crate) fn book_plies(&self) -> usize {
        self.book_plies
    }

    pub(crate) fn can_take_back(&self) -> bool {
        !self.game.moves().is_empty()
    }

    /// Whether a person may move now. An analysis does not hold them up.
    fn human_to_move(&self) -> bool {
        !self.game.is_over()
            && !self.players.includes(self.game.position().side_to_move())
            && self.session.as_ref().is_none_or(|s| {
                s.thinking
                    .is_none_or(|search| search.purpose == Purpose::Analysis)
            })
    }

    /// The board shows the expected line rather than the game.
    pub(crate) fn is_previewing(&self) -> bool {
        self.preview.is_some()
    }

    /// The engine's expected line from the game's position, if it has one.
    fn expected_line(&self) -> Option<Line> {
        self.session
            .as_ref()?
            .line
            .as_ref()?
            .from(self.game.position())
    }

    /// Whether the engine is free to look for the expected line.
    fn can_analyse(&self) -> bool {
        !self.game.is_over()
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.ready && s.thinking.is_none())
    }

    pub(crate) fn can_fast_forward(&self) -> bool {
        match &self.preview {
            Some(preview) => preview.can_forward(),
            None => self.expected_line().is_some() || self.can_analyse(),
        }
    }

    /// Plays one more ply of the expected line on the board. The first step
    /// starts the preview; without a line for the position, the engine
    /// looks for one first.
    pub(crate) fn fast_forward(&mut self) {
        match &mut self.preview {
            Some(preview) => preview.forward(),
            None => {
                if let Some(line) = self.expected_line() {
                    self.preview = Some(Preview::new(line));
                } else if self.can_analyse() && self.search(Purpose::Analysis) {
                    self.preview = Some(Preview::new(Line::empty(*self.game.position())));
                }
            }
        }
        self.input.clear();
        self.refresh();
    }

    /// Takes back one ply of the preview, back to the game after the first.
    pub(crate) fn step_back(&mut self) {
        if let Some(preview) = &mut self.preview
            && !preview.back()
        {
            self.preview = None;
        }
        self.refresh();
    }

    pub(crate) fn back_to_game(&mut self) {
        self.preview = None;
        self.refresh();
    }

    pub(crate) fn click(&mut self, square: Option<Square>) {
        if self.preview.is_some() || !self.human_to_move() {
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
        self.preview = None;
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
        self.preview = None;
        self.refresh();
    }

    /// Takes back the last move, and the engine's reply before it, so that
    /// a person is to move again.
    pub(crate) fn take_back(&mut self) {
        self.cancel_search();
        self.input.clear();
        self.message = None;
        self.preview = None;
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
        if let Some(search) = self.session.as_ref().and_then(|s| s.thinking)
            && search.purpose == Purpose::Move
            && !players.includes(search.position.side_to_move())
        {
            self.cancel_search();
        }
        self.input.clear();
        self.refresh();
        self.think_if_engine_to_move();
    }

    /// Restarts the engine process, e.g. after rebuilding it, and keeps the
    /// game. The engine picks up where it left off if it is to move.
    pub(crate) fn reload_engine(&mut self) {
        // Dropping the session asks the old engine to quit.
        self.session = None;
        self.preview = None;
        match Session::start() {
            Ok(session) => {
                self.session = Some(session);
                self.message = None;
            }
            Err(message) => {
                self.players = Players::Nobody;
                self.message = Some(message);
            }
        }
        self.input.clear();
        self.refresh();
    }

    /// Plays from the opening book at `path`, or from none, and remembers
    /// the choice. The engine restarts to load it and keeps the game.
    pub(crate) fn set_book(&mut self, path: Option<String>) {
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(BOOK_KEY);
        match &path {
            Some(path) => {
                let value = NSString::from_str(path);
                let value: &AnyObject = &value;
                // SAFETY: a string is a valid property-list value.
                unsafe { defaults.setObject_forKey(Some(value), &key) };
            }
            None => defaults.removeObjectForKey(&key),
        }
        self.book = path;
        self.reload_engine();
    }

    /// Follows the first `plies` moves of each book game and remembers the
    /// choice. With a book, the engine restarts to rebuild it.
    pub(crate) fn set_book_plies(&mut self, plies: usize) {
        NSUserDefaults::standardUserDefaults().setInteger_forKey(
            plies.try_into().unwrap_or(isize::MAX),
            &NSString::from_str(BOOK_PLIES_KEY),
        );
        if plies != self.book_plies {
            self.book_plies = plies;
            if self.book.is_some() {
                self.reload_engine();
            }
        }
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
        if session
            .thinking
            .is_some_and(|search| search.purpose == Purpose::Move)
        {
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
        while let Some(event) = self
            .session
            .as_ref()
            .and_then(|session| session.events.try_recv().ok())
        {
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
                // Always set the book, so that the app's choice overrides a
                // DAME_BOOK the engine may have inherited.
                let book = self.book.as_deref().unwrap_or_default();
                let _ = session
                    .engine
                    .send(&format!("set-param name=book-file value={}", quote(book)));
                let _ = session.engine.send(&format!(
                    "set-param name=book-plies value={}",
                    self.book_plies
                ));
                let _ = session.engine.send("init");
            }
            Event::Ready => {
                session.ready = true;
                self.think_if_engine_to_move();
            }
            Event::Info(info) if session.stale == 0 => {
                if let Some(search) = session.thinking {
                    session.line = Some(Line::parse(search.position, &info.pv));
                }
                session.info = Some(info);
            }
            Event::Info(_) => {}
            Event::Done { best } => {
                let search = session.thinking.take();
                if session.stale > 0 {
                    session.stale -= 1;
                    return;
                }
                if let Some(Search {
                    position,
                    purpose: Purpose::Analysis,
                }) = search
                {
                    // Nothing is played; the line is for the preview.
                    if session.line.as_ref().is_none_or(Line::is_empty) {
                        // E.g. a forced move, which needs no search.
                        let best = best.unwrap_or_default();
                        session.line = Some(Line::parse(position, &best));
                    }
                    if let Some(preview) = &mut self.preview
                        && preview.is_waiting()
                        && !session.line.clone().is_some_and(|line| preview.fill(line))
                    {
                        self.preview = None;
                    }
                    // In case the engine was given this side meanwhile.
                    self.think_if_engine_to_move();
                    return;
                }
                let mv = best.and_then(|best| self.game.position().parse_move(&best).ok());
                match mv {
                    Some(mv) => self.play(mv),
                    None => self.message = Some(danger("The engine returned no legal move")),
                }
            }
            Event::Error(message) => {
                self.message = Some(danger(format!("Engine error: {message}")));
            }
            Event::Exited => {
                let name = session.name.clone();
                self.session = None;
                self.preview = None;
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

    /// Starts a search for the side to move, to play its result.
    fn think(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        if !session.ready || self.game.is_over() {
            return;
        }
        match session.thinking.map(|search| search.purpose) {
            Some(Purpose::Move) => return,
            // The engine's own move comes first; the analysis is abandoned.
            Some(Purpose::Analysis) => {
                session.thinking = None;
                session.stale += 1;
                let _ = session.engine.send("stop");
            }
            None => {}
        }
        if self.search(Purpose::Move) {
            self.input.clear();
        }
        self.refresh();
    }

    /// Sends the game and starts a search of its position; returns whether
    /// the engine took the commands. An analysis ignores the opening book,
    /// so that it reports a line.
    fn search(&mut self, purpose: Purpose) -> bool {
        let Some(session) = &mut self.session else {
            return false;
        };
        let (seconds, go) = match purpose {
            Purpose::Move => (self.move_time, "go think"),
            Purpose::Analysis => (ANALYSIS_SECONDS, "go analyze"),
        };
        let moves: Vec<String> = self.game.moves().iter().map(|mv| mv.to_hub()).collect();
        let commands = [
            format!(
                "pos pos={} moves={}",
                self.game.start().hub(),
                quote(&moves.join(" "))
            ),
            format!("level move-time={seconds}"),
            go.to_owned(),
        ];
        let sent = commands
            .iter()
            .all(|command| session.engine.send(command).is_ok());
        if sent {
            let position = *self.game.position();
            session.thinking = Some(Search { position, purpose });
            session.info = None;
            session.line = Some(Line::empty(position));
        }
        sent
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
        let scene = Scene {
            position: *self.game.position(),
            flipped: self.flipped,
            selected: self.input.from(),
            targets: self.input.targets(&moves),
            via: self.input.via(),
            last_move: self.game.moves().last().copied(),
            trail: Vec::new(),
            captured: Bitboard::EMPTY,
            preview: self.preview.is_some(),
        };
        let scene = match self.preview.as_ref().map(|p| (p, p.played().split_last())) {
            Some((preview, Some((&last, trail)))) => Scene {
                position: preview.position(),
                last_move: Some(last),
                trail: trail.to_vec(),
                captured: preview.captured(),
                ..scene
            },
            _ => scene,
        };
        self.board.set_scene(scene);
        self.status.set_status(self.status());
    }

    /// The status bar's account of the preview: the moves shown as played,
    /// and those still ahead.
    fn preview_items(preview: &Preview) -> Vec<Item> {
        if preview.is_waiting() {
            return vec![
                Item::new("Preview: analysing…", Tone::Attention)
                    .with_symbol(FORWARD_SYMBOL)
                    .with_tooltip(
                        "The engine looks for the expected line to play through on the board; \
                         Esc returns to the game",
                    ),
            ];
        }
        let played = preview.played();
        let skipped = played.len().saturating_sub(SHOWN_PLIES);
        let shown = Line::pdn(&played[skipped..]);
        let ellipsis = if skipped > 0 { "… " } else { "" };
        let mut items = vec![
            Item::new(
                format!("Preview +{}: {ellipsis}{shown}", preview.ply()),
                Tone::Attention,
            )
            .with_symbol(FORWARD_SYMBOL)
            .with_tooltip(format!(
                "Preview, not the game: the position after {} of the {} plies the engine \
                 expects ({}). Click or press ⌘→ to go on, ⌘← to step back, Esc to return to \
                 the game",
                preview.ply(),
                preview.line().moves().len(),
                Line::pdn(preview.line().moves())
            ))
            .with_action(sel!(fastForward:)),
        ];
        let ahead = preview.ahead();
        if !ahead.is_empty() {
            items.push(
                Item::new(shorten(&Line::pdn(ahead), SHOWN_PLIES), Tone::Muted)
                    .with_tooltip(format!("Still ahead: {}", Line::pdn(ahead))),
            );
        }
        items
    }

    fn status(&self) -> Status {
        let mut status = Status::default();
        let side = self.game.position().side_to_move();
        if let Some(preview) = &self.preview {
            status.left.extend(Self::preview_items(preview));
        } else {
            status.left.push(self.outcome_item());
        }
        if let Some(message) = &self.message {
            status.left.push(message.clone());
        } else if self.input.is_ambiguous() {
            status.left.push(Item::new(
                "Several captures end there: click a piece to take",
                Tone::Attention,
            ));
        }
        if self.preview.is_none() {
            status.left.extend(self.expected_line_item());
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
            if let Some(depth) = info.depth {
                status.right.push(
                    Item::new(depth.to_string(), Tone::Muted)
                        .with_symbol(DEPTH_SYMBOL)
                        .with_tooltip("Search depth in plies"),
                );
            }
            if let Some(score) = info.score {
                // Scores are from the searching side; show them for White.
                let searched = session
                    .line
                    .as_ref()
                    .map_or(!side, |line| line.start().side_to_move());
                let score = if searched == Color::White {
                    score
                } else {
                    -score
                };
                status.right.push(
                    Item::new(format!("{score:+.2}"), Tone::Muted)
                        .with_symbol(SCORE_SYMBOL)
                        .with_tooltip("Evaluation in men, from White's point of view"),
                );
            }
        }
        if let Some(path) = &self.book {
            let name = Path::new(path)
                .file_name()
                .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());
            status.right.push(
                Item::new(self.book_plies.to_string(), Tone::Muted)
                    .with_symbol(BOOK_SYMBOL)
                    .with_tooltip(format!(
                        "Opening book {name}: the engine follows the first {} plies of each \
                         game. Click to change",
                        self.book_plies
                    ))
                    .with_action(sel!(chooseBookPlies:)),
            );
        }
        status.right.push(
            Item::new(format!("{} s", self.move_time), Tone::Muted)
                .with_symbol(TIME_SYMBOL)
                .with_tooltip("Engine thinking time per move; change it in the Game menu"),
        );
        let (dot, state) = match (session.ready, session.thinking.map(|s| s.purpose)) {
            (false, _) => (Tone::Muted, "starting"),
            (true, Some(Purpose::Move)) => (Tone::Attention, "thinking"),
            (true, Some(Purpose::Analysis)) => (Tone::Attention, "analysing"),
            (true, None) => (Tone::Success, "ready"),
        };
        status.right.push(
            Item::new(state, Tone::Muted)
                .with_dot(dot)
                .with_tooltip(format!("Engine: {}", session.name)),
        );
        status
    }

    /// The engine's expected line, which fast forwards when clicked; just
    /// the icon if the engine first has to find the line.
    fn expected_line_item(&self) -> Option<Item> {
        if let Some(line) = self.expected_line() {
            let line = Line::pdn(line.moves());
            Some(
                Item::new(shorten(&line, SHOWN_PLIES), Tone::Muted)
                    .with_symbol(FORWARD_SYMBOL)
                    .with_tooltip(format!(
                        "Expected line: {line}. Click or press ⌘→ to play it through on the \
                         board"
                    ))
                    .with_action(sel!(fastForward:)),
            )
        } else if self.can_analyse() {
            Some(
                Item::new("", Tone::Muted)
                    .with_symbol(FORWARD_SYMBOL)
                    .with_tooltip(
                        "Fast forward: click or press ⌘→ to let the engine find the expected \
                         line and play it through on the board",
                    )
                    .with_action(sel!(fastForward:)),
            )
        } else {
            None
        }
    }

    /// The game's state: whose turn it is, or how it ended.
    fn outcome_item(&self) -> Item {
        let side = self.game.position().side_to_move();
        match self.game.outcome() {
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
        }
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

/// The first `plies` moves of `line`, with an ellipsis if there are more.
fn shorten(line: &str, plies: usize) -> String {
    let mut moves = line.split_whitespace();
    let mut shown = moves.by_ref().take(plies).collect::<Vec<_>>().join(" ");
    if moves.next().is_some() {
        shown.push_str(" …");
    }
    shown
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
    fn long_lines_are_shortened() {
        assert_eq!(shorten("32-28 19-23 28x19", 3), "32-28 19-23 28x19");
        assert_eq!(shorten("32-28 19-23 28x19 14x23", 3), "32-28 19-23 28x19 …");
        assert_eq!(shorten("", 3), "");
    }
}
