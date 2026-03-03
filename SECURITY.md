# Security Policy

Veltrix is a systems-level programming language and compiler ecosystem.  
Security, correctness, and deterministic behavior are core priorities.

This document outlines supported versions and how to report vulnerabilities responsibly.

---

## Supported Versions

Security updates are provided only for actively maintained versions.

| Version | Supported |
|---------|----------|
| 0.x (main branch) | ✅ |
| < 0.x releases | ❌ |

### Notes

- Until **v1.0.0**, Veltrix is considered pre-stable.
- APIs, semantics, and internal behavior may change.
- Only the latest development branch is guaranteed to receive security patches.

After v1.0.0, a more formal support matrix will be introduced.

---

## What Qualifies as a Security Vulnerability?

Examples include:

- Memory safety violations
- Arbitrary code execution via compiler/runtime bugs
- Sandbox escapes (if applicable)
- Denial-of-service vectors in parser or interpreter
- Privilege escalation in tooling
- Dependency-related security flaws
- Compiler miscompilations that introduce unsafe behavior

General bugs, crashes, or feature requests should be filed as normal GitHub issues.

---

## Reporting a Vulnerability

If you discover a security issue:

**Do NOT open a public GitHub issue.**

Instead, report privately via:

**Email:** [INSERT SECURITY CONTACT EMAIL HERE]

Include:

- Detailed description of the vulnerability
- Steps to reproduce
- A minimal proof-of-concept (if possible)
- Affected version or commit hash
- Environment details (OS, architecture, toolchain)

Incomplete reports may delay investigation.

---

## Response Timeline

- **Acknowledgement:** Within 72 hours
- **Initial assessment:** Within 7 days
- **Resolution timeline:** Depends on severity and complexity

For critical vulnerabilities, fixes may be prioritized immediately.

---

## Disclosure Policy

Veltrix follows responsible disclosure:

1. The issue is investigated privately.
2. A fix is developed and tested.
3. A patch is released.
4. A public advisory is published.

We request reporters to avoid public disclosure until a fix is available.

Credit will be given to reporters unless anonymity is requested.

---

## Security Best Practices for Contributors

Contributors working on core systems should:

- Avoid introducing unsafe patterns.
- Justify any use of `unsafe` code.
- Add tests covering edge cases and failure modes.
- Consider denial-of-service vectors in parser and interpreter changes.
- Review dependencies for known vulnerabilities.

Security is a shared responsibility.

---

## Disclaimer

Veltrix is experimental software prior to v1.0.0.  
It should not be used in production systems requiring hardened security guarantees.

---

Thank you for helping improve the safety and reliability of Veltrix.
