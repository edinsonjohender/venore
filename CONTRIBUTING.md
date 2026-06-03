# Contributing to Venore

Thank you for your interest in contributing to Venore. Read this document fully before opening anything.

> **Maintainer reality check:** this project has one maintainer with limited review bandwidth. Recent OSS history shows AI-generated PRs flooding repositories faster than humans can review them. The rules below exist to make sure every contribution is small, scoped, and worth a maintainer's attention. **Drive-by contributions are not welcome here. Pre-coordinated, focused contributions are.**

---

## Golden Rule: Issue First, Always

**Do not open a pull request without a prior approved issue.**

The flow is non-negotiable:

1. **Open an issue** describing the bug, problem, or feature.
2. **Wait for the maintainer to engage.** Discussion happens in the issue thread — that is where the design, scope, and acceptance criteria get pinned down.
3. **Wait for the issue to be labeled `approved`** by a maintainer. This is the explicit go-ahead to start coding.
4. **Only then** open a PR. Reference the approved issue with `Fixes #N` in the PR body.

PRs without an approved issue **will be closed without review**. No exceptions, even for "obvious" fixes. The issue thread is how the maintainer confirms the contribution is wanted and scoped correctly before anyone writes code.

### Why this exists

If your idea is good, a 5-minute issue gets it approved. If the maintainer disagrees, the issue closes before anyone wasted hours on a PR. Either way, the issue is cheaper than the PR.

---

## Pull Request Size Rules

PRs must be **small and focused**. Specifically:

- **One PR = one approved issue.** No bundling unrelated changes.
- **Target: under ~300 lines of diff** (excluding generated files, lockfiles, snapshots).
- **If your change is larger**, the issue discussion has to explicitly approve the size, OR you split it into multiple PRs each tied to its own issue.
- **No rewrites, no broad refactors, no "while I was in here" cleanups.** Stay inside the scope the issue defined.
- **No mixed concerns.** Logic changes and formatting changes go in separate PRs. New features and refactors go in separate PRs.

A PR that touches 40 files because you ran a tool over the codebase gets closed.

---

## AI Policy (Hard Rules)

Venore itself is an AI-powered tool — we are not anti-AI. We are anti-**slop**.

### Mandatory

- **Disclosure.** If you used Copilot, Claude, Cursor, ChatGPT, Codex, or any agent: state it in the PR description and add `Assisted-by: <tool>` to every commit message.
- **Author understanding.** You must be able to explain every line of every file in your PR — what it does, why it's there, how it interacts with the rest of the codebase. The maintainer will ask. "The AI wrote it" is not an answer; it is grounds for immediate close.
- **Human review before submit.** A human must read, understand, and approve every change before pushing. If your PR description sounds like LLM marketing copy, the PR will be closed.

### Forbidden

- **Autonomous agent PRs.** Background agents that open PRs unattended are blocked. A real human must drive the contribution end-to-end.
- **Bulk AI PRs.** Submitting multiple PRs in a short window that look generated (similar structure, generic descriptions, sprawling diffs) is grounds for permanent block.
- **Unsolicited refactors / rewrites / "cleanups".** Even if the AI suggests them, do not submit them. They are noise.
- **Typo / doc / dependency-bump PRs that bypass the issue rule.** Open an issue and confirm it is welcome before sending the PR.

### Auto-close conditions

Any of the following closes the PR immediately, no discussion:

- No linked approved issue
- Diff exceeds ~300 lines without prior issue-level approval of the size
- Multiple unrelated concerns in the same PR
- Commit messages or PR description read like raw LLM output
- Author cannot answer basic questions about their own changes
- Touches files the linked issue did not authorize

---

## How to Contribute (the actual flow)

### 1. Bug reports
- Use the **Bug Report** issue template.
- Include: clear steps to reproduce, expected vs actual behavior, OS, Venore version, relevant logs.
- Wait for the maintainer to triage and label.

### 2. Feature requests
- Use the **Feature Request** issue template.
- Describe the *problem* you want solved, not the implementation you have in mind.
- Wait for explicit `approved` label before coding. A label of `needs-discussion`, `wontfix`, or no engagement means **do not start a PR**.

### 3. Pull requests (after an approved issue)
- Fork and branch from `main`.
- Reference the issue: `Fixes #N` in the PR body.
- Keep the diff inside the scope the issue authorized.
- Run `cargo build`, `cargo test`, and `pnpm typecheck` before pushing.
- Respond to review feedback within 7 days, or the PR will be closed (you can reopen later).

---

## Code Standards

**Rust (`venore-core`):**
- Use `VenoreError` + `Result<T>` for errors — not `anyhow`, not `String`.
- Use `tracing` macros for logging — not `println`.
- Check if a utility or pattern already exists before creating a new one.
- Follow the existing module structure.

**Frontend (`venore-desktop/ui`):**
- Zero business logic in the frontend — it only receives data and sends intents.
- Use existing shadcn / Radix UI components; do not create custom equivalents.
- All user-facing strings go through the i18n system (`useTranslation()`).

---

## Commit Messages

```
<type>: <short description in present tense, under 70 chars>

<optional body explaining the why>

Assisted-by: <tool>   # only if AI tools were involved
```

Types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `perf`.

---

## What We Will Not Accept

- Unsolicited changes (no linked approved issue)
- Large diffs that bundle multiple concerns
- Changes that add abstractions, layers, or dependencies "just in case"
- Code that duplicates functionality already in the repo
- PRs that break existing tests
- Contributions that introduce security vulnerabilities
- Any PR whose description, commits, or diff reads as AI-generated without human review

---

## Getting Help

- **GitHub Discussions** is the right place for questions about architecture, design, or "where should this live".
- **Issues are for tracked work items** (bugs / features / chores), not for open-ended Q&A.
- Read the existing code and module structure before asking placement questions.

---

## Security

Do not file public issues for security vulnerabilities. See [SECURITY.md](SECURITY.md). Reports go to **hi@skalar.app**.

---

## License

By contributing, you agree that your contributions are licensed under [AGPL-3.0-or-later](LICENSE).
