{
  description = "MyHelp CLI and Tauri desktop development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    {
      self,
      nixpkgs,
      ...
    }:
    let
      cliSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllCliSystems = nixpkgs.lib.genAttrs cliSystems;

      linuxSystem = "x86_64-linux";
      linuxPkgs = nixpkgs.legacyPackages.${linuxSystem};
    in
    {
      packages = forAllCliSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          myhelp-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "myhelp";
            version = "0.1.0";
            src = pkgs.lib.cleanSource ./.;

            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [
              "-p"
              "myhelp-cli"
            ];
            cargoTestFlags = [
              "-p"
              "myhelp-core"
              "-p"
              "myhelp-cli"
            ];

            meta = {
              description = "Create, search, and read personal help pages";
              homepage = "https://github.com/hjosugi/myhelp";
              license = pkgs.lib.licenses.mit;
              mainProgram = "myhelp";
            };
          };

          default = self.packages.${system}.myhelp-cli;
        }
      );

      apps = forAllCliSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.myhelp-cli}/bin/myhelp";
          meta.description = "Create, search, and read personal help pages";
        };
      });

      checks = forAllCliSystems (system: {
        myhelp-cli = self.packages.${system}.myhelp-cli;
      });

      devShells.${linuxSystem}.default = linuxPkgs.mkShell {
        packages = with linuxPkgs; [
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

          LD_LIBRARY_PATH = linuxPkgs.lib.makeLibraryPath (
            with linuxPkgs;
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
