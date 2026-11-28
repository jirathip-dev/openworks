import json
import os
import re
import shutil
import subprocess
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORK = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target") / "publish-test"
REPO = WORK / "repo"
DB = "crates/openworks-db/src/lib.rs"
APP = "crates/openworks/src/lib.rs"


def gate_run():
    text = (ROOT / ".gatekeeper/gates/publish.toml").read_text()
    return json.loads(re.search(r'^run = (".*")$', text, re.M).group(1))


def publish():
    env = {**os.environ, "CARGO_TARGET_DIR": str(WORK / "target")}
    done = subprocess.run(["sh", "-c", gate_run()], cwd=REPO, env=env, capture_output=True, text=True)
    return done.returncode, done.stdout + done.stderr


class Publish(unittest.TestCase):
    def setUp(self):
        shutil.rmtree(REPO, ignore_errors=True)
        files = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], cwd=ROOT, check=True, capture_output=True, text=True)
        for name in files.stdout.split():
            if (ROOT / name).is_file() or (ROOT / name).is_symlink():
                (REPO / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / name, REPO / name, follow_symlinks=False)

    def test_second_run_sees_a_module_added_after_the_first(self):
        code, out = publish()
        self.assertEqual(code, 0, out)
        module = f"probe_{time.time_ns()}"
        (REPO / DB).write_text((REPO / DB).read_text() + f"\n/// Probe.\npub mod {module} {{\n    /// Probe.\n    pub const ID: u8 = 1;\n}}\n")
        (REPO / APP).write_text((REPO / APP).read_text() + f"\npub use openworks_db::{module}::ID as PROBE;\n")
        code, out = publish()
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
