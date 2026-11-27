import json
import os
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORK = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target") / "surface-test"
REPO = WORK / "repo"
LIB = "crates/openworks-core/src/lib.rs"
WEB = "web/src/index.ts"
EVENT = "contract/events/job-created.json"
RUST_API = "pub struct Job { pub id: u64 }\npub fn open(id: u64) -> Job { Job { id } }\n"
WEB_API = "export function mount(el: Element, props: { title: string }): void {\n  el.textContent = props.title;\n}\n"
EVENT_SHAPE = {"type": "object", "properties": {"job_id": {"type": "string"}}}


def git(*args):
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=REPO, check=True, capture_output=True)


def write(path, text):
    (REPO / path).parent.mkdir(parents=True, exist_ok=True)
    (REPO / path).write_text(text)


def set_version(cargo, web):
    manifest = REPO / "Cargo.toml"
    manifest.write_text(re.sub(r'^version = "[^"]+"', f'version = "{cargo}"', manifest.read_text(), count=1, flags=re.M))
    package = json.loads((REPO / "web/package.json").read_text())
    package["version"] = web
    (REPO / "web/package.json").write_text(json.dumps(package))


def surface(command="check"):
    env = {**os.environ, "CARGO_TARGET_DIR": str(WORK / "target")}
    done = subprocess.run(["python3", ".gatekeeper/surface.py", command], cwd=REPO, env=env, capture_output=True, text=True)
    return done.returncode, done.stdout + done.stderr


class Surface(unittest.TestCase):
    def setUp(self):
        shutil.rmtree(REPO, ignore_errors=True)
        files = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], cwd=ROOT, check=True, capture_output=True, text=True)
        for name in files.stdout.split():
            if (ROOT / name).is_file() or (ROOT / name).is_symlink():
                (REPO / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / name, REPO / name, follow_symlinks=False)
        (REPO / "node_modules").symlink_to(ROOT / "node_modules")
        self.core = (REPO / LIB).read_text()
        write(LIB, self.core + RUST_API)
        write(WEB, WEB_API)
        write(EVENT, json.dumps(EVENT_SHAPE))
        git("init", "-q")
        (REPO / ".git/info/exclude").write_text("/node_modules\n")
        git("add", "-A")
        git("commit", "-qm", "release")
        git("tag", "v0.1.0")

    def test_unchanged_and_added_surface_passes(self):
        write(LIB, self.core + RUST_API + "pub fn close(job: Job) -> u64 { job.id }\n")
        self.assertEqual(surface(), (0, "public surface keeps everything v0.1.0 has\n"))

    def test_breaking_changes_fail_until_the_version_breaks(self):
        write(LIB, self.core + "pub struct Job { pub id: u32 }\n")
        write(WEB, WEB_API.replace("title", "heading"))
        write(EVENT, json.dumps({"type": "object", "properties": {}}))
        code, out = surface()
        self.assertEqual(code, 1, out)
        self.assertIn("rust openworks_core::open ", out)
        self.assertIn("rust openworks_core::Job::id ", out)
        self.assertIn("web src/index.d.ts title: string;", out)
        self.assertIn('contract contract/events/job-created.json .properties.job_id.type="string"', out)
        set_version("0.1.1", "0.1.1")
        self.assertEqual(surface()[0], 1)
        set_version("0.2.0", "0.2.0")
        self.assertEqual(surface(), (0, "4 breaking changes since v0.1.0, released as 0.2.0\n"))

    def test_one_version_for_crates_and_web(self):
        self.assertEqual(surface("version"), (0, "0.1.0\n"))
        set_version("0.2.0", "0.1.0")
        code, out = surface("version")
        self.assertEqual(code, 1)
        self.assertIn("web/package.json is 0.1.0, Cargo.toml is 0.2.0", out)


if __name__ == "__main__":
    unittest.main()
