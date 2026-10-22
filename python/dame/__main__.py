"""
The `dame` command line, as the `dame` script or `python -m dame`.

Runs the same Rust code as the `dame` binary inside the extension; see the
command line documentation for its commands.
"""

from __future__ import annotations

import signal
import sys

from dame._core import cli


def main() -> None:
    """Run the command line on `sys.argv` and exit with its exit code."""
    # The engine never returns to Python while it runs, so Python would only
    # see Ctrl-C afterwards; restore the default action, as in the binary.
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    sys.exit(cli(["dame", *sys.argv[1:]]))


if __name__ == "__main__":
    main()
