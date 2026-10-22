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
