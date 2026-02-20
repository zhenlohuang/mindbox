# Repository Guidelines

## Project Structure & Module Organization
This repository is a Rust workspace with four crates:
- `mindbox-common`: shared config, types, and error handling.
- `mindbox-kernel`: kernel abstraction and skill matching.
- `mindbox-server`: Axum-based API/SSE server.
- `mindbox-cli`: command-line client (`mindbox`).
- `mindbox-skills/`: task skills (`*/SKILL.md`) injected by the kernel.
- `docs/`: design and architecture notes.

## Build, Test, and Development Commands
- `cargo build --workspace`: build all crates.
- `cargo test --workspace`: run all unit and async tests.
- `cargo run -p mindbox-server`: start server on `MINDBOX_PORT`/`PORT` (default `8080`).
- `cargo run -p mindbox-cli -- --help`: inspect CLI commands.
- `cargo run -p mindbox-cli -- task start --help`: inspect task options.
- `docker compose up --build mindbox`: run the full containerized stack.
- `cargo fmt --all` and `cargo clippy --workspace --all-targets`: formatting and lint checks before PR.

## Coding Style & Naming Conventions
Use idiomatic Rust (edition `2024`) and keep formatting rustfmt-compatible (4-space indentation, trailing commas where helpful).
- Modules/functions/files: `snake_case`
- Types/traits/enums: `CamelCase`
- Constants/env vars: `SCREAMING_SNAKE_CASE` (for example `MINDBOX_KERNEL`)

Keep shared contracts in `mindbox-common`; avoid duplicating request/response models across crates.

## Testing Guidelines
Place tests close to implementation with `#[cfg(test)]` modules (see `mindbox-common/src/config.rs`), and use `#[tokio::test]` for async paths (see `mindbox-server/src/services/task_service.rs`).
- Prefer behavior-focused names (for example `config_defaults_are_resolved`).
- Run targeted tests while iterating: `cargo test -p mindbox-common`.
- Run `cargo test --workspace` before opening a PR.

## Commit & Pull Request Guidelines
Current history favors short, imperative subjects with optional scope prefix (example: `docs: Add initial draft of Mindbox system design`).
- Recommended format: `<scope>: <imperative summary>` (`server: validate task id format`).
- Keep commits focused by crate or feature.
- PRs should include: purpose, impacted crates, test commands/results, and CLI/API examples for behavior changes.
- Link related issues/tasks and note any required env vars or migration steps.
