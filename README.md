# cargo ab-lint
CLI / cargo subcommand with extra lints for rust projects.

## Lints
* Workspace dependencies with redundant `features` already present in root.
* Workspace dependencies with redundant `default-features` set.
* Unused workspace dependencies.

## Usage
Run it in a cargo project to view lints, if any.
```
cargo ab-lint
```

To apply fixes:
```
cargo ab-lint --fix
```

## Install
### Using cargo
<!-- Latest release
```sh
cargo install cargo-ab-lint
``` -->

Latest code direct from git
```sh
cargo install --git https://github.com/alexheretic/cargo-ab-lint
```

## Minimum supported rust compiler
Maintained with [latest stable rust](https://gist.github.com/alexheretic/d1e98d8433b602e57f5d0a9637927e0c).
