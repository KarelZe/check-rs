# Playing

The `dame` binary is the engine's command-line front end. Build it from a
checkout with:

```bash
cargo install --path crates/dame-cli
```

## In the terminal

```bash
dame play                        # you play White, 2 s per engine move
dame play --color black --time 5
dame play --fen "W:W28,32:B17,23,27"
```

Enter moves in standard notation: `32-28` for a quiet move and `28x19` for a
capture. When two captures share their start and end square, list the captured
pieces as well, e.g. `28x10x14x23`. The commands `moves`, `hint`, `undo` and
`quit` do what you would expect.

## Opening book

An opening book is a PDN file of games; the engine follows the first ten
moves of each (see [Opening book](design.md#opening-book)). Any game collection
works, e.g. a download from a game database. Set `DAME_BOOK` to the file and the
engine opens games from it, in `dame play` and `dame hub`:

```bash
export DAME_BOOK=~/draughts/games.pdn
dame play                        # the engine plays book moves while it has them
```

While a position is in the book, the engine plays a book move without
searching; `dame play` marks it with `(book)`. A missing file or an illegal
move in it is an error. `dame search` always searches, so it ignores the book.

## On macOS

`Dame.app` is a native board for macOS that plays against `dame` or any
other Hub engine. See [macOS app](macos.md).

## In a GUI: the Hub protocol

`dame hub` speaks the [Hub protocol](https://github.com/rhalbersma/scan/blob/master/protocol.txt)
(version 2) on standard input and output. Hub is the protocol of Scan; on macOS
and Linux, Fabien Letouzey's Hub GUI (a small Java program distributed with
Scan) can run any Hub engine. Point `hub.ini` at the binary:

```ini
engine-dir = /path/to/dame-rs/target/release
engine-command = ./dame hub
```

The engine declares two parameters:

| Parameter   | Type   | Default | Meaning                                            |
|-------------|--------|---------|----------------------------------------------------|
| `hash`      | int    | `64`    | transposition table size in MiB                    |
| `book-file` | string | `$DAME_BOOK` or `""` | opening book: a PDN file of games, see [Opening book](design.md#opening-book) |

A GUI that sets `book-file` overrides `DAME_BOOK`; an empty value turns the
book off. The book is loaded on `init`, and a file that cannot be loaded is
reported with an `error` line.

Hub engines also plug into match managers and bots, for example
[pydraughts](https://github.com/AttackingOrDefending/pydraughts)' `HubEngine`.

!!! note "CheckerBoard"
    Martin Fierz's CheckerBoard only supports 8x8 variants, and its engine
    interface is a Windows DLL, so it cannot host an international draughts
    engine.

## Other commands

```bash
dame search --time 5             # analyse the start position
dame search --fen "B:W18,24:BK3" --depth 20
dame perft 9                     # move generator node count
dame perft 3 --divide
```
