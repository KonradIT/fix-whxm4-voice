{
  description = "Replace the voice prompts on Sony WH-1000XM4 headphones";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "fix-whxm4-voice";
          version = (pkgs.lib.importTOML ./Cargo.toml).package.version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          postInstall = ''
            wrapProgram $out/bin/fix-whxm4-voice \
              --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.ffmpeg ]}
          '';
          meta.mainProgram = "fix-whxm4-voice";
        };
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
            pkgs.rust-analyzer
            pkgs.ffmpeg
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });
    };
}
