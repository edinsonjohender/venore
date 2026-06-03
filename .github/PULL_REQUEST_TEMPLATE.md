<!--
Thanks for contributing to Venore. Read CONTRIBUTING.md first.

Hard rules — this PR will be closed automatically if any of these fail:

1. There is a linked issue labeled `approved` by a maintainer.
2. The diff is under ~300 lines (excluding lockfiles/snapshots), or the
   issue thread explicitly approved a larger size.
3. The PR addresses exactly one concern. No bundled cleanups, no mixed
   logic + formatting changes, no "while I was in here".
4. `cargo test`, `cargo clippy --all-targets -- -D warnings`, and (for
   UI changes) `pnpm typecheck` all pass locally.
5. If you used AI tools, they are disclosed below and every commit has
   an `Assisted-by:` trailer. You can explain every line.
-->

## Summary

<!-- One or two sentences. What changes, and why? -->

## Linked issue

Fixes #<!-- issue number — must be labeled `approved` by a maintainer -->

## Scope confirmation

- [ ] The linked issue is labeled `approved`.
- [ ] This PR's diff stays within what the issue authorized.
- [ ] Diff is under ~300 lines, OR the issue thread explicitly approved a larger size.
- [ ] One concern only — no bundled refactors, no opportunistic cleanups.

## Type of change

- [ ] feat — new user-facing feature
- [ ] fix — bug fix
- [ ] refactor — internal change with no behavior change
- [ ] docs — documentation only
- [ ] chore — tooling, build, deps
- [ ] test — adding or fixing tests

## How was this tested?

<!--
Concrete checks you ran. For UI changes, describe what you exercised in the
running app — not just "types pass".
-->

## AI tool disclosure

<!--
Disclosure is MANDATORY if you used AI assistance of any kind (Claude,
Copilot, Cursor, ChatGPT, Codex, agents, etc.). Failing to disclose is
grounds for immediate close and permanent block.

You must be able to explain every line. "The AI wrote it" is not an answer.
-->

- [ ] I used an AI assistant on this PR
- Tools used:
- [ ] I added `Assisted-by: <tool>` trailers to every commit on this branch
- [ ] I have read every line and can explain what each does and why

## Author checklist

- [ ] I read CONTRIBUTING.md before opening this PR.
- [ ] I followed the existing code patterns and conventions.
- [ ] I did not add unnecessary abstractions, layers, or dependencies.
- [ ] I am the author of every line, or have properly attributed external sources.
- [ ] I will respond to review feedback within 7 days.
