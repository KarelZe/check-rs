from collections.abc import Callable
from enum import Enum
from os import PathLike
from typing import Any, final

__version__: str
STARTING_FEN: str
WHITE: Color
BLACK: Color

class IllegalMoveError(ValueError):
    """Raised when a move is illegal or cannot be parsed."""

@final
class Color(Enum):
    """A side. White moves first."""

    WHITE = 0
    BLACK = 1

    def __invert__(self) -> Color:
        """The other side: `~Color.WHITE` is `Color.BLACK`."""

@final
class Piece:
    """A piece: a color and whether it is a king."""

    def __init__(self, color: Color, *, king: bool = False) -> None: ...
    @property
    def color(self) -> Color:
        """The owner of the piece."""
    @property
    def is_king(self) -> bool:
        """Whether the piece is a king."""
    def symbol(self) -> str:
        """`w`/`b` for men, `W`/`B` for kings."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Move:
    """
    A legal move: origin, destination and the captured squares.

    Moves are obtained from `Board.legal_moves`, `Board.parse_move` or a search.
    `str(move)` gives standard notation (`32-28`, `28x19`); `move.hub()` also
    lists the captured squares (`28x19x23`), which is unambiguous.
    """

    @property
    def from_square(self) -> int:
        """The origin square, 1 to 50."""
    @property
    def to_square(self) -> int:
        """The destination square, 1 to 50."""
    @property
    def captured(self) -> list[int]:
        """The squares of the captured pieces in ascending order."""
    @property
    def is_capture(self) -> bool:
        """Whether the move captures."""
    def hub(self) -> str:
        """The move in Hub notation, which lists the captured squares."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Termination(Enum):
    """Why a game ended."""

    NO_MOVES = 0
    """The side to move has no legal move and loses."""
    REPETITION = 1
    """The same position occurred three times."""
    TWENTY_FIVE_MOVES = 2
    """25 moves by each side with only kings moved and nothing captured."""
    SIXTEEN_MOVES = 3
    """A lone king against three pieces including a king, for 16 moves each."""
    FIVE_MOVES = 4
    """A lone king against at most two pieces including a king, for 5 moves each."""

@final
class Outcome:
    """The result of a finished game."""

    @property
    def winner(self) -> Color | None:
        """The winner, or `None` for a draw."""
    @property
    def termination(self) -> Termination:
        """The rule that ended the game."""
    def result(self) -> str:
        """The PDN result: `"2-0"`, `"0-2"` or `"1-1"`."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Board:
    """
    A draughts board: the start position plus the moves played from it.

    Squares are numbered 1 to 50; White starts on 31-50 and moves first.
    The board enforces the rules: captures are mandatory, the maximum
    capture must be taken, and games end by the FMJD rules.

    Args:
        fen: The start position in FEN notation, e.g. `"W:W31-50:B1-20"`.
            Defaults to the standard starting position.

    Raises:
        ValueError: If `fen` is not a valid position.

    Examples:
        >>> board = Board()
        >>> board.push("32-28")
        Move('32-28')
        >>> board.push("19-23")
        Move('19-23')
        >>> board.legal_moves
        [Move('28x19x23')]
    """

    def __init__(self, fen: str | None = None) -> None: ...
    def fen(self) -> str:
        """The current position in FEN notation."""
    @property
    def turn(self) -> Color:
        """The side to move."""
    @property
    def legal_moves(self) -> list[Move]:
        """The legal moves; empty once the game is over."""
    @property
    def move_stack(self) -> list[Move]:
        """The moves played so far."""
    @property
    def ply(self) -> int:
        """The number of moves played so far."""
    @property
    def must_capture(self) -> bool:
        """Whether the side to move has to capture."""
    def parse_move(self, text: str) -> Move:
        """
        Matches move text against the legal moves.

        Accepts `32-28`, `28x19`, Hub notation listing the captured squares
        (`28x19x23`) and PDN capture paths (`28x19x10`).

        Raises:
            IllegalMoveError: If no single legal move matches.
        """
    def is_legal(self, move: Move) -> bool:
        """Whether `move` is legal in the current position."""
    def push(self, move: Move | str) -> Move:
        """
        Plays a move given as a `Move` or as text and returns it.

        Raises:
            IllegalMoveError: If the move is illegal.
        """
    def pop(self) -> Move:
        """
        Takes back the last move and returns it.

        Raises:
            IndexError: If no move has been played.
        """
    def piece_at(self, square: int) -> Piece | None:
        """
        The piece on `square` (1 to 50), if any.

        Raises:
            ValueError: If `square` is not between 1 and 50.
        """
    def piece_map(self) -> dict[int, Piece]:
        """All pieces by square."""
    def outcome(self) -> Outcome | None:
        """The outcome if the game is over, else `None`."""
    def is_game_over(self) -> bool:
        """Whether the game is over."""
    def zobrist_hash(self) -> int:
        """A 64-bit Zobrist hash of the current position."""
    def copy(self) -> Board:
        """An independent copy of the board, including the move history."""
    def __copy__(self) -> Board: ...
    def __deepcopy__(self, memo: Any) -> Board: ...
    def __eq__(self, other: object) -> bool:
        """Boards are equal if their current positions are equal."""

@final
class OpeningBook:
    """
    An opening book: weighted moves by position.

    A book is built from games in PDN: the first moves of each game are added
    from the starting position, and moves shared by several games gain weight.

    Examples:
        >>> book = OpeningBook.from_pdn("1. 32-28 18-23 *\\n1. 32-28 17-21 *\\n1. 33-28 *")
        >>> book.moves(Board())
        [(Move('32-28'), 2), (Move('33-28'), 1)]
    """

    @staticmethod
    def from_pdn(text: str, *, plies: int = 20) -> OpeningBook:
        """
        Builds a book from the first `plies` moves of each game in PDN `text`.

        Games of other variants (a `GameType` other than 20) and games that
        start from another position (a `FEN` tag) are skipped.

        Raises:
            ValueError: If a game contains an illegal move.
        """
    @staticmethod
    def from_file(path: str | PathLike[str], *, plies: int = 20) -> OpeningBook:
        """
        Builds a book from the first `plies` moves of each game in the PDN file at `path`.

        Raises:
            OSError: If the file cannot be read.
            ValueError: If a game contains an illegal move.
        """
    def moves(self, board: Board) -> list[tuple[Move, int]]:
        """The book moves for the current position with their weights, heaviest first."""
    def __len__(self) -> int: ...

@final
class SearchInfo:
    """Progress reported after each completed search iteration."""

    @property
    def depth(self) -> int:
        """The completed depth in plies."""
    @property
    def seldepth(self) -> int:
        """The deepest ply reached, including captures resolved at the leaves."""
    @property
    def score(self) -> int:
        """The score in centi-men from the side to move's point of view."""
    @property
    def nodes(self) -> int:
        """Nodes searched so far."""
    @property
    def elapsed(self) -> float:
        """Seconds spent so far."""
    @property
    def pv(self) -> list[Move]:
        """The principal variation."""

