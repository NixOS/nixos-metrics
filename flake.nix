{
  description = "NixOS metrics";

  nixConfig.extra-substituters = ["https://nixos-metrics.cachix.org"];
  nixConfig.extra-trusted-public-keys = ["nixos-metrics.cachix.org-1:rzijSu/2SDjsi1XDK5cdkNCGrWjU/SMpvyGA9auWmb0="];

  inputs.nixpkgs.url = "nixpkgs/nixos-unstable";
  inputs.flake-compat.url = "github:edolstra/flake-compat";
  inputs.flake-compat.flake = false;
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  inputs.pre-commit-hooks.inputs.nixpkgs.follows = "nixpkgs";
  inputs.pre-commit-hooks.inputs.flake-utils.follows = "flake-utils";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  inputs.rust-overlay.inputs.flake-utils.follows = "flake-utils";
  inputs.naersk.url = "github:nmattia/naersk";
  inputs.naersk.inputs.nixpkgs.follows = "nixpkgs";

  outputs = {
    self,
    nixpkgs,
    flake-compat,
    flake-utils,
    pre-commit-hooks,
    rust-overlay,
    naersk,
  }:
    flake-utils.lib.eachSystem
    [
      flake-utils.lib.system.x86_64-linux
      # TODO: test and add to CI
      #flake-utils.lib.system.aarch64-linux
      #flake-utils.lib.system.aarch64-linux
      flake-utils.lib.system.aarch64-darwin
      #flake-utils.lib.system.x86_64-darwin
    ]
    (
      system: let
        inherit (nixpkgs) lib;

        warnToUpdateNix = lib.warn "Consider updating to Nix > 2.7 to remove this warning!";
        pkgCargo = lib.importTOML ./Cargo.toml;
        version = "${pkgCargo.package.version}_${builtins.substring 0 8 self.lastModifiedDate}_${self.shortRev or "dirty"}";

        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        rust = let
          _rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analysis"
              "rls-preview"
              "rustfmt-preview"
              "clippy-preview"
            ];
          };
        in
          pkgs.buildEnv {
            name = _rust.name;
            inherit (_rust) meta;
            buildInputs = [pkgs.makeWrapper];
            paths = [_rust];
            pathsToLink = ["/" "/bin"];
            # XXX: This is needed because cargo and clippy commands need to
            # also be aware of other binaries in order to work properly.
            # https://github.com/cachix/pre-commit-hooks.nix/issues/126
            postBuild = ''
              for i in $out/bin/*; do
                wrapProgram "$i" --prefix PATH : "$out/bin"
              done
            '';
          };

        pre-commit = pre-commit-hooks.lib.${system}.run {
          src = self;
          hooks = {
            alejandra = {
              enable = true;
            };
            rustfmt = {
              enable = true;
              entry = pkgs.lib.mkForce "${rust}/bin/cargo-fmt fmt -- --check --color always";
            };
          };
        };

        naersk-lib = naersk.lib."${system}".override {
          cargo = rust;
          rustc = rust;
        };

        nixos-metrics = naersk-lib.buildPackage {
          inherit (pkgCargo.package) name;
          inherit version;

          root = self;

          buildInputs = with pkgs;
            [
              openssl
              openssl.dev
              pkg-config
            ]
            ++ lib.optional pkgs.hostPlatform.isDarwin [
              pkgs.libiconv
            ];
        };
      in rec {
        checks = {inherit pre-commit nixos-metrics;};

        packages = {inherit nixos-metrics;};
        packages.default = packages.nixos-metrics;

        devShells.default = pkgs.mkShell {
          inputsFrom = [nixos-metrics];
          shellHook =
            pre-commit.shellHook
            + ''
              echo "=== NixOS metrics development shell ==="
              echo "Info: Git hooks can be installed using \`pre-commit install\`"
            '';
        };

        defaultPackage = warnToUpdateNix packages.default;
        devShell = warnToUpdateNix devShells.default;
      }
    );
}
