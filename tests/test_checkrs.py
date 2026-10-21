"""Smoke tests for the `dame` extension module."""

from __future__ import annotations

import dame


def test_version_is_exposed() -> None:
    assert dame.__version__.count(".") == 2
