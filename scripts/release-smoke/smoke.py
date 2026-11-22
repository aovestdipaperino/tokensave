#!/usr/bin/env python3
"""Run release smoke tests against an installed TokenSave executable."""

import argparse
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile


def run(args, *, cwd, env, error=None, timeout=120):
    print(f"[{platform.system()}] {shlex.join(map(str, args))}", flush=True)
    result = subprocess.run(
        args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        encoding="utf-8", errors="replace", timeout=timeout, check=False,
    )
    print(result.stdout, end="", flush=True)
    if error is None and result.returncode == 0:
        return result.stdout
    if error is not None and result.returncode > 0 and error in result.stdout:
        return result.stdout
    raise RuntimeError(f"unexpected exit status {result.returncode} for {args!r}")


def isolated_env(root, binary):
    env = {
        key: value for key, value in os.environ.items()
        if not key.upper().startswith(("GIT_", "TOKENSAVE_"))
    }
    for key in ("HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA",
                "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "CARGO_HOME"):
        path = root / key.lower()
        path.mkdir()
        env[key] = str(path)
    env.update(
        PATH=f"{binary.parent}{os.pathsep}{env.get('PATH', '')}",
        GIT_CONFIG_NOSYSTEM="1",
        GIT_CONFIG_GLOBAL=str(root / "gitconfig"),
        TOKENSAVE_TELEMETRY="off",
        TOKENSAVE_UPDATE_CHECK="off",
        TOKENSAVE_SKIP_AGENT_MAINTENANCE="1",
        TOKENSAVE_STATE_DIR="",
        TOKENSAVE_USER_CONFIG_DIR=str(Path(env["HOME"]) / ".tokensave"),
        RUST_MIN_STACK="8388608",
    )
    return env


def smoke(binary):
    with tempfile.TemporaryDirectory(prefix="tokensave release smoke ") as directory:
        root = Path(directory).resolve()
        project = root / "git project"
        project.mkdir()
        env = isolated_env(root, binary)

        def ts(*args, **kwargs):
            return run([binary, *args], cwd=project, env=env, **kwargs)

        run(["git", "init", "--quiet"], cwd=project, env=env)
        assert "Usage:" in ts("--help")
        assert "tokensave " in ts("--version")
        assert "rustc:" in ts("--version", "--verbose")

        source = project / "main.py"
        source.write_text("def smoke_original_symbol():\n    return 1\n", encoding="utf-8")
        ts("init", "--git-hook", "--cpu-usage", "low")
        # Re-init is refused with recovery instructions. Subsequent sync and
        # queries prove the existing index remains usable.
        ts("init", "--git-hook", "--cpu-usage", "low", error="already initialized")

        user_state = Path(env["TOKENSAVE_USER_CONFIG_DIR"])
        assert (user_state / "config.toml.lock").is_file(), user_state
        assert (user_state / "state.toml").is_file(), user_state

        hook = project / ".git" / "hooks" / "post-commit"
        assert "tokensave:" in hook.read_text(encoding="utf-8")
        if os.name != "nt":
            assert os.access(hook, os.X_OK)

        ts("sync", str(project))
        assert "smoke_original_symbol" in ts("tool", "search", "smoke_original_symbol")
        source.write_text("def smoke_updated_symbol():\n    return 2\n", encoding="utf-8")
        ts("sync", str(project))
        assert "smoke_updated_symbol" in ts("tool", "search", "smoke_updated_symbol")
        ts("sync", str(project))
        ts("verify", "--deep", "--json")

        cursor = project / ".cursor" / "mcp.json"
        cursor.parent.mkdir(exist_ok=True)
        foreign = {"command": "other-tool"}
        cursor.write_text(json.dumps({"mcpServers": {"other": foreign}}), encoding="utf-8")
        for _ in range(2):
            ts("install", "--agent", "cursor", "--local", "--git-hook", "no")
            servers = json.loads(cursor.read_text(encoding="utf-8"))["mcpServers"]
            assert servers["other"] == foreign
            configured = Path(servers["tokensave"]["command"])
            assert configured.samefile(binary), configured
            assert servers["tokensave"]["args"] == ["serve"]

        before = cursor.read_bytes()
        ts("uninstall", "--agent", "cursor", "--local", "--dry-run")
        assert cursor.read_bytes() == before
        ts("uninstall", "--agent", "cursor", "--local")
        assert json.loads(cursor.read_text(encoding="utf-8"))["mcpServers"] == {"other": foreign}
        assert "smoke_updated_symbol" in ts("tool", "search", "smoke_updated_symbol")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    smoke(parser.parse_args().binary.resolve(strict=True))
