# Contributing to Veltrix

Thank you for your interest in contributing to Veltrix.

Veltrix is a systems-level programming language and compiler ecosystem focused on correctness, clarity, and long-term maintainability. Contributions must align with these principles.

Before contributing, please read this document fully.

---

## Guiding Principles

Veltrix prioritizes:

- Deterministic behavior
- Strong semantic guarantees
- Clear architecture
- Minimal surface-area complexity
- Long-term ecosystem stability over short-term convenience

All contributions are evaluated against these criteria.

---

## Ways to Contribute

You can contribute by:

- Reporting bugs
- Improving documentation
- Writing tests
- Fixing small issues
- Proposing architectural improvements
- Implementing approved features

---

## Before You Start

1. Read the `README.md`
2. Review open issues
3. Check if your idea already exists
4. Open a discussion for significant changes

For non-trivial features, open an issue first before writing code.

---

## Reporting Bugs

When filing a bug report, include:

- Veltrix version / commit hash
- Operating system
- Minimal reproducible example
- Expected behavior
- Actual behavior
- Relevant error messages or stack traces

Bug reports without reproducible examples may be closed.

---

## Feature Proposals

For significant changes:

2. Clearly describe:
- Problem statement
- Proposed solution
- Alternatives considered
- Performance impact
- Breaking change implications

Major changes require discussion and approval before implementation.

---

## Development Workflow

Clone:

```bash
git clone https://github.com/<org>/veltrix.git
cd veltrix

cargo build

cargo test
