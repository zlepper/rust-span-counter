{
  description = "CLI tool that extracts strings from Rust files and provides word-by-word character spans";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = nixpkgs.legacyPackages;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "rust-span-counter";
            version = (pkgs.lib.importTOML ./Cargo.toml).package.version;

            src = ./.;

            cargoHash = "sha256-m2yCZsFAUjM+cSJDzM5IgL3a/ySOeuF1cPpIt/rW9Hg=";

            meta = with pkgs.lib; {
              description = "CLI tool that extracts strings from Rust files and provides word-by-word character spans";
              homepage = "https://github.com/user/rust-span-counter";
              license = licenses.mit;
              maintainers = [ ];
            };
          };
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/rust-span-counter";
        };
      });
    };
}