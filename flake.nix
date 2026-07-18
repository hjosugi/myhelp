{
  description = "MyHelp CLI and Tauri desktop development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          clippy
          rust-analyzer
          rustc
          rustfmt

          nodejs_24
          pnpm_10

          pkg-config
          glib
          gtk3
          webkitgtk_4_1
          libsoup_3
          openssl
          libayatana-appindicator
          librsvg
        ];

        env = {
          # WebKitGTK can otherwise fail with EGL_BAD_PARAMETER on some Linux
          # graphics stacks while running `pnpm tauri dev`.
          WEBKIT_DISABLE_COMPOSITING_MODE = "1";

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
            with pkgs;
            [
              glib
              gtk3
              webkitgtk_4_1
              libsoup_3
              openssl
              libayatana-appindicator
              librsvg
            ]
          );
        };
      };
    };
}
