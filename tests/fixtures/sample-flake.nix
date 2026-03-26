{
  description = "Example flake for testing";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in {
        packages = {
          default = pkgs.stdenv.mkDerivation {
            pname = "my-app";
            version = "1.0.0";
            src = ./.;
            buildInputs = [ pkgs.openssl pkgs.zlib ];
            nativeBuildInputs = [ pkgs.pkg-config ];
          };

          docs = pkgs.stdenv.mkDerivation {
            pname = "my-app-docs";
            version = "1.0.0";
            src = ./docs;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ pkgs.rustc pkgs.cargo pkgs.openssl ];
          shellHook = ''
            echo "Welcome to dev shell"
          '';
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/my-app";
        };

        checks.default = pkgs.stdenv.mkDerivation {
          pname = "my-app-tests";
          version = "1.0.0";
          src = ./.;
        };
      }
    );
}
