"""Authentication header helpers for SAITEC skill API calls."""

import json
import os
from pathlib import Path
from typing import Iterable, Optional


API_KEY_ENV = "SAITEC_API_KEY"
AUTH_FILE_ENV = "SAITEC_TUI_AUTH_FILE"
SAITEC_TUI_HOME_ENV = "SAITEC_TUI_HOME"


def _clean_api_key(value: Optional[str]) -> Optional[str]:
    if not value:
        return None
    trimmed = value.strip()
    return trimmed or None


def _candidate_auth_files() -> Iterable[Path]:
    explicit = _clean_api_key(os.getenv(AUTH_FILE_ENV))
    if explicit:
        yield Path(explicit)

    saitec_home = _clean_api_key(os.getenv(SAITEC_TUI_HOME_ENV))
    if saitec_home:
        yield Path(saitec_home) / "auth.json"

    home = Path.home()
    yield home / ".saitec_tui" / "auth.json"
    yield home / ".saitec-tui" / "auth.json"


def _api_key_from_auth_file(path: Path) -> Optional[str]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None

    if not isinstance(data, dict):
        return None

    value = data.get("api_key")
    if isinstance(value, str):
        return _clean_api_key(value)
    return None


def resolve_api_key() -> str:
    env_key = _clean_api_key(os.getenv(API_KEY_ENV))
    if env_key:
        return env_key

    for path in _candidate_auth_files():
        file_key = _api_key_from_auth_file(path)
        if file_key:
            return file_key

    return ""


def build_auth_headers() -> dict:
    return {"X-API-Key": resolve_api_key()}
