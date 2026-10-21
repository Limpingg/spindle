#!/usr/bin/env python3
"""Turns a Spotifast tree into Spindle's branding, mechanically.

Spindle tracks Spotifast through "rename, then merge": every Spotifast
release is renamed with this script into a branch whose history is
Spotifast's renamed, so a merge into main sees the same renames on both
sides and conflicts only where Nick's own changes meet Spotifast's. The
rules must therefore stay deterministic: the same input tree always gives
the same output tree. Change them only together with a fresh renamed
base, or merges will fight over every line the rule touches.

Run it in a checked out Spotifast tree: scripts/rename-upstream.py
"""

import os
import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()

# Files and folders that are renamed on disk.
RENAMES = [
    ("src/spotifast.rs", "src/spindle.rs"),
    ("assets/skins/spotifast.wsz", "assets/skins/spindle.wsz"),
    ("contrib/omarchy/spotifast-theme", "contrib/omarchy/spindle-theme"),
    ("contrib/omarchy/spotifast.json.tpl", "contrib/omarchy/spindle.json.tpl"),
    ("packaging/windows/spotifast.iss", "packaging/windows/spindle.iss"),
    ("packaging/windows/spotifast.ico", "packaging/windows/spindle.ico"),
    ("packaging/windows/spotifast-installer.txt", "packaging/windows/spindle-installer.txt"),
    ("packaging/spotifast-portable.txt", "packaging/spindle-portable.txt"),
    ("packaging/macos/spotifast.icns", "packaging/macos/spindle.icns"),
]

# Where the words are replaced. Documentation, the website, Linux packaging,
# the MSI packaging, the examples and the GitHub templates stay Spotifast's:
# they describe the upstream project and nothing ships them from here. The
# translation template keeps its upstream name, which the catalogs refer to.
TEXT_ROOTS = [
    "src",
    "tests",
    "assets/i18n",
    "contrib/omarchy",
    "packaging/macos",
    "packaging/windows/spindle.iss",
    "packaging/windows/spindle-installer.txt",
    "packaging/spindle-portable.txt",
    "Cargo.toml",
    "build.rs",
    ".cargo/config.toml",
]
TEXT_SUFFIXES = {
    ".rs", ".po", ".pot", ".toml", ".plist", ".sh", ".rb", ".iss",
    ".tpl", ".txt", ".json", "",
}

# Replacements that come before the general words, and Spotifast mentions
# that stay as they are. The placeholders keep both out of the way of the
# general replacements, and the bundle identifier is Nick's own.
SPECIAL = [
    ("rocks.spotifast.Spotifast", "com.nickaustin.spindle"),
    ("spotifast.rocks", "spotifast.rocks"),
    ("crmne/spotifast", "crmne/spotifast"),
    ("Spotifast by Carmine Paolino", "Spotifast by Carmine Paolino"),
    ("fork of Spotifast", "fork of Spotifast"),
    ("spotifast-dev", "spotifast-dev"),
    ("spotifast branch", "spotifast branch"),
    ("SpotifastUpdate", "SpotifastUpdate"),
    ("spotifast.pot", "spotifast.pot"),
]

MARK = "\u0000KEEP{}\u0000"


def rename_text(text: str) -> str:
    for index, (before, _) in enumerate(SPECIAL):
        text = text.replace(before, MARK.format(index))
    text = text.replace("Spotifast", "Spindle")
    text = text.replace("spotifast", "spindle")
    text = text.replace("SPOTIFAST", "SPINDLE")
    for index, (_, after) in enumerate(SPECIAL):
        text = text.replace(MARK.format(index), after)
    return text


def rename_lock(path: Path) -> None:
    """Renames the package in Cargo.lock and keeps cargo's sorted order,
    so the next build does not rewrite the file."""
    text = path.read_text(encoding="utf-8")
    head, _, rest = text.partition("[[package]]")
    blocks = ["[[package]]" + block for block in rest.split("[[package]]")]
    renamed = []
    for block in blocks:
        if 'name = "spotifast"' in block:
            block = block.replace('name = "spotifast"', 'name = "spindle"', 1)
        renamed.append(block.rstrip("\n") + "\n")

    def key(block: str):
        name = re.search(r'^name = "([^"]+)"', block, re.M)
        version = re.search(r'^version = "([^"]+)"', block, re.M)
        # semver order, as cargo sorts: numbers, then a pre-release sorts
        # before its release, and build metadata does not count.
        text = (version.group(1) if version else "").split("+", 1)[0]
        release, _, prerelease = text.partition("-")
        numbers = tuple(int(part) if part.isdigit() else 0 for part in release.split("."))
        pre = tuple(
            (0, int(part), "") if part.isdigit() else (1, 0, part)
            for part in prerelease.split(".")
        ) if prerelease else ()
        return (name.group(1) if name else "", numbers, 0 if prerelease else 1, pre)

    renamed.sort(key=key)
    path.write_text(head + "\n".join(renamed), encoding="utf-8")


# Spotifast's GitHub workflows (CI, packaging, releases, the website) would
# run in Spindle's repository and fail; they are dropped, and Spindle's own
# live beside them under names that start with "spindle-".
DROPPED = [".github/workflows"]


def main() -> None:
    os.chdir(ROOT)
    for dropped in DROPPED:
        path = Path(dropped)
        if path.is_dir():
            for file in sorted(path.iterdir()):
                if file.is_file() and not file.name.startswith("spindle-"):
                    file.unlink()
    for source, target in RENAMES:
        if Path(source).exists() and not Path(target).exists():
            Path(target).parent.mkdir(parents=True, exist_ok=True)
            os.replace(source, target)
    files = []
    for root in TEXT_ROOTS:
        path = Path(root)
        if path.is_file():
            files.append(path)
        elif path.is_dir():
            files.extend(p for p in path.rglob("*") if p.is_file())
    changed = 0
    for path in files:
        if path.suffix.lower() not in TEXT_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        renamed = rename_text(text)
        if renamed != text:
            path.write_text(renamed, encoding="utf-8")
            changed += 1
    if Path("Cargo.lock").is_file():
        rename_lock(Path("Cargo.lock"))
        changed += 1
    print(f"renamed {changed} files under {ROOT}")


if __name__ == "__main__":
    main()
