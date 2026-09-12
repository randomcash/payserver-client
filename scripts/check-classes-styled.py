#!/usr/bin/env python3
"""Every `ps-` class the markup emits must have a rule somewhere.

This exists because a rename shipped without it. `btn-sm` became `ps-btn-sm`
across 69 call sites, the rule stayed behind as `.btn-sm`, and 104 buttons lost
their styling on testnet. Nothing caught it:

* the layout suite passes, because it asserts geometry, not colour or size
* the e2e suite passes, because it locates elements by class and the classes are
  all present - just unstyled
* checking that the stylesheet's selector set was preserved passes, because it
  was: the rules still exist, under names nothing renders any more

That last one is the trap. "No selector was lost" and "every emitted class is
styled" sound like the same check and are not. This is the second one.

Scoped to `ps-` classes on purpose: those are the shared ui-kit vocabulary, where
a name can be renamed on one side of the repository boundary and not the other.
Client-only classes are checked by the same reasoning but produce far more noise
from prose that merely looks like a class name.

Usage: scripts/check-classes-styled.py   (exits non-zero on an unstyled class)
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def emitted_classes() -> dict[str, int]:
    """Every `ps-` token appearing in a `class=` attribute, with a use count."""
    files = subprocess.run(
        ["git", "grep", "-l", "class=", "--", "src/"],
        cwd=REPO, capture_output=True, text=True, check=False,
    ).stdout.split()
    found: dict[str, int] = {}
    for f in files:
        text = (REPO / f).read_text()
        # Leptos writes class as a literal, a `move ||` closure or an `if`
        # expression; all three put the class list in a double-quoted string.
        # Every double-quoted string, not just the first one after `class=`.
        # A conditional class is written `if cond { "a" } else { "b" }`, and a
        # non-greedy match after `class=` reads only the first branch - which is
        # how `ps-btn-warning` hid from the first version of this script.
        # The `ps-` prefix is distinctive enough that prose does not match.
        for m in re.finditer(r'"([^"\n]*)"', text):
            for tok in m.group(1).split():
                if tok.startswith("ps-"):
                    found[tok] = found.get(tok, 0) + 1
    return found


def uikit_css() -> str:
    """ui-kit's own stylesheet, located through cargo rather than a guessed path.

    It lives in a git dependency, so its on-disk location is a cargo checkout
    hash, not a sibling directory. `cargo metadata` is the only reliable way to
    find it, and it works in CI where the dependency is fetched but no commons
    checkout exists.
    """
    import json
    try:
        meta = json.loads(subprocess.run(
            ["cargo", "metadata", "--format-version", "1"],
            cwd=REPO, capture_output=True, text=True, check=True,
        ).stdout)
    except (subprocess.CalledProcessError, FileNotFoundError, json.JSONDecodeError):
        return ""
    for pkg in meta.get("packages", []):
        if pkg.get("name") == "ui-kit":
            css = Path(pkg["manifest_path"]).parent / "styles" / "ui-kit.css"
            if css.exists():
                return css.read_text()
    return ""


def styled_classes() -> set[str]:
    """Classes with at least one rule, across both stylesheets."""
    css = (REPO / "styles.css").read_text()
    extra = uikit_css()
    if extra:
        css += extra
    else:
        print("warning: ui-kit's stylesheet could not be located through cargo; "
              "classes styled only by ui-kit may be reported as unstyled",
              file=sys.stderr)
    # Comments mention class names in prose. Counting those as rules is exactly
    # how a missing rule would hide.
    css = re.sub(r"/\*.*?\*/", "", css, flags=re.S)
    return set(re.findall(r"\.([a-zA-Z][\w-]*)", css))


def main() -> int:
    emitted = emitted_classes()
    styled = styled_classes()
    missing = {c: n for c, n in emitted.items() if c not in styled}

    if not missing:
        print(f"all {len(emitted)} ps- classes emitted by the markup have a rule")
        return 0

    print("unstyled ps- classes in rendered markup:\n", file=sys.stderr)
    for c, n in sorted(missing.items(), key=lambda kv: -kv[1]):
        print(f"  {c:<28} {n:>4} use(s)", file=sys.stderr)
    print(
        "\nEach of these renders with no styling. Either add a rule, or point the "
        "markup at a class that has one.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
