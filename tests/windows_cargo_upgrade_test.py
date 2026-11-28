"""Windows integration checks. Run with a freshly built TOKENSAVE_TEST_EXE."""

import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import unittest


HELPER = Path(__file__).resolve().parents[1] / "scripts" / "upgrade-windows.ps1"
BINARY = "tokensave.exe"
BACKUP = BINARY + ".cargo-upgrade-backup"
READY_TIMEOUT = 30
UPGRADE_TIMEOUT = 120


@unittest.skipUnless(os.name == "nt", "Windows Cargo upgrades")
class WindowsCargoUpgrade(unittest.TestCase):
    def setUp(self):
        self.addCleanup((temporary := tempfile.TemporaryDirectory()).cleanup)
        self.root = Path(temporary.name)
        self.install = self.root / "Cargo root"
        self.exe = self.install / "bin" / BINARY
        self.exe.parent.mkdir(parents=True)
        self.binary = Path(os.environ["TOKENSAVE_TEST_EXE"]).resolve(strict=True)
        self.fixture = self.root / "replacement"
        source = self.fixture / "src"
        source.mkdir(parents=True)
        (self.fixture / "Cargo.toml").write_text(
            '[package]\nname = "tokensave"\nversion = "1.0.0"\nedition = "2021"\n'
        )
        (self.fixture / "build.rs").write_text(
            'fn main() { assert!(!std::path::Path::new('
            '&std::env::var_os("TOKENSAVE_TEST_INSTALL_EXE").unwrap()).exists()); }'
        )
        (source / "main.rs").write_text('fn main() { println!("replacement"); }')
        self.env = os.environ | {"TOKENSAVE_TEST_INSTALL_EXE": str(self.exe)}

    def upgrade(self, *options, fixture=None):
        # PowerShell's -File cannot bind an argument array in Windows PowerShell.
        # Environment variables carry paths without injecting them into code.
        env = self.env | {
            "TOKENSAVE_TEST_HELPER": str(HELPER),
            "TOKENSAVE_TEST_INSTALL": str(self.install),
            "TOKENSAVE_TEST_FIXTURE": str(fixture or self.fixture),
        }
        command = (
            "& $env:TOKENSAVE_TEST_HELPER -InstallRoot $env:TOKENSAVE_TEST_INSTALL "
            "-CargoArguments @('--path', $env:TOKENSAVE_TEST_FIXTURE, '--offline') "
            + " ".join(options)
        )
        return subprocess.run(
            ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command],
            env=env, cwd=self.root, capture_output=True, text=True, timeout=UPGRADE_TIMEOUT,
        )

    def spawn(self, exe, *args, **kwargs):
        child = subprocess.Popen([exe, *args], **kwargs)
        self.addCleanup(child.__exit__, None, None, None)
        self.addCleanup(child.kill)
        return child

    def serve(self, exe, project, env):
        child = self.spawn(
            exe, "serve", "--path", project, env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        child.stdin.write(
            b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":'
            b'{"protocolVersion":"2024-11-05","capabilities":{},'
            b'"clientInfo":{"name":"upgrade-test","version":"1"}}}\n'
        )
        child.stdin.flush()
        response = queue.Queue()
        threading.Thread(target=lambda: response.put(child.stdout.readline()), daemon=True).start()
        self.assertIn(b'"serverInfo"', response.get(timeout=READY_TIMEOUT))
        return child

    def test_graceful_upgrade_targets_only_the_selected_executable(self):
        shutil.copyfile(self.binary, self.exe)
        other = self.root / BINARY
        shutil.copyfile(self.binary, other)
        home = self.root / "home"
        home.mkdir()
        env = self.env | {
            key: str(home)
            for key in (
                "HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME",
                "TOKENSAVE_USER_CONFIG_DIR",
            )
        }
        env |= {"TOKENSAVE_SKIP_AGENT_MAINTENANCE": "1", "TOKENSAVE_STATE_DIR": ""}
        project = self.root / "project"
        project.mkdir()
        result = subprocess.run(
            [self.exe, "init", project, "--no-git-hook"], env=env, cwd=project,
            capture_output=True, text=True, timeout=READY_TIMEOUT,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        sentinel = project / ".tokensave" / "upgrade-test-sentinel"
        sentinel.write_text("preserve project data")
        targets = [self.serve(self.exe, project, env) for _ in range(2)]
        unrelated = self.serve(other, project, env)

        result = self.upgrade()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for target in targets:
            self.assertEqual(target.wait(timeout=READY_TIMEOUT), 0)
        self.assertIsNone(unrelated.poll())
        self.assertEqual(sentinel.read_text(), "preserve project data")
        self.assertEqual(subprocess.check_output([self.exe], text=True).strip(), "replacement")
        self.assertFalse(self.exe.with_name(BACKUP).exists())

    def test_failed_cargo_install_restores_original(self):
        shutil.copyfile(self.binary, self.exe)
        result = self.upgrade(fixture=self.root / "missing")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Cargo install exited", result.stderr)
        self.assertFalse(self.exe.with_name(BACKUP).exists())
        self.assertEqual(self.exe.read_bytes(), self.binary.read_bytes())

    def test_legacy_process_requires_explicit_force(self):
        legacy = Path(os.environ["SystemRoot"]) / "System32" / "ping.exe"
        shutil.copyfile(legacy, self.exe)
        child = self.spawn(self.exe, "-t", "127.0.0.1", stdout=subprocess.DEVNULL)
        result = self.upgrade()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not exit in time", result.stderr)
        self.assertIsNone(child.poll())
        result = self.upgrade("-ForceStop")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        child.wait(timeout=READY_TIMEOUT)
        self.assertEqual(subprocess.check_output([self.exe], text=True).strip(), "replacement")


if __name__ == "__main__":
    unittest.main()
