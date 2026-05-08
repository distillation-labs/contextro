#!/usr/bin/env python3

"""Auto-restarting HTTP dev server for live MCP testing."""

from __future__ import annotations

import asyncio
import logging
import os
import signal
import sys
from pathlib import Path

from contextro_mcp.parsing.file_watcher import DebouncedFileWatcher

logger = logging.getLogger("contextro.dev")

WATCH_EXTENSIONS = {".py", ".toml"}
WATCH_TOP_LEVEL_DIRS = {"scripts", "src"}
IGNORED_PARTS = {".git", ".mypy_cache", ".pytest_cache", ".ruff_cache", ".venv", "__pycache__"}


class DevServerSupervisor:
    """Runs Contextro in a child process and restarts it on source changes."""

    def __init__(
        self,
        *,
        project_root: Path,
        watch_root: Path,
        debounce_seconds: float,
    ):
        self.project_root = project_root
        self.watch_root = watch_root
        self.debounce_seconds = debounce_seconds
        self._process: asyncio.subprocess.Process | None = None
        self._restart_lock = asyncio.Lock()
        self._shutdown_event = asyncio.Event()
        self._watcher = DebouncedFileWatcher(
            project_root=watch_root,
            callback=self._restart_child,
            debounce_delay=debounce_seconds,
            should_ignore_path=self._should_ignore_path,
            supported_extensions=WATCH_EXTENSIONS,
        )

    def _child_env(self) -> dict[str, str]:
        env = os.environ.copy()
        env.setdefault("CTX_TRANSPORT", "http")
        env.setdefault("CTX_HTTP_HOST", "0.0.0.0")
        env.setdefault("CTX_HTTP_PORT", "8000")
        env.setdefault("CTX_AUTO_WARM_START", "true")
        return env

    @staticmethod
    def _should_ignore_path(file_path: Path, root: Path) -> bool:
        try:
            relative = file_path.relative_to(root)
        except ValueError:
            return True

        if not relative.parts:
            return True

        if relative.name == "pyproject.toml":
            return False

        if relative.parts[0] not in WATCH_TOP_LEVEL_DIRS:
            return True

        return any(part in IGNORED_PARTS for part in relative.parts)

    async def _start_child(self, *, reason: str) -> None:
        command = [sys.executable, "-m", "contextro_mcp.server"]
        logger.info("Starting Contextro dev server (%s)", reason)
        self._process = await asyncio.create_subprocess_exec(
            *command,
            cwd=str(self.project_root),
            env=self._child_env(),
        )
        asyncio.create_task(self._monitor_child(self._process))

    async def _monitor_child(self, process: asyncio.subprocess.Process) -> None:
        returncode = await process.wait()
        if self._shutdown_event.is_set():
            return

        async with self._restart_lock:
            if process is not self._process:
                return
            self._process = None
            logger.warning("Contextro dev server exited with code %s; restarting", returncode)
            await asyncio.sleep(1.0)
            if not self._shutdown_event.is_set():
                await self._start_child(reason="crash recovery")

    async def _stop_child(self) -> None:
        process = self._process
        self._process = None
        if process is None or process.returncode is not None:
            return

        logger.info("Stopping Contextro dev server")
        process.terminate()
        try:
            await asyncio.wait_for(process.wait(), timeout=10.0)
        except asyncio.TimeoutError:
            logger.warning("Contextro dev server did not stop in time; killing it")
            process.kill()
            await process.wait()

    async def _restart_child(self) -> None:
        async with self._restart_lock:
            if self._shutdown_event.is_set():
                return

            logger.info("Source change detected; reloading Contextro dev server")
            await self._stop_child()
            if not self._shutdown_event.is_set():
                await self._start_child(reason="source reload")

    async def run(self) -> None:
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            try:
                loop.add_signal_handler(sig, self._shutdown_event.set)
            except NotImplementedError:
                signal.signal(sig, lambda *_: self._shutdown_event.set())

        logger.info(
            "Watching %s for changes (debounce %.1fs)",
            self.watch_root,
            self.debounce_seconds,
        )
        await self._start_child(reason="initial start")
        await self._watcher.start()

        try:
            await self._shutdown_event.wait()
        finally:
            await self._watcher.stop()
            await self._stop_child()


def _default_project_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _env_float(name: str, default: float) -> float:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        value = float(raw)
    except ValueError:
        return default
    return value if value > 0 else default


async def _async_main() -> None:
    project_root = Path(os.environ.get("CONTEXTRO_DEV_PROJECT_ROOT", _default_project_root()))
    watch_root = Path(os.environ.get("CONTEXTRO_DEV_WATCH_ROOT", project_root))
    supervisor = DevServerSupervisor(
        project_root=project_root.resolve(),
        watch_root=watch_root.resolve(),
        debounce_seconds=_env_float("CONTEXTRO_DEV_DEBOUNCE_SECONDS", 1.0),
    )
    await supervisor.run()


def main() -> None:
    log_level = getattr(
        logging,
        os.environ.get("CONTEXTRO_DEV_LOG_LEVEL", "INFO").upper(),
        logging.INFO,
    )
    logging.basicConfig(
        level=log_level,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    asyncio.run(_async_main())


if __name__ == "__main__":
    main()
