# dame

## Introduction

`dame`[^name] is an extremely fast [international draughts](https://en.wikipedia.org/wiki/International_draughts)
(10x10 checkers) engine written in Rust 🦀, with a pythonic API.

[^name]: *Dame* is German for draughts (checkers), and also for a king in the game: a
piece that reaches the far row is crowned a *Dame*.

!!! warning
    `dame` is in early development. The API will change.

## Installation

Install the latest version from [pypi](https://pypi.org/project/dame-rs/):

```bash
uv add dame-rs
```

or if you prefer slow:

```bash
pip install dame-rs
```

## Usage

```python
import dame

board = dame.Board()  # or dame.Board("W:W31-50:B1-20")
board.legal_moves  # [Move('31-26'), Move('32-27'), ...]
board.push("32-28")
board.push("19-23")
board.legal_moves  # [Move('28x19x23')]: capturing is mandatory

engine = dame.Engine(hash_mb=64)
result = engine.search(board, time=1.0)
result.best_move, result.score, result.depth  # (Move('28x19x23'), 2, 13)
board.push(result.best_move)
print(board)
```

The search runs without the GIL and reports progress through an optional
`info` callback. See the [API Reference](api.md).

To play against the engine or connect it to a GUI, see [Playing](playing.md).
