
## Colored output

In a terminal, `dame play`, `dame search`, `dame perft` and the help are
colored: White's pieces bold and Black's red, the last move's squares yellow,
moves cyan, and scores green or red for the side to move. Output that is
piped or redirected stays plain, and `dame hub` is never colored because GUIs
read it. The usual environment variables change that, as in ruff, ty or uv:

```bash
NO_COLOR=1 dame play             # no colors
FORCE_COLOR=1 dame search | less -R   # colors even through a pipe
```

`NO_COLOR` wins over `FORCE_COLOR` (and `CLICOLOR_FORCE`). On Windows, colors
are on in Windows Terminal and Unix-like terminals such as Git Bash; the classic
console needs `FORCE_COLOR=1`, and only shows them if it supports ANSI codes.
