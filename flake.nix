{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, flake-utils, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        version = "1.0.0";
        pkgs = (import nixpkgs) { inherit system; };
        nativeBuildInputs = with pkgs; [ cmake pkg-config rustc cargo ];
        buildInputs = [ ];
        mkPackage = { name, buildInputs ? [ ] }: pkgs.rustPlatform.buildRustPackage {
          cargoBuildOptions = "--package ${name}";
          pname = name;
          inherit version;
          inherit buildInputs;
          inherit nativeBuildInputs;
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "tree-sitter-wbproto-0.0.1" = "sha256-oBo41rvptzQzsyD6chjbvOfiH9+SVVX+s3+yDvSXWk4=";
            };
          };
          src = ./.;
          postInstall = "
            cp -r target/*/release/share $out/share
          ";
        };
      in
      rec {
        formatter = pkgs.nixpkgs-fmt;
        packages.wbproto-beautifier = mkPackage { name = "wbproto-beautifier"; };
        packages.default = packages.wbproto-beautifier;
        apps = rec {
          wbproto-beautifier = { type = "app"; program = "${packages.default}/bin/wbproto-beautifier"; };
          default = wbproto-beautifier;
        };
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [ rustc cargo busybox clang-tools ];
          inherit buildInputs;
        };
      }
    );
}
