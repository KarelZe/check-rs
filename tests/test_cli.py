"""Tests for the `dame` console script and `python -m dame`."""

from __future__ import annotations

import os
import shutil
import signal
import subprocess
import sys
import sysconfig

import pytest

import dame

# The script that `[project.scripts]` installs next to the interpreter.
SCRIPT = shutil.which("dame", path=sysconfig.get_path("scripts"))
# Without an opening book from the developer's environment.
ENV = {key: value for key, value in os.environ.items() if not key.startswith("DAME_")}


def run(*args: str, stdin: str = "") -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "dame", *args], input=stdin, capture_output=True, text=True, env=ENV, timeout=60
    )


def test_console_script() -> None:
    assert SCRIPT is not None
    result = subprocess.run([SCRIPT, "perft", "2"], capture_output=True, text=True, env=ENV, timeout=60, check=True)
    assert result.stdout.startswith("perft(2) = 81 in ")


def test_perft_divide() -> None:
    result = run("perft", "1", "--divide")
    assert result.returncode == 0
    lines = result.stdout.splitlines()
    assert len(lines) == 10
    assert lines[-1].startswith("perft(1) = 9 in ")


def test_version() -> None:
    result = run("--version")
    assert (result.returncode, result.stdout) == (0, f"dame-cli {dame.__version__}\n")


def test_usage_error() -> None:
    result = run("bogus")
    assert result.returncode == 2
    assert "Usage: dame <COMMAND>" in result.stderr


def test_error() -> None:
    result = run("perft", "2", "--fen", "xx")
    assert result.returncode == 1
    assert result.stderr.startswith("error: invalid FEN")


def test_hub() -> None:
    result = run("hub", stdin="hub\ninit\nlevel depth=4\ngo think\nquit\n")
    assert result.returncode == 0
    lines = result.stdout.splitlines()
    assert lines[0].startswith("id name=dame ")
    assert "wait" in lines
    assert "ready" in lines
    assert lines[-1].startswith("done move=")


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX signals")
def test_ctrl_c_stops_a_search() -> None:
    with subprocess.Popen(
        [sys.executable, "-m", "dame", "search", "--time", "60"], stdout=subprocess.PIPE, env=ENV
    ) as process:
        assert process.stdout is not None
        process.stdout.readline()  # the search is running
        process.send_signal(signal.SIGINT)
        assert process.wait(timeout=10) == -signal.SIGINT
