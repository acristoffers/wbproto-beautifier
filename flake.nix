{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    flake-utils.url = "github:numtide/flake-utils";

    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };
  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        version = "1.0.0";
        pkgs = (import nixpkgs) { inherit system; };
        naersk' = pkgs.callPackage naersk { };
        buildInputs = [ ];
        mkPackage = { name, buildInputs ? [ ] }: naersk'.buildPackage {
          cargoBuildOptions = opts: opts ++ [ "--package" name ];
          inherit buildInputs;
          inherit name;
          inherit version;
          nativeBuildInputs = with pkgs; [ cmake pkg-config ];
          src = ./.;
          postInstall = "
            cp -r target/release/share $out/share
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
          nativeBuildInputs = with pkgs; [ rustc cargo ];
          inherit buildInputs;
        };
      }
    );
}
