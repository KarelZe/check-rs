"""Integration tests for the `dame` extension module."""

from __future__ import annotations

import copy
import pickle
import threading
from typing import TYPE_CHECKING

import pytest

import dame

if TYPE_CHECKING:
    from pathlib import Path

from dame import BLACK, WHITE, Board, Color, Engine, IllegalMoveError, Move, OpeningBook, Piece, Termination


def test_version_is_exposed() -> None:
    assert dame.__version__.count(".") == 2


class TestBoard:
    def test_starting_position(self) -> None:
        board = Board()
        assert board.fen() == Board(dame.STARTING_FEN).fen()
        assert board.turn is WHITE
        assert [str(move) for move in board.legal_moves] == [
            "31-26",
            "32-27",
            "33-28",
            "34-29",
            "35-30",
            "31-27",
            "32-28",
            "33-29",
            "34-30",
        ]
        assert board.ply == 0
        assert not board.is_game_over()

    @pytest.mark.parametrize("fen", ["", "X:W1", "W:W51", "W:W1:B1", "W:W3"])
    def test_invalid_fen(self, fen: str) -> None:
        with pytest.raises(ValueError, match=r"FEN|square|promotion"):
            Board(fen)

    def test_push_and_pop(self) -> None:
        board = Board()
        first = board.push("32-28")
        board.push(board.parse_move("19-23"))
        assert board.turn is WHITE
        assert board.must_capture
        assert [move.hub() for move in board.legal_moves] == ["28x19x23"]
        assert board.move_stack[0] == first
        assert board.pop().hub() == "19-23"
        assert board.pop() == first
        with pytest.raises(IndexError):
            board.pop()

    def test_illegal_moves(self) -> None:
        board = Board()
        with pytest.raises(IllegalMoveError, match="illegal"):
            board.push("32-23")
        with pytest.raises(IllegalMoveError, match="invalid"):
            board.push("hello")
        assert issubclass(IllegalMoveError, ValueError)
        other = Board("W:W28,32:B17,23,27").legal_moves[0]
        assert not board.is_legal(other)
        with pytest.raises(IllegalMoveError):
            board.push(other)

    def test_maximum_capture_rule(self) -> None:
        board = Board("W:W28,32:B17,23,27")
        (move,) = board.legal_moves
        assert (move.from_square, move.to_square, move.captured) == (32, 12, [17, 27])
        assert move.is_capture
        assert str(move) == "32x12"
        assert repr(move) == "Move('32x12x17x27')"

    def test_pieces(self) -> None:
        board = Board("B:WK10,28:B19")
        assert board.piece_at(10) == Piece(WHITE, king=True)
        assert board.piece_at(19) == Piece(BLACK)
        assert board.piece_at(1) is None
        with pytest.raises(ValueError, match="square"):
            board.piece_at(51)
        assert {square: piece.symbol() for square, piece in board.piece_map().items()} == {10: "W", 19: "b", 28: "w"}

    def test_outcome(self) -> None:
        board = Board("W:W46:B41,37")
        outcome = board.outcome()
        assert outcome is not None
        assert outcome.winner is BLACK
        assert outcome.termination is Termination.NO_MOVES
        assert outcome.result() == "0-2"
        assert board.legal_moves == []
        assert Board().outcome() is None

    def test_repetition(self) -> None:
        board = Board("W:WK50:BK1")
        for _ in range(2):
            for move in ["50-44", "1-7", "44-50", "7-1"]:
                board.push(move)
        outcome = board.outcome()
        assert outcome is not None
        assert outcome.winner is None
        assert outcome.termination is Termination.REPETITION

    def test_copy_and_pickle(self) -> None:
        board = Board()
        board.push("32-28")
        for clone in [board.copy(), copy.copy(board), copy.deepcopy(board), pickle.loads(pickle.dumps(board))]:
            assert clone == board
            assert clone.move_stack == board.move_stack
            clone.push(clone.legal_moves[0])
            assert clone != board

    def test_boards_are_unhashable_but_moves_are_hashable(self) -> None:
        with pytest.raises(TypeError):
            hash(Board())
        assert len(set(Board().legal_moves)) == 9

    def test_zobrist_hash_tracks_position(self) -> None:
        board = Board()
        start = board.zobrist_hash()
        board.push("32-28")
        assert board.zobrist_hash() != start
        board.pop()
        assert board.zobrist_hash() == start

    def test_str_is_a_diagram(self) -> None:
        text = str(Board())
        assert text.splitlines()[0] == "  b   b   b   b   b"
        assert text.endswith("white to move")


