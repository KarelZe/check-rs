# Engine design

`dame` plays [international draughts](https://en.wikipedia.org/wiki/International_draughts):
a 10x10 board, men that capture backwards, flying kings, and the majority
capture rule. This page records how the engine is built and why.

## Crates

| Crate            | Purpose                                                       |
|------------------|---------------------------------------------------------------|
| `dame`        | engine core: board, move generation, search, evaluation. No dependencies. |
| `dame-cli`    | the `dame` binary: Hub protocol, terminal play, perft.     |
| `dame-python` | PyO3 bindings, imported as `dame._core`.                    |
| `dame-macos`  | native macOS app: an AppKit board that drives Hub engines.  |

## Board representation

Squares are numbered 1–50 as in standard notation. White starts on 31–50 at
the bottom and moves first.

A position is three 64-bit bitboards (`white`, `black`, `kings`) plus the side
to move and an incremental Zobrist hash. It is `Copy`, and the search uses
copy-make instead of undoing moves.

Square `s` maps to bit `d + d / 10`, where `d = s - 1`. Every pair of rows
therefore occupies 11 bits, one of which is an unused *ghost* bit (bits 10,
21, 32 and 43). This makes every diagonal step a constant shift:

| direction  | shift |
|------------|-------|
| up-left    | −6    |
| up-right   | −5    |
| down-left  | +5    |
| down-right | +6    |

A step off the side of the board lands on a ghost bit, and a step off the top
or bottom falls outside the 54 used bits. Masking with the set of empty squares
therefore handles the board edges without per-row masks.

## Move generation

Moves store the origin, the destination and the set of captured pieces (16
bytes). The squares a capture lands on in between are not stored: by the rules,
two sequences with the same origin, destination and captured set are the same
move.

* **Captures** are mandatory, and only sequences that take the most pieces are
  legal. A depth-first search enumerates each piece's sequences. Captured pieces
  stay on the board until the move ends ("Turkish strike"): they block, and
  cannot be jumped twice. When a better count is found the list is cleared,
  and duplicates are dropped as they are added.
* **Flying kings** may land on any empty square behind a captured piece.
  Continuing straight on from a later landing square repeats what the first
  landing square finds, so only the first one explores the straight direction.
  This is lossless; it keeps duplicate sequences from multiplying.
* **Promotion** happens only when a move *ends* on the last row. A man that
  passes through it during a capture stays a man.

The generator is validated with *perft* against published node counts from
the perft tables of Rein Halbersma's [dctl](https://github.com/rhalbersma/dctl)
and the [damforum.nl perft thread](https://damforum.nl/bb3/viewtopic.php?f=53&t=2308).
The suite covers the initial position to depth 9, plus king-heavy and
promotion-heavy positions (`crates/dame/tests/perft.rs`).

## Search

The search is a principal variation search with iterative deepening and
aspiration windows. It also uses:

* a transposition table: 64-byte clusters of four entries, replacing stale
  entries first and then shallow ones;
* move ordering: table move, captures by size, two killer moves, history;
* late move reductions for quiet moves;
* a one-ply extension when a node has only one legal move;
* a quiescence search that resolves pending captures. Capturing is mandatory,
  so there is no stand-pat while the side to move has a capture.

There is no null-move pruning: zugzwang is common in draughts.

Draws inside the tree are two-fold repetition and the 25-move rule. At the game
level (`Game`), the engine implements the FMJD rules:

* threefold repetition;
* the 25-move rule: kings only, no captures;
* the 16- and 5-move rules for a lone king against a few pieces.

## Evaluation

`Evaluate` is a trait, and the searcher is generic over it, so evaluation
functions are monomorphized into the search. The baseline `Classical`
evaluation scores, in centi-men from the side to move's point of view:

* material: man 100, king 300;
* advancement of men;
* center control;
* back-row guards;
* a damping factor in materially drawish endings.

## Opening book

`OpeningBook` maps Zobrist hashes to weighted moves. It is built from a
collection of games in PDN (Portable Draughts Notation), the format game
databases and draughts sites export:

```text
[Event "Two openings"]
[Result "1-1"]
1. 32-28 18-23 2. 37-32 12-18 {comments and (side lines) are skipped} 1-1

1. 33-28 17-21 *
```

The first 20 plies (ten moves per side) of each game are added from the
starting position, and moves that several games share gain weight. Games of
other variants (`GameType` other than 20) and games that start from a set-up
position (`FEN`) are skipped; an illegal move is an error that names the game
and line. Python's `OpeningBook.from_pdn` and `from_file` take another depth
as `plies`; `dame` reads it from `DAME_BOOK_PLIES` or the Hub parameter
`book-plies`, and the macOS app from its status bar.

The searcher picks book moves with probability proportional to their weight.
The keys are stable across releases: Zobrist keys derive from a fixed seed.

## Endgame databases

The search can probe an `EndgameDb` in positions with few pieces, when the
side to move has no capture. No database ships with `dame` yet.
