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

  outputs = {
    self,
    nixpkgs,
    flake-compat,
    flake-utils,
    pre-commit-hooks,
  }:
    flake-utils.lib.eachSystem
    [
      flake-utils.lib.system.x86_64-linux
      # TODO: test and add to CI
      #flake-utils.lib.system.aarch64-linux
      #flake-utils.lib.system.aarch64-linux
      #flake-utils.lib.system.aarch64-darwin
      #flake-utils.lib.system.x86_64-darwin
    ]
    (
      system: let
        inherit (nixpkgs) lib;

        warnToUpdateNix = lib.warn "Consider updating to Nix > 2.7 to remove this warning!";

        pkgs = nixpkgs.legacyPackages.${system};

        pre-commit = pre-commit-hooks.lib.${system}.run {
          src = self;
          hooks = {
            alejandra = {
              enable = true;
            };
            black = {
              enable = true;
            };
            isort = {
              enable = true;
            };
          };
        };
        process-data = pkgs.writers.writePython3Bin "process-data" {flakeIgnore = ["E501"];} ./process.py;
      in rec {
        checks = {inherit pre-commit;};

        packages = {inherit process-data;};
        packages.default = packages.process-data;

        devShells.default = pkgs.mkShell {
          buildInputs = [pkgs.python3];
          shellHook =
            pre-commit.shellHook
            + ''
              echo "=== NixOS metrics website development shell ==="
              echo "Info: Git hooks can be installed using \`pre-commit install\`"
            '';
        };

        defaultPackage = warnToUpdateNix packages.default;
        devShell = warnToUpdateNix devShells.default;
      }
    );
}
