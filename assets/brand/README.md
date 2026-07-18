# MyHelp brand assets

`myhelp-mark.svg` is the editable source for every generated application icon
under `src-tauri/icons`.

## Design

The mark combines a plain help page, a folded corner, and an `M`. The shapes
stay deliberately broad so the page and monogram remain recognizable at
16–32 px. The evergreen outer shape gives the light paper mark a consistent
boundary on both light and dark launchers.

## Color tokens

| Token | Value | Use |
|---|---|---|
| Evergreen | `#24563A` | Primary background and UI action color |
| Leaf | `#397B53` | Monogram and UI focus/accent color |
| Mist | `#CFE8D6` | Fold detail and quiet highlight |
| Paper | `#F7F9F5` | Page surface and light foreground |
| Ink | `#17211C` | UI text paired with the brand palette |

The tokens reuse the green, paper, and ink family already present in the
desktop interface. The icon does not use gradients or platform-dependent
effects.

## Distinctness and small sizes

The page silhouette is the primary shape; the `M` is contained inside it
instead of acting as a standalone ribbon or wordmark. The design avoids the
terminal prompts, compass shapes, animal mascots, and red wordmarks associated
with adjacent cheatsheet tools. Its broad strokes remain separate in the
16 px Windows icon and the 30 px Windows tile.

The evergreen outer field is intentionally opaque around the light page, so a
separate light- or dark-launcher variant is not needed.

## Platform outputs

| Platform | Generated assets |
|---|---|
| Linux | PNGs from 32 px through 512 px |
| macOS | `icon.icns` with the standard desktop icon sizes |
| Windows | `icon.ico` with 16, 24, 32, 48, 64, and 256 px entries, plus Store tiles from 30 px through 310 px |

`pnpm icons:check` also verifies that the Tauri product name, window title, and
bundle icon configuration still point to the MyHelp identity. CI runs the
check on Linux, macOS, and Windows.

## Regenerating icons

From the repository root:

```bash
pnpm icons
```

This runs Tauri's icon generator against `myhelp-mark.svg` and replaces the
Linux PNG, macOS ICNS, Windows ICO, and Windows Store tile files together.
Mobile outputs from Tauri's general-purpose generator stay in a temporary
directory so the desktop repository does not accumulate unused assets.

CI verifies that the generated files remain current:

```bash
pnpm icons:check
```

## License

The MyHelp mark is original project artwork and is available under the same
[MIT License](../../LICENSE) as the source code. Files in `src-tauri/icons` are
generated derivatives of this SVG and use the same license.
