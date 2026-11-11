# Contributing to GitDragon

Thanks for your interest in improving **GitDragon** (`git-dragon`), the animated pixel-art dragon generator for GitHub profiles. Contributions of all sizes are welcome: bug reports, new ideas, doc fixes, and code.

## How to Contribute

1. Fork the repository and create a feature branch.
2. Make your changes with clear, descriptive commit messages.
3. Ensure all tests pass and clippy is clean.
4. Open a Pull Request describing your changes.

## Development Setup

```bash
git clone https://github.com/codewithsyedabdullah/git-dragon.git
cd git-dragon
cargo build
```

## Running Tests

```bash
cargo test
cargo clippy -- -D warnings
```

## Code Style

- Use idiomatic Rust with explicit error handling (no `.unwrap()` in production code).
- Keep functions focused and well-documented.
- Follow the existing module structure: `collect`, `history`, `coat`, `render`, `output`.

## Reporting Issues

Open a GitHub Issue with:
- A clear description of the problem.
- Steps to reproduce.
- Expected vs. actual behavior.
- Your OS, Rust version, and `git-dragon` version.