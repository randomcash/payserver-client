# payserver-client

The merchant-facing frontend for the random.cash payservers. Leptos compiled to
WebAssembly, served by nginx, talking to whichever payserver is configured at
runtime.

**One client, any chain.** It must never depend on a payserver repository. The
API contract lives in `payserver-commons/api-types`, which both sides compile
against — that is the seam, and it is the reason this repo is separate.

## This repository is public

No session URLs in commits or PR bodies, no secrets. Also no ticket ids or
tracker links in source files — no `RCS-123`, no `linear.app` URL, anywhere in
code or comments. A ticket id here leaks the shape of unreleased work, and an
outside reader cannot open it anyway. Write the reason the code exists, not a
pointer to where someone once explained it; the ticket id belongs in the
commit message and PR title. `scripts/check-no-ticket-refs.sh` enforces this
in CI; `CLAUDE.md`, `AGENTS.md` and `docs/` are exempt.

## It is compiled, which changes what a plugin or theme can be

There is no runtime template layer. Anything that renders is Rust compiled into
the bundle, so "let a plugin provide a page" cannot mean "load Rust into the
client". Keep that in mind before promising extensibility that the architecture
does not support.

## Every class the markup emits must have a rule

`scripts/check-classes-styled.py` enforces this in CI, and it exists because the
alternative shipped: a rename moved the markup to new class names, the rules
stayed behind under the old ones, and **104 buttons rendered unstyled on
testnet**. Nothing caught it —

- the layout suite asserts geometry, not colour or size, so an unstyled button of
  the same shape passes
- e2e locates elements by class, and every class was present, just carrying no
  rules
- comparing the stylesheet's selector set before and after showed nothing lost,
  which was true and irrelevant: the rules still existed, under names nothing
  renders any more

*"No selector was lost"* and *"every class the markup emits has a rule"* sound
like the same check. Only the second one catches this.

## Renaming a class is a cross-repo change

`ethpayserver`'s e2e suite locates elements by class, and tests against the
client image pinned in its `ops/client-image.pin`. So a rename here needs a
matching change there — **the selectors and the pin bump in one commit**, or
whoever bumps the pin next inherits failures they did not cause.

Check before renaming:

```bash
git grep -n "\.the-class" ../ethpayserver/e2e/
```

## Editing CSS with a script

Tempting, and it has gone wrong twice in one session. Comments in `styles.css`
mention class names in prose, so a regex that matches selectors will happily
splice a comment in half — producing a mangled rule that the browser discards
silently. One such mangle broke the rule pinning a `<select>` to the same height
as an `<input>`.

If you must, be comment-aware, and verify structurally afterwards: every selector
that existed before still exists, comment markers balance, braces balance.

## The gate

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 scripts/check-classes-styled.py
```

CI additionally runs a Playwright layout suite and builds the wasm bundle.

## Commons is pinned by revision

`payserver-commons` is pinned by `rev`, so a change there is invisible here until
the pin moves. `scripts/commons.sh link` points the build at a local checkout for
development; **`scripts/commons.sh unlink` before verifying or committing**, or
you are testing your working copy rather than the pin. The lock file must never
be committed while linked.

## Conventions that bite

- **`git grep`, not bare `grep`** — `grep` is `ugrep` here and honours
  `.gitignore`.
- **Never `git add -A`** — check for strays and add paths explicitly.
- **A test that cannot fail is worse than no test.** Break the thing it covers
  and confirm it goes red.
