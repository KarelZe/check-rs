"""
An extremely fast international draughts engine, written in Rust.

Examples:
    >>> import dame
    >>> board = dame.Board()
    >>> len(board.legal_moves)
    9
    >>> board.push("32-28")
    Move('32-28')
    >>> board.turn
    Color.BLACK
    >>> result = dame.Engine().search(board, depth=6)
    >>> result.best_move in board.legal_moves
    True

"""

from __future__ import annotations

from dame._core import (
    BLACK,
    STARTING_FEN,
    WHITE,
    Board,
    Color,
    Engine,
    IllegalMoveError,
    Move,
    OpeningBook,
    Outcome,
    Piece,
    SearchInfo,
    SearchResult,
    Termination,
    __version__,
    perft,
)

__all__ = [
    "BLACK",
    "STARTING_FEN",
    "WHITE",
    "Board",
    "Color",
    "Engine",
    "IllegalMoveError",
    "Move",
    "OpeningBook",
    "Outcome",
    "Piece",
    "SearchInfo",
    "SearchResult",
    "Termination",
    "__version__",
    "perft",
]
