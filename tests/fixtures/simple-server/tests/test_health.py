"""Tests for the simple server fixture."""

from __future__ import annotations

from simple_server.main import health_payload


def test_health_payload() -> None:
    """Health payload reports ok."""
    assert health_payload() == {"status": "ok"}