def test_color() -> None:
    assert ~WHITE is BLACK
    assert Color.BLACK is BLACK
    assert str(WHITE) == "white"


@pytest.mark.parametrize(("depth", "nodes"), [(1, 9), (2, 81), (3, 658), (4, 4265), (5, 27117)])
def test_perft(depth: int, nodes: int) -> None:
    assert dame.perft(Board(), depth) == nodes


class TestEngine:
    def test_finds_the_only_move(self) -> None:
        result = Engine(hash_mb=1).search(Board("W:W28,32:B17,23,27"), depth=4)
        assert result.best_move is not None
        assert result.best_move.hub() == "32x12x17x27"
        assert result.pv[0] == result.best_move

    def test_finds_a_mate(self) -> None:
        result = Engine(hash_mb=1).search(Board("W:WK46:B10"), depth=4)
        assert result.best_move == Board("W:WK46:B10").parse_move("46x5")
        assert result.mate == 1

    def test_finished_game(self) -> None:
        result = Engine(hash_mb=1).search(Board("W:W46:B41,37"), depth=4)
        assert result.best_move is None

    def test_limits(self) -> None:
        engine = Engine(hash_mb=1)
        assert engine.search(Board(), depth=3).depth == 3
        assert engine.search(Board(), nodes=5_000).nodes < 10_000
        assert engine.search(Board(), time=0.05).elapsed < 1.0
        with pytest.raises(ValueError, match="time"):
            engine.search(Board(), time=-1.0)
        with pytest.raises(ValueError, match="depth"):
            engine.search(Board(), depth=0)

    def test_info_callback(self) -> None:
        infos: list[dame.SearchInfo] = []
        Engine(hash_mb=1).search(Board(), depth=4, info=infos.append)
        assert [info.depth for info in infos] == [1, 2, 3, 4]
        assert all(info.pv for info in infos)

    def test_callback_errors_propagate(self) -> None:
        def fail(_info: dame.SearchInfo) -> None:
            msg = "boom"
            raise RuntimeError(msg)

        with pytest.raises(RuntimeError, match="boom"):
            Engine(hash_mb=1).search(Board(), depth=4, info=fail)

    def test_stop_from_another_thread(self) -> None:
        engine = Engine(hash_mb=1)
        timer = threading.Timer(0.1, engine.stop)
        timer.start()
        result = engine.search(Board())
        timer.join()
        assert result.best_move in Board().legal_moves

    def test_book(self) -> None:
        book = OpeningBook.from_text("32-28 18-23\n32-28 17-21\n33-28")
        assert len(book) == 2
        assert book.moves(Board()) == [(_move("32-28"), 2), (_move("33-28"), 1)]
        result = Engine(hash_mb=1, book=book).search(Board(), depth=4)
        assert result.from_book
        assert str(result.best_move) in {"32-28", "33-28"}
        with pytest.raises(ValueError, match="line 1"):
            OpeningBook.from_text("32-23")

    def test_book_from_file(self, tmp_path: Path) -> None:
        path = tmp_path / "book.txt"
        path.write_text("32-28\n")
        assert len(OpeningBook.from_file(path)) == 1
        with pytest.raises(OSError, match="missing"):
            OpeningBook.from_file(path.parent / "missing.txt")

    def test_concurrent_searches(self) -> None:
        results: list[Move | None] = []

        def worker() -> None:
            results.append(Engine(hash_mb=1).search(Board(), depth=5).best_move)

        threads = [threading.Thread(target=worker) for _ in range(4)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        assert len(results) == 4
        assert all(move in Board().legal_moves for move in results)


def _move(text: str) -> Move:
    return Board().parse_move(text)
