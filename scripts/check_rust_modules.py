#!/usr/bin/env python3
"""Fail when a Rust source file under a crate's src directory is unreachable."""

from pathlib import Path
import re
import sys


MODULE_RE = re.compile(r"^\s*(?:pub\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
PATH_RE = re.compile(r"^\s*#\s*\[path\s*=\s*[\"']([^\"']+)[\"']\s*\]")


def reachable(crate_src: Path) -> set[Path]:
    roots = [path for path in (crate_src / "lib.rs", crate_src / "main.rs") if path.exists()]
    seen: set[Path] = set()
    pending = list(roots)

    while pending:
        current = pending.pop().resolve()
        if current in seen:
            continue
        seen.add(current)
        pending_path: Path | None = None
        for line in current.read_text(encoding="utf-8").splitlines():
            if path_match := PATH_RE.match(line):
                pending_path = (current.parent / path_match.group(1)).resolve()
                continue
            if module_match := MODULE_RE.match(line):
                if pending_path is not None:
                    target = pending_path
                    pending_path = None
                else:
                    module = module_match.group(1)
                    target = current.parent / module / "mod.rs"
                    if not target.exists():
                        target = current.parent / f"{module}.rs"
                if target.exists():
                    pending.append(target)
    return {path for path in seen if path.suffix == ".rs"}


def main() -> int:
    orphaned: list[Path] = []
    for crate_src in sorted(Path("contracts").glob("*/src")):
        files = {path.resolve() for path in crate_src.rglob("*.rs")}
        missing = files - reachable(crate_src)
        orphaned.extend(sorted(path.relative_to(Path.cwd()) for path in missing))

    if orphaned:
        for path in orphaned:
            print(f"Unreachable Rust source: {path}")
        return 1
    print("Rust module reachability check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())