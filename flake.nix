{
  description = "MyHelp CLI and Tauri desktop development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # nixpkgs unstable dropped Intel macOS in 26.11. Keep the declared CLI
  # package available from the final stable branch that supports it.
  inputs.nixpkgsDarwin.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";

  outputs =
    {
      self,
      nixpkgs,
      nixpkgsDarwin,
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
      pkgsFor =
        system:
        if system == "x86_64-darwin" then
          nixpkgsDarwin.legacyPackages.${system}
        else
          nixpkgs.legacyPackages.${system};

      linuxSystem = "x86_64-linux";
      linuxPkgs = pkgsFor linuxSystem;
    in
    {
      packages = forAllCliSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          myhelp-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "myhelp";
            version = "0.6.0";
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
