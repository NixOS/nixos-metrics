{
  description = "NixOS metrics";

  inputs.nixpkgs.url = "nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
  inputs.pre-commit-hooks.inputs.nixpkgs.follows = "nixpkgs";
  inputs.pre-commit-hooks.inputs.flake-utils.follows = "flake-utils";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  inputs.rust-overlay.inputs.flake-utils.follows = "flake-utils";
  inputs.naersk.url = "github:nmattia/naersk";
  inputs.naersk.inputs.nixpkgs.follows = "nixpkgs";

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , pre-commit-hooks
    , rust-overlay
    , naersk
    }:
    let
      SYSTEMS = [
        #"aarch64-darwin"
        #"aarch64-linux"
        #"x86_64-darwin"
        "x86_64-linux"
      ];
      cargoTOML = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      version = "${cargoTOML.package.version}_${builtins.substring 0 8 self.lastModifiedDate}_${self.shortRev or "dirty"}";
    in
    flake-utils.lib.eachSystem SYSTEMS (system:
    let
      pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
      rustToolchain = pkgs.rust-bin.stable.latest.minimal.override {
        extensions = [
          # minimal
          "rustc"
          "rust-std"
          "cargo"
          # default
          "clippy"
          "rustfmt-preview"
          "rust-docs"
          # extra
          "rls-preview"
          "rust-analysis"
          "rust-src"
        ];
      };
    in
    {
      devShell = pkgs.mkShell {
        buildInputs = [ pkgs.openssl pkgs.openssl.dev ];
        nativeBuildInputs = [ pkgs.pkgconfig rustToolchain ];
        packages = [ pkgs.rnix-lsp pkgs.entr ];
      };
    }
    );
}
