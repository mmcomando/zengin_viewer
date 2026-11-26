"""Dump Rust functions and type names grouped by module path.

Scanner based, not a real parser: it strips comments and string/char literals,
tracks `mod name { ... }` nesting, and records `fn`, `struct`, `enum`, `trait`,
`union` and `type` items at any depth. Good enough for an overview, not for
refactoring.

Usage:
    python3 rust_symbol_dump.py [ROOT ...] [--kind fn|type|all]
        [--include-tests] [--grep SUBSTR] [--files] [--no-lines]
        [--skip-crate NAME ...]

    ROOT defaults to the current directory, or its nearest Cargo.toml ancestor.
    --kind fn         functions only
    --kind type       type items only
    --include-tests   keep items from `mod tests` / `mod test` blocks
    --grep SUBSTR     only items whose name contains SUBSTR (case-insensitive)
    --files           append the source file next to each module header
    --no-lines        omit the source line numbers (default: shown)
    --skip-crate NAME skip a crate; repeatable, matches the first module segment
"""

import bisect
import os
import re
import sys
from pathlib import Path

SKIP_DIRS = {"target", "node_modules"}
TEST_MODS = {"tests", "test"}
TYPE_KINDS = {"struct", "enum", "trait", "union", "type"}

IDENT_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
RAW_STRING_RE = re.compile(r'b?r(#*)"')
CARGO_NAME_RE = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.MULTILINE)


