# Language workflow starter pack

These pages are small, portable reminders for creating, testing, formatting,
and updating projects. They are deliberately not framework templates.

Preview the pack from a checkout:

```bash
cargo run -p myhelp-cli -- \
  --pages-dir examples/language-workflows list
cargo run -p myhelp-cli -- \
  --pages-dir examples/language-workflows show rust-new-project
```

To make a page personal, copy it into the vault reported by `myhelp path` and
edit it there. Keep machine-specific Nix Flake locations, organization names,
and private URLs in the personal copy.

Each page cites primary documentation. Please verify commands against those
links when a language toolchain makes a breaking release.
