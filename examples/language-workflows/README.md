# Language workflow starter pack

These pages are small, portable reminders for creating, testing, formatting,
and updating projects. They form the maintained base pack and are deliberately
not framework templates.

Preview the pack from a checkout:

```bash
cargo run -p myhelp-cli -- \
  --pages-dir examples/language-workflows list
cargo run -p myhelp-cli -- \
  --pages-dir examples/language-workflows show rust-new-project
```

`manifest.json` is the pack inventory and review record. On 2026-07-22, every
command was checked against the primary documentation linked by its page and
against the following reproducible reference snapshot. Versions come from
Nixpkgs unstable; the Quicklisp date comes from its published distribution:

<!-- markdownlint-disable MD013 MD060 -->

| Page | Verified toolchain |
|---|---|
| C and C++ | CMake 4.1.2 |
| Common Lisp | SBCL 2.6.5; Quicklisp distribution dated 2026-01-01 |
| Elixir | Elixir 1.18.4 |
| Gleam | Gleam 1.17.0 |
| Go | Go 1.26.4 |
| Haskell | cabal-install 3.16.1.0 |
| Java | OpenJDK 21.0.12; Maven 3.9.16 |
| Lua | Lua 5.4.7 |
| Python | uv 0.11.28 |
| Ruby | Ruby 3.4.9; Bundler 2.7.2 |
| Rust | Rust 1.96.1, edition 2024 |
| TypeScript | Node.js 24.18.0; pnpm 11.15.0; TypeScript 5.9.3 |
| Zig | Zig 0.16.0 |

<!-- markdownlint-enable MD013 MD060 -->

These are verification snapshots, not minimum versions or project pins.
Recheck a page and update its manifest entry when a breaking tool release
changes a command.

## Validation and safety labels

Run the same static contract as CI:

```bash
pnpm workflows:test
pnpm workflows:check
cargo test -p myhelp-cli --test language_workflows --locked
```

The static check requires a one-to-one manifest, one or more approved HTTPS
documentation links per page, portable paths, and explicit safety wording.
Dependency-changing examples must tell the reader to review resulting diffs.
Any future removal command must say `Destructive`, and an unbounded major
upgrade must say `Major update`. The base pack intentionally contains neither.
The Rust integration test proves every manifest topic is listed, returned
byte-for-byte by `myhelp show`, and accepted by `myhelp tldr validate`.

## Personal Nix overlay without public machine paths

A project can declare a tracked `personal` Flake input whose default
`overlays.default` is empty, then include that overlay when importing nixpkgs.
Keep the default project usable by everyone. On a personal machine, put only
the override URL in an ignored `.envrc.local` and invoke:

```bash
nix develop --override-input personal "$PERSONAL_NIX_FLAKE"
```

This keeps `/home/...`, `/Users/...`, drive letters, usernames, and private
repository URLs out of the public Flake. The Python page expresses the same
pattern as `{{personal_flake_url}}`; replace the placeholder only in a personal
copy.

## Scope and contributions

Framework workflows belong in optional sibling packs with their own manifest,
not in this language-level base pack. Keep pages to concise daily commands and
link primary documentation. The current pages were written for MyHelp; they do
not copy prose or examples from third-party cheatsheet collections.

To make a page personal, copy it into the vault reported by `myhelp path` and
edit it there. Keep machine-specific Nix Flake locations, organization names,
and private URLs in the personal copy.

Do not add a tool-generated cache, lockfile, or framework template to this
directory.
