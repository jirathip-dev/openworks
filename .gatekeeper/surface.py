import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

DROPPED = {"items", "fields", "variants", "tuple", "impls", "implementations", "id", "span"}


def run(args, cwd, env=None):
    done = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"{' '.join(args)} failed:\n{done.stderr}")
    return done.stdout


def target_dir(root):
    return Path(os.environ.get("CARGO_TARGET_DIR") or root / "target")


def one_version(root):
    cargo = (root / "Cargo.toml").read_text()
    found = re.search(r'^\[workspace\.package\][^\[]*?^version = "([^"]+)"', cargo, re.M | re.S)
    if not found:
        sys.exit("Cargo.toml: [workspace.package] has no version")
    version = found.group(1)
    for manifest in sorted(root.glob("crates/*/Cargo.toml")):
        if not re.search(r"^version\.workspace = true$", manifest.read_text(), re.M):
            sys.exit(f"{manifest.relative_to(root)}: take the one version: version.workspace = true")
    web = json.loads((root / "web/package.json").read_text())["version"]
    if web != version:
        sys.exit(f"web/package.json is {web}, Cargo.toml is {version}: one version for all")
    return version


def semver(text):
    found = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", text)
    return tuple(int(part) for part in found.groups()) if found else None


def canon(value):
    if isinstance(value, dict):
        return {key: canon(inner) for key, inner in value.items() if key not in DROPPED}
    if isinstance(value, list):
        return [canon(inner) for inner in value]
    return value


def rust_children(item):
    kind, inner = next(iter(item["inner"].items()))
    if kind == "module":
        return [(child, {"public"}) for child in inner["items"]]
    if kind in ("struct", "union"):
        shape = inner["kind"] if kind == "struct" else inner
        ids = shape.get("tuple") or shape.get("plain", {}).get("fields") or shape.get("fields") or []
        return [(child, {"public"}) for child in ids if child is not None]
    if kind == "enum":
        return [(child, {"default"}) for child in inner["variants"]]
    if kind == "variant":
        shape = inner["kind"]
        ids = shape.get("tuple") or shape.get("struct", {}).get("fields") or [] if isinstance(shape, dict) else []
        return [(child, {"default"}) for child in ids if child is not None]
    if kind == "trait":
        return [(child, {"default"}) for child in inner["items"]]
    return []


def rust_lines(doc):
    index, lines = doc["index"], set()

    def visit(item_id, prefix, allowed):
        item = index.get(str(item_id))
        if item is None or item["visibility"] not in allowed:
            return
        path = f"{prefix}::{item['name']}" if prefix else item["name"]
        body = canon(item["inner"])
        lines.add(f"rust {path} {json.dumps(body, sort_keys=True, separators=(',', ':'))}")
        for child, child_allowed in rust_children(item):
            visit(child, path, child_allowed)
        kind, inner = next(iter(item["inner"].items()))
        for impl_id in inner.get("impls", []) if isinstance(inner, dict) else []:
            impl = index[str(impl_id)]["inner"]["impl"]
            if impl.get("blanket_impl") is not None:
                continue
            trait = impl["trait"]
            if trait is not None:
                lines.add(f"rust {path} impl {json.dumps(canon(trait), sort_keys=True)}")
                continue
            for method in impl["items"]:
                visit(method, path, {"public"})

    visit(doc["root"], "", {"public"})
    return lines


def rust_surface(tree, root):
    lines = set()
    env = {**os.environ, "RUSTC_BOOTSTRAP": "1", "CARGO_TARGET_DIR": str(target_dir(root))}
    for lib in sorted(tree.glob("crates/*/src/lib.rs")):
        crate = lib.parent.parent.name
        args = ["cargo", "rustdoc", "-q", "-p", crate, "--lib", "--manifest-path", str(tree / "Cargo.toml")]
        run(args + ["--", "-Z", "unstable-options", "--output-format", "json"], tree, env)
        doc_file = target_dir(root) / "doc" / f"{crate.replace('-', '_')}.json"
        lines |= rust_lines(json.loads(doc_file.read_text()))
    return lines


def web_surface(tree, root):
    tsc = root / "node_modules/.bin/tsc"
    if not tsc.exists():
        run(["npm", "ci", "--no-audit", "--no-fund"], root)
    out = target_dir(root) / "surface" / "dts" / ("head" if tree == root else tree.name)
    shutil.rmtree(out, ignore_errors=True)
    flags = ["--declaration", "--emitDeclarationOnly", "--noEmit", "false", "--outDir", str(out)]
    subprocess.run([str(tsc), "-p", str(tree / "web"), *flags], cwd=tree, check=False, capture_output=True)
    lines = set()
    for dts in sorted(out.rglob("*.d.ts")):
        for line in dts.read_text().splitlines():
            if line.strip():
                lines.add(f"web {dts.relative_to(out)} {line.strip()}")
    return lines


def flatten(value, path):
    if isinstance(value, dict):
        for key, inner in value.items():
            yield from flatten(inner, f"{path}.{key}")
    elif isinstance(value, list):
        for inner in value:
            yield from flatten(inner, f"{path}[]")
    else:
        yield f"{path}={json.dumps(value)}"


def contract_surface(tree):
    lines = set()
    for doc in sorted((tree / "contract").rglob("*.json")):
        name = doc.relative_to(tree).as_posix()
        lines |= {f"contract {name} {line}" for line in flatten(json.loads(doc.read_text()), "")}
    return lines


def surface(tree, root):
    return rust_surface(tree, root) | web_surface(tree, root) | contract_surface(tree)


def last_release(root):
    tags = [(semver(tag), tag) for tag in run(["git", "tag", "--list", "v*"], root).split()]
    tags = [entry for entry in tags if entry[0]]
    return max(tags) if tags else None


def released_tree(root, tag):
    tree = target_dir(root) / "surface" / tag
    shutil.rmtree(tree, ignore_errors=True)
    archive = target_dir(root) / "surface" / f"{tag}.tar"
    archive.parent.mkdir(parents=True, exist_ok=True)
    run(["git", "archive", "--output", str(archive), tag], root)
    with tarfile.open(archive) as source:
        source.extractall(tree)
    return tree


def allows_break(old, new):
    if old[0] == 0:
        return new[:2] > old[:2]
    return new[0] > old[0]


def check(root):
    version = semver(one_version(root))
    release = last_release(root)
    if release is None:
        print(f"no release tag yet: {'.'.join(map(str, version))} is the first")
        return 0
    old_version, tag = release
    removed = sorted(surface(released_tree(root, tag), root) - surface(root, root))
    if not removed:
        print(f"public surface keeps everything {tag} has")
        return 0
    if allows_break(old_version, version):
        print(f"{len(removed)} breaking changes since {tag}, released as {'.'.join(map(str, version))}")
        return 0
    print(f"{len(removed)} items of {tag}'s public surface are gone or changed:")
    for line in removed[:50]:
        print(f"  {line}")
    print("bump the version in Cargo.toml and web/package.json to the next breaking release")
    return 1


def main():
    root = Path(__file__).resolve().parent.parent
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "version":
        print(one_version(root))
        return 0
    if command == "check":
        return check(root)
    sys.exit("usage: surface.py version|check")


if __name__ == "__main__":
    sys.exit(main())
