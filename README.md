# Nebula

Nebula is a GTK/libadwaita front-end for browsing and managing Void Linux packages, using the XBPS tooling in the background.

## Requirements

- A Void Linux installation
- GTK 4 and libadwaita runtimes
- Rust 1.76 or newer

## Quick Start

```sh
cargo run
```

## Production Build

```sh
cargo build --release
```

The optimized binary is written to `target/release/nebula-gtk`. Use `cargo run --release` if you want to execute the release build directly after compiling.

## Category Data

- Clone `void-linux/void-packages` into `vendor/void-packages` to supply package metadata.
- Regenerate the curated suggestions file when the repository changes:

  ```sh
  SKIP_GRESOURCE=1 cargo run --bin category_harvest
  ```

- Hand edits live in `data/category_overrides.toml`; the generated dataset is saved to `data/generated/category_suggestions.json`.