def strip_noise(text: str) -> str:
    """Blank out comments and string/char literals, keeping newlines."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif c == "/" and i + 1 < n and text[i + 1] == "*":
            depth = 1
            j = i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            out.append(re.sub(r"[^\n]", " ", text[i:j]))
            i = j
        elif c in "rb" and (m := RAW_STRING_RE.match(text, i)):
            term = '"' + m.group(1)
            j = text.find(term, m.end())
            j = n if j < 0 else j + len(term)
            out.append(" " * (j - i))
            i = j
        elif c == '"' or (c == "b" and text.startswith('b"', i)):
            j = i + 1 + (1 if c == "b" else 0)
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(" " * (j - i))
            i = j
        elif c == "'":
            if i + 1 < n and text[i + 1] == "\\":
                j = i + 2
                while j < n and text[j] != "'":
                    j += 1
                j = min(j + 1, n)
                out.append(" " * (j - i))
                i = j
            elif i + 2 < n and text[i + 2] == "'":
                out.append(" " * 3)
                i += 3
            else:
                out.append("'")
                i += 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


def skip_ws(text: str, i: int) -> int:
    n = len(text)
    while i < n and text[i].isspace():
        i += 1
    return i


def scan_items(text: str):
    """Yield (mod_stack, kind, name, line) for every item in stripped text."""
    newlines = [i for i, c in enumerate(text) if c == "\n"]
    depth = 0
    mods = []
    pending_mod = None
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == "{":
            depth += 1
            if pending_mod is not None:
                mods.append((depth, pending_mod))
                pending_mod = None
            i += 1
        elif c == "}":
            if mods and mods[-1][0] == depth:
                mods.pop()
            depth = max(depth - 1, 0)
            i += 1
        elif c.isalpha() or c == "_":
            m = IDENT_RE.match(text, i)
            word = m.group(0)
            if word == "mod":
                m2 = IDENT_RE.match(text, skip_ws(text, m.end()))
                if not m2:
                    i = m.end()
                    continue
                if skip_ws(text, m2.end()) < n and text[skip_ws(text, m2.end())] == "{":
                    pending_mod = m2.group(0)
                i = m2.end()
            elif word == "fn" or word in TYPE_KINDS:
                m2 = IDENT_RE.match(text, skip_ws(text, m.end()))
                if m2:
                    line = bisect.bisect_right(newlines, m.start()) + 1
                    yield [name for _, name in mods], word, m2.group(0), line
                    i = m2.end()
                else:
                    i = m.end()
            else:
                i = m.end()
        else:
            i += 1


_crate_name_cache = {}


def crate_name_for_dir(base: Path) -> str:
    key = str(base.resolve())
    if key not in _crate_name_cache:
        name = base.name
        manifest = base / "Cargo.toml"
        if manifest.is_file():
            m = CARGO_NAME_RE.search(manifest.read_text(errors="ignore"))
            if m:
                name = m.group(1)
        _crate_name_cache[key] = name.replace("-", "_")
    return _crate_name_cache[key]


def module_path_for_file(path: Path) -> str:
    base = next(
        (d for d in (path.parent, *path.parent.parents) if (d / "Cargo.toml").is_file()),
        None,
    )
    if base is not None:
        parts = list(path.relative_to(base).with_suffix("").parts)
        crate = crate_name_for_dir(base)
        if parts and parts[0] == "src":
            parts = parts[1:]
    else:
        parts = [p for p in path.with_suffix("").parts if p not in ("/", "")]
        crate = None

    if parts and parts[-1] in ("lib", "main", "mod"):
        parts = parts[:-1]
    segs = ([crate] if crate else []) + [
        re.sub(r"[^A-Za-z0-9_]", "_", s) for s in parts
    ]
    return "::".join(s for s in segs if s)


def iter_rs_files(roots):
    for root in roots:
        if root.is_file() and root.suffix == ".rs":
            yield root
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS and not d.startswith("."))
            for fn in sorted(filenames):
                if fn.endswith(".rs"):
                    yield Path(dirpath) / fn


def is_test_file(path: Path) -> bool:
    return any(p in TEST_MODS for p in path.with_suffix("").parts)


def parse_args(argv):
    roots = []
    kind = "all"
    include_tests = False
    grep = None
    show_files = False
    no_lines = False
    skip_crates = set()
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--kind" and i + 1 < len(argv):
            kind = argv[i + 1]
            i += 2
        elif a == "--include-tests":
            include_tests = True
            i += 1
        elif a == "--grep" and i + 1 < len(argv):
            grep = argv[i + 1].lower()
            i += 2
        elif a == "--files":
            show_files = True
            i += 1
        elif a == "--no-lines":
            no_lines = True
            i += 1
        elif a == "--skip-crate" and i + 1 < len(argv):
            skip_crates.add(argv[i + 1])
            i += 2
        elif a in ("-h", "--help"):
            print(__doc__)
            sys.exit(0)
        else:
            roots.append(Path(a))
            i += 1
    if not roots:
        here = Path.cwd().resolve()
        root = next((p for p in (here, *here.parents) if (p / "Cargo.toml").is_file()), here)
        roots = [root]
    return roots, kind, include_tests, grep, show_files, no_lines, skip_crates


def main() -> None:
    roots, kind, include_tests, grep, show_files, no_lines, skip_crates = parse_args(sys.argv[1:])
    want_fn = kind in ("all", "fn")
    want_type = kind in ("all", "type")
    if not (want_fn or want_type):
        sys.exit(f"unknown --kind {kind!r} (expected fn, type or all)")

    modules = {}
    for path in iter_rs_files(roots):
        if not include_tests and is_test_file(path):
            continue
        text = path.read_text(errors="ignore")
        file_mod = module_path_for_file(path)
        if file_mod.split("::")[0] in skip_crates:
            continue
        for mod_stack, word, name, line in scan_items(strip_noise(text)):
            if not include_tests and any(m in TEST_MODS for m in mod_stack):
                continue
            if grep and grep not in name.lower():
                continue
            if word == "fn" and not want_fn:
                continue
            if word != "fn" and not want_type:
                continue
            mod = file_mod
            if mod_stack:
                mod = "::".join([mod, *mod_stack])
            entry = modules.setdefault(mod, {"fn": {}, "type": {}, "file": path})
            bucket = entry["fn" if word == "fn" else "type"]
            key = name if word == "fn" else (word, name)
            if key not in bucket or line < bucket[key]:
                bucket[key] = line

    total_fn = total_type = 0
    for mod in sorted(modules):
        data = modules[mod]
        items = [(line, "fn", name) for name, line in data["fn"].items()]
        items += [(line, word, name) for (word, name), line in data["type"].items()]
        if not items:
            continue
        items.sort(key=lambda t: (t[0], t[2]))
        header = mod
        if show_files:
            header += f"  [{os.path.relpath(data['file'], Path.cwd())}]"
        print(header)
        width = len(str(max(line for line, _, _ in items)))
        for line, word, name in items:
            if no_lines:
                print(f"  {word} {name}")
            else:
                print(f"  {line:>{width}}: {word} {name}")
        print()
        total_fn += len(data["fn"])
        total_type += len(data["type"])

    print(f"{len(modules)} modules, {total_fn} functions, {total_type} type items")


if __name__ == "__main__":
    main()
