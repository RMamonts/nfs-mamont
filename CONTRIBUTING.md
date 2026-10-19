# Contributing to NFS Mamont

First off, thanks for taking the time to contribute!

We are excited that you are interested in helping to make NFS Mamont better.
This document provides guidelines and conventions to keep the project healthy
and the review process smooth. By following it you help the maintainers spend
less time on process and more time on code.

Reading our [Code of Conduct](CODE_OF_CONDUCT.md) is the first step — we expect
everyone who engages with the project to follow it.

> [!TIP]
> New to open source? [opensource.guide](https://opensource.guide/how-to-contribute/)
> is a great place to start.

---

## Table of contents

- [Getting started](#getting-started)
- [Reporting issues](#reporting-issues)
- [Submitting changes](#submitting-changes)
- [Development workflow](#development-workflow)
  - [Setting up the environment](#setting-up-the-environment)
  - [Building and testing](#building-and-testing)
  - [Code style](#code-style)
  - [Commit conventions](#commit-conventions)
  - [Branch naming](#branch-naming)
- [Pull request guidelines](#pull-request-guidelines)
  - [Checklist before submitting](#checklist-before-submitting)
  - [Review and merging](#review-and-merging)
- [Code of conduct](#code-of-conduct)
- [License](#license)

---

## Getting started

Before diving into code, please introduce yourself and understand the project:

1. Read the [README](README.md) to learn about the project's goals, features,
   and current status. Note that the project is **under active development**
   and currently targets **NFSv3** (with MOUNT), with NFSv4 planned.
2. Look at our [Kanban board](https://github.com/orgs/RMamonts/projects/1/views/1)
   (the TODO column) for issues that are ready to be picked up, or check the
   [issues](https://github.com/RMamonts/nfs-mamont/issues) page.
3. If you plan to work on something non-trivial, comment on the issue first and
   propose your approach. This avoids duplicated work and helps maintainers give
   early feedback.

We welcome all kinds of contributions, not just code:

- Reporting bugs and unexpected behavior.
- Proposing features and improvements.
- Improving documentation, comments, and doc comments.
- Adding or improving tests and fuzz targets.
- Reviewing others' pull requests.

## Reporting issues

Bugs, feature requests, and questions are all tracked as GitHub issues.

- **Search first.** Before opening a new issue, check whether one already
  exists (including closed ones) to avoid duplicates.
- **One issue per report.** Keep each issue focused on a single problem or
  feature so it can be discussed and tracked independently.
- **Bug reports** should include:
  - A clear, descriptive title.
  - The exact steps to reproduce the problem.
  - The expected behavior and what actually happened.
  - Environment details: OS version, Rust toolchain (`rustc --version`), and
    dependency versions.
  - If possible, a minimal reproducible example.
- **Feature requests** should describe the problem you are trying to solve and
  a clear motivation, not just the requested solution. This helps us understand
  and evaluate the trade-offs.

> [!NOTE]
> Security vulnerabilities should **not** be reported as public issues. Please
> report them privately to the maintainers (see the CODEOWNERS file for current
> maintainers) so they can be addressed before disclosure.

## Submitting changes

The contribution workflow is:

1. **Fork and clone** the repository and add your fork as a remote.
2. Create a **feature branch** from `main` (see [Branch naming](#branch-naming)).
3. Make your changes in small, self-contained commits with descriptive messages
   (see [Commit conventions](#commit-conventions)).
4. Run the [local checks](#building-and-testing) before you push.
5. Open a [pull request](#pull-request-guidelines) against `main`.
6. Respond to review feedback and iterate until the PR is merged.

## Development workflow

### Setting up the environment

- **Rust toolchain:** the project targets **MSRV 1.75.0**. CI runs on both
  `1.75.0` and the current stable toolchain, so your code must compile on the
  MSRV. Install with [`rustup`](https://rustup.rs/):

  ```bash
  rustup install 1.75.0
  rustup component add rustfmt clippy --toolchain 1.75.0
  ```

  (Working on the current stable toolchain and only occasionally checking the
  MSRV is the normal workflow.)

- **Platform:** development happens on **Linux**. `libc` is only pulled in by
  the optional [`mlock`](README.md#safety--requirements) feature.

### Building and testing

The repository is a Cargo workspace with a single member crate, `nfs-mamont`.

```bash
# Build the library
cargo build

# Run the test suite (including doctests)
cargo test

# Static analysis
cargo clippy -- -D warnings

# Formatting check
cargo fmt --all --check

# Documentation
cargo doc --no-deps
```

Optional, to exercise the memory-locking helpers:

```bash
cargo test --features mlock
```

These commands mirror what CI runs in its `lint`, `test`, `docs`, `security`,
and `udeps` jobs, so passing them locally is the best signal that CI will be
green on your PR.

The security and unused-dependency jobs require extra tools:

```bash
# Security audit (known vulnerabilities in the dependency tree)
cargo install cargo-audit
cargo audit

# Check for unused dependencies (requires nightly)
cargo +nightly install cargo-udeps
cargo +nightly udeps --all-targets
```

### Code style

We keep the code formatting and linting rules enforced by the toolchain:

- **Rustfmt.** Always run `cargo fmt --all --check` before committing. The
  project ships a [`.rustfmt.toml`](.rustfmt.toml) that configures the default
  style — follow it rather than overriding it locally.
- **Clippy.** Run `cargo clippy -- -D warnings` and fix all warnings. CI treats
  warnings as errors, so a clean local run avoids review churn.
- **Consistency.** Match the style of the surrounding code: naming, module
  organization, error handling (this project relies heavily on `Result` and
  its custom RPC/XDR layer), and documentation comments. When in doubt, prefer
  the patterns already present in the codebase.

### Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/). This keeps
the history readable and, eventually, enables changelog automation.

Commit messages follow the form:

```
<type>(<optional scope>): <short summary>
```

with the body (separated by a blank line) explaining the *why* of the change.

**Types** used in this project:

| Type        | When to use                                              |
| ----------- | -------------------------------------------------------- |
| `feat`      | A new feature or capability                              |
| `fix`       | A bug fix                                                |
| `refactor`  | Code change with no functional change in behavior        |
| `test`      | Adding or updating tests                                 |
| `docs`      | Documentation only (README, comments, doc comments)      |
| `perf`      | A performance improvement                                |
| `chore`     | Maintenance, tooling, version bumps, CI, and the like    |

**Scopes**, when helpful, name the module or crate the change touches, for
example `feat(nfs-mamont): ...`, `refactor(backend): ...`, or
`fix(mount): ...`.

Examples of messages already used in the repository:

```
feat: return no_delay socket option (#381)
fix(nfs-mamont): make mk_node pub instead of pub(super) (#405)
refactor(backend): move tests to backend/tests module (#399)
test(serializer): move tests to dedicated module (#401)
docs: fix typos and update Rust version in README (#408)
```

Keep the subject line short (under ~72 characters), imperative, and lowercase
after the type. If the change is complete in a single line, don't add a body
just for the sake of it.

### Branch naming

Use short, descriptive branch names that reflect the work. It helps to prefix
with your username or a `chore/`/`fix`-style hint, for example:

```
jk-nfsv3-read
chore/remove-mirror-fs
fix/encapsulation
```

Many contributors on this project use a `<username>/<short-description>`
scheme — pick whichever you find clear and keep it consistent.

## Pull request guidelines

Open a pull request against `main` with a title and description that make the
goal obvious to reviewers.

- **Reference the issue** your PR addresses (e.g. "Closes #123").
- **Describe what and why.** Summarize the change, the motivation, and any
  trade-offs or alternative approaches you considered.
- **Keep PRs focused.** Smaller PRs are easier to review and quicker to merge.
  If a change naturally splits into independent parts, consider multiple PRs.
- **Don't include unrelated changes** (whitespace churn, renamed identifiers,
  or fixes for unrelated code). If you notice a separate bug, open an issue for
  it instead.

### Checklist before submitting

Make sure your PR is ready:

- [ ] Code compiles on the **MSRV (1.75.0)** and the current stable toolchain.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy -- -D warnings` passes.
- [ ] `cargo test` passes (including new tests for your change).
- [ ] `cargo audit` reports no new known vulnerabilities.
- [ ] New public API is covered by rustdoc comments and, where appropriate,
      tests/doctests.
- [ ] Documentation (README, docs in code) is updated if behaviour changed.

### Review and merging

- Be patient and responsive. Maintainers may ask for changes; treat feedback
  as a collaboration toward a better change, not as criticism.
- Address review comments in follow-up commits rather than force-pushing a
  rewritten history during review (a rewritten history makes it hard to see
  what changed since the last review).
- After approval, maintainers will merge your PR. Please do not merge your own
  PR without approval.

## Code of conduct

All project participants must follow our
[Code of Conduct](CODE_OF_CONDUCT.md). Harassment, discrimination, and other
unacceptable behavior will not be tolerated. Please report concerns to the
maintainers listed in the [CODEOWNERS](CODEOWNERS) file.

## License

By contributing to NFS Mamont you agree that your contributions are licensed
under the [MIT License](LICENSE), as described in the project license. Make
sure you have the right to contribute under this license.