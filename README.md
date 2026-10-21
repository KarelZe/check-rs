# dame-rs

`dame` is an extremely fast [international draughts](https://en.wikipedia.org/wiki/International_draughts)
(10x10 checkers) engine written in Rust 🦀, with a pythonic API.

> [!WARNING]
> `dame` is in early development. The API will change.

## Installation

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
`info` callback. See the [API Reference](https://KarelZe.github.io/dame-rs/api/).

## Playing

Play against the engine in the terminal, or plug it into any GUI that speaks
the [Hub protocol](https://github.com/rhalbersma/scan/blob/master/protocol.txt):

```bash
cargo install --path crates/dame-cli
dame play --time 2
dame hub
```

See [Playing](https://KarelZe.github.io/dame-rs/playing/) for details and
[Engine design](https://KarelZe.github.io/dame-rs/design/) for how the engine works.

## Development

```bash
uv sync                               # build the extension and install dev tools
cargo test --release                  # Rust tests
uv run pytest                         # Python tests
uv run prek run --all-files           # linters and formatters
```

See [AGENTS.md](AGENTS.md) for the full workflow.

## License

`dame` is licensed under the [MIT license](LICENSE).

## Contact

To get in contact, please open an issue or contact me via `github@markusbilz.com`.
