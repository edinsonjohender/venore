# Security Policy

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Instead, email **hi@skalar.app** with the following:

- A clear description of the vulnerability
- Steps to reproduce (proof-of-concept, sample input, etc.)
- The Venore version, OS, and platform where you observed it
- Impact assessment (what an attacker could achieve)
- Any suggested mitigation, if you have one

If you would like the report to be encrypted, request a public key in your first message.

## What to expect

- **Acknowledgment:** within 5 business days from receipt.
- **Initial assessment:** within 14 business days, including whether we accept the report and a rough timeline.
- **Fix and disclosure:** we coordinate the disclosure timeline with you. Default policy is to publish a fix and a public advisory **within 90 days** of the report, sooner if a fix is straightforward and there is no active exploitation in the wild.

We will credit you in the advisory unless you prefer to remain anonymous.

## Scope

In-scope:

- The Venore desktop application (`venore-desktop`) and the libraries it ships (`venore-core`, `venore-cli`, `venore-api`).
- Issues in our default dependencies that are exploitable through Venore's exposed surface (Tauri commands, mesh WebSocket, embedded HTTP server, file system access).
- Privilege-escalation, sandbox-escape, or RCE vectors via the AI agent's tool inventory.
- Leakage of user secrets stored in the OS keyring through Venore code paths.

Out of scope:

- Findings that require physical access or that the user has actively configured against (e.g. installing untrusted plugins, running with `--no-sandbox`, granting unrestricted file system permissions).
- Theoretical issues without a working proof-of-concept.
- Bugs in third-party services (Anthropic API, OpenAI API, GitHub, etc.) that are not introduced by Venore's integration.
- Vulnerabilities in dependencies that have already been patched in a newer release we have not yet adopted — please file an issue requesting the upgrade instead.

## Coordinated disclosure

We follow a coordinated disclosure model. Public disclosure before a fix is available is discouraged. If you are planning to publish research, please coordinate the timeline with us so users can update before the details are public.

If we cannot ship a fix within the agreed timeline, we will explain why and renegotiate. We will not retaliate against good-faith researchers who follow this policy.