@final
class SearchResult:
    """The outcome of a search."""

    @property
    def best_move(self) -> Move | None:
        """The best move, or `None` if the game is over."""
    @property
    def ponder(self) -> Move | None:
        """The expected reply."""
    @property
    def score(self) -> int:
        """The score in centi-men from the side to move's point of view."""
    @property
    def mate(self) -> int | None:
        """Moves until mate for a forced win (positive) or loss (negative), else `None`."""
    @property
    def depth(self) -> int:
        """The last completed depth; 0 for book moves."""
    @property
    def nodes(self) -> int:
        """The number of nodes searched."""
    @property
    def elapsed(self) -> float:
        """Seconds spent."""
    @property
    def pv(self) -> list[Move]:
        """The principal variation."""
    @property
    def from_book(self) -> bool:
        """Whether the move came from the opening book."""

@final
class Engine:
    """
    A search engine. Reusing one engine across moves keeps its hash table.

    The search runs without holding the GIL, so other Python threads keep
    running; `KeyboardInterrupt` stops it.

    Args:
        hash_mb: Transposition table size in MiB.
        book: An opening book to play from before searching.

    Examples:
        >>> engine = Engine(hash_mb=16)
        >>> result = engine.search(Board("W:WK46:B10"), depth=4)
        >>> result.best_move, result.mate
        (Move('46x5x10'), 1)
    """

    def __init__(self, *, hash_mb: int = 64, book: OpeningBook | None = None) -> None: ...
    def search(
        self,
        board: Board,
        *,
        depth: int | None = None,
        nodes: int | None = None,
        time: float | None = None,
        info: Callable[[SearchInfo], object] | None = None,
    ) -> SearchResult:
        """
        Searches the current position of `board`.

        The search stops at the first limit reached. Without limits it runs
        until `stop()` is called from another thread.

        Args:
            board: The position to search, with its history for repetitions.
            depth: Maximum depth in plies.
            nodes: Maximum number of nodes.
            time: Maximum time in seconds.
            info: Called with a `SearchInfo` after each completed iteration.

        Returns:
            The best move with its score and principal variation.

        Raises:
            ValueError: If a limit is invalid.
        """
    def stop(self) -> None:
        """Asks a running search to stop; it returns its best move so far."""
    def clear(self) -> None:
        """Clears the hash table and move-ordering statistics, e.g. for a new game."""

def perft(board: Board, depth: int) -> int:
    """
    Counts the leaf nodes of the legal move tree at `depth` plies.

    Examples:
        >>> perft(Board(), 3)
        658
    """
