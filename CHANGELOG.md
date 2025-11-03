# Changelog

## Unreleased
- Document Rust 1.88.0 minimum requirement now that the 2024 edition is stable.
- Upgrade GTK stack to `gtk4 0.10.2`, `libadwaita 0.8.0`, and `glib-build-tools 0.21.0`.
- Update dependency set to `reqwest 0.12`, `feed-rs 2.3`, `toml 0.9`, and `phf 0.13` (including build-time codegen tooling).
- Regenerate PHF category map with the new codegen API and migrate `glib::clone!` usage to the latest capture syntax.
