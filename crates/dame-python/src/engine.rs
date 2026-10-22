//! The `Engine` class and search results.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use dame::{Classical, Limits, Searcher};
use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;

use crate::board::Board;
use crate::types::Move;

/// Stack size of the search thread; the recursion keeps move lists on the stack.
const SEARCH_STACK_SIZE: usize = 64 << 20;

/// How often a waiting search checks for `KeyboardInterrupt`.
const SIGNAL_POLL: Duration = Duration::from_millis(50);

fn moves(moves: &[dame::Move]) -> Vec<Move> {
    moves.iter().copied().map(Move).collect()
}

/// An opening book: weighted moves by position.
#[pyclass(frozen, from_py_object, module = "dame")]
#[derive(Clone)]
pub(crate) struct OpeningBook(pub(crate) Arc<dame::OpeningBook>);

#[pymethods]
impl OpeningBook {
    /// Builds a book from the first `plies` moves of each game in PDN `text`.
    #[staticmethod]
    #[pyo3(signature = (text, *, plies = dame::OpeningBook::DEFAULT_PLIES))]
    fn from_pdn(text: &str, plies: usize) -> PyResult<Self> {
        dame::OpeningBook::from_pdn(text, plies)
            .map(|book| Self(Arc::new(book)))
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    /// Builds a book from the first `plies` moves of each game in the PDN
    /// file at `path`.
    #[staticmethod]
    #[pyo3(signature = (path, *, plies = dame::OpeningBook::DEFAULT_PLIES))]
    fn from_file(path: std::path::PathBuf, plies: usize) -> PyResult<Self> {
        let bytes = std::fs::read(&path)
            .map_err(|err| PyOSError::new_err(format!("{}: {err}", path.display())))?;
        Self::from_pdn(&String::from_utf8_lossy(&bytes), plies)
    }

    /// The book moves for the current position of `board` with their weights,
    /// heaviest first.
    fn moves(&self, board: &Board) -> Vec<(Move, u32)> {
        self.0
            .moves(board.game.position())
            .into_iter()
            .map(|entry| (Move(entry.mv), entry.weight))
            .collect()
    }

    fn __len__(&self) -> usize {
        self.0.len()
    }
}

/// Progress reported after each completed search iteration.
#[pyclass(frozen, get_all, module = "dame")]
pub(crate) struct SearchInfo {
    /// The completed depth in plies.
    depth: u32,
    /// The deepest ply reached, including captures resolved at the leaves.
    seldepth: u32,
    /// The score in centi-men from the side to move's point of view.
    score: i32,
    /// Nodes searched so far.
    nodes: u64,
    /// Seconds spent so far.
    elapsed: f64,
    /// The principal variation.
    pv: Vec<Move>,
}

#[pymethods]
impl SearchInfo {
    fn __repr__(&self) -> String {
        format!(
            "SearchInfo(depth={}, score={}, nodes={}, pv=[{}])",
            self.depth,
            self.score,
            self.nodes,
            self.pv
                .iter()
                .map(|mv| format!("'{}'", mv.0))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// The outcome of a search.
#[pyclass(frozen, get_all, module = "dame")]
pub(crate) struct SearchResult {
    /// The best move, or `None` if the game is over.
    best_move: Option<Move>,
    /// The expected reply.
    ponder: Option<Move>,
    /// The score in centi-men from the side to move's point of view.
    score: i32,
    /// The last completed depth; 0 for book moves.
    depth: u32,
    /// The number of nodes searched.
    nodes: u64,
    /// Seconds spent.
    elapsed: f64,
    /// The principal variation.
    pv: Vec<Move>,
    /// Whether the move came from the opening book.
    from_book: bool,
}

#[pymethods]
impl SearchResult {
    /// Moves until mate if the score is a forced win (positive) or loss
    /// (negative), else `None`.
    #[getter]
    fn mate(&self) -> Option<i32> {
        dame::search::mate_in(self.score)
    }

    fn __repr__(&self) -> String {
        format!(
            "SearchResult(best_move={}, score={}, depth={}, nodes={})",
            self.best_move.map_or_else(
                || "None".to_owned(),
                |mv| format!("Move('{}')", mv.0.to_hub())
            ),
            self.score,
            self.depth,
            self.nodes
        )
    }
}

/// A search engine. Reusing one engine across moves keeps its hash table.
#[pyclass(frozen, module = "dame")]
pub(crate) struct Engine {
    searcher: Mutex<Searcher<Classical>>,
    stop: Mutex<Option<Arc<AtomicBool>>>,
}

#[pymethods]
impl Engine {
    #[new]
    #[pyo3(signature = (*, hash_mb = 64, book = None))]
    fn new(hash_mb: usize, book: Option<OpeningBook>) -> Self {
        let mut searcher = Searcher::new(Classical::default(), hash_mb);
        searcher.set_book(book.map(|book| book.0));
        Self {
            searcher: Mutex::new(searcher),
            stop: Mutex::new(None),
        }
    }

    /// Searches the current position of `board`.
    ///
    /// Without limits the search runs until `stop()` is called from another
    /// thread or the process receives `KeyboardInterrupt`.
    #[pyo3(signature = (board, *, depth = None, nodes = None, time = None, info = None))]
    fn search(
        &self,
        py: Python<'_>,
        board: &Board,
        depth: Option<u32>,
        nodes: Option<u64>,
        time: Option<f64>,
        info: Option<Py<PyAny>>,
    ) -> PyResult<SearchResult> {
        let move_time = time
            .map(|t| {
                Duration::try_from_secs_f64(t)
                    .map_err(|_| PyValueError::new_err(format!("invalid time {t}")))
            })
            .transpose()?;
        if depth == Some(0) {
            return Err(PyValueError::new_err("depth must be at least 1"));
        }
        let game = board.game.clone();
        let stop = Arc::new(AtomicBool::new(false));
        *self.stop.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&stop));
        let limits = Limits {
            depth,
            nodes,
            move_time,
            stop: Some(Arc::clone(&stop)),
            ..Limits::default()
        };

        let callback_error: Mutex<Option<PyErr>> = Mutex::new(None);
        let report = |progress: &dame::Info| {
            let Some(callback) = &info else { return };
            Python::attach(|py| {
                let progress = SearchInfo {
                    depth: progress.depth,
                    seldepth: progress.seldepth,
                    score: progress.score,
                    nodes: progress.nodes,
                    elapsed: progress.elapsed.as_secs_f64(),
                    pv: moves(&progress.pv),
                };
                if let Err(err) = callback.call1(py, (progress,)) {
                    *callback_error
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = Some(err);
                    stop.store(true, Ordering::Relaxed);
                }
            });
        };

        let mut interrupt = None;
        let result = py.detach(|| {
            thread::scope(|scope| {
                let (done, finished) = mpsc::channel();
                let (game, limits, report) = (&game, &limits, &report);
                let handle = thread::Builder::new()
                    .name("dame-search".into())
                    .stack_size(SEARCH_STACK_SIZE)
                    .spawn_scoped(scope, move || {
                        let mut searcher =
                            self.searcher.lock().unwrap_or_else(PoisonError::into_inner);
                        let result = searcher.search(game, limits, report);
                        let _ = done.send(());
                        result
                    })
                    .expect("failed to spawn the search thread");
                // Wait for the search while staying responsive to Ctrl-C.
                while let Err(mpsc::RecvTimeoutError::Timeout) = finished.recv_timeout(SIGNAL_POLL)
                {
                    #[allow(clippy::redundant_closure_for_method_calls)]
                    // the closure is higher-ranked
                    let checked = Python::attach(|py| py.check_signals());
                    if let Err(err) = checked {
                        interrupt = Some(err);
                        stop.store(true, Ordering::Relaxed);
                    }
                }
                handle.join().expect("the search thread panicked")
            })
        });
        *self.stop.lock().unwrap_or_else(PoisonError::into_inner) = None;

        if let Some(err) = interrupt.or_else(|| {
            callback_error
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner)
        }) {
            return Err(err);
        }
        Ok(SearchResult {
            best_move: result.best_move.map(Move),
            ponder: result.ponder.map(Move),
            score: result.score,
            depth: result.depth,
            nodes: result.nodes,
            elapsed: result.elapsed.as_secs_f64(),
            pv: moves(&result.pv),
            from_book: result.from_book,
        })
    }

    /// Asks a running search to stop; it returns its best move so far.
    fn stop(&self) {
        if let Some(stop) = &*self.stop.lock().unwrap_or_else(PoisonError::into_inner) {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// Clears the hash table and move-ordering statistics, e.g. for a new game.
    fn clear(&self, py: Python<'_>) {
        py.detach(|| {
            self.searcher
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
        });
    }
}
