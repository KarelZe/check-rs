"""Benchmarks for CodSpeed; run with `pytest tests/test_benchmarks.py --codspeed`."""

from __future__ import annotations

import pytest

from dame import Board, Engine, perft


@pytest.mark.benchmark
def test_perft_initial_depth_5() -> None:
    assert perft(Board(), 5) == 27_117


@pytest.mark.benchmark
def test_search_initial_depth_6() -> None:
    assert Engine(hash_mb=4).search(Board(), depth=6).best_move is not None


@pytest.mark.benchmark
def test_legal_moves_and_push() -> None:
    board = Board()
    for _ in range(20):
        moves = board.legal_moves
        if not moves:
            break
        board.push(moves[0])
