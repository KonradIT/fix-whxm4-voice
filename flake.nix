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
          nativeBuildInputs = [
            pkgs.makeWrapper
            pkgs.installShellFiles
            pkgs.pkg-config
          ];
          # bluer talks to BlueZ over D-Bus.
          buildInputs = [ pkgs.dbus ];
          # The sound script stays a script so users can read and adapt it; `generate` runs this copy.
          postInstall = ''
            install -Dm755 sounds/generate.sh $out/share/fix-whxm4-voice/generate.sh
            wrapProgram $out/bin/fix-whxm4-voice \
              --prefix PATH : ${
                pkgs.lib.makeBinPath [
                  pkgs.ffmpeg
                  pkgs.bash
                  pkgs.gawk
                  pkgs.coreutils
                ]
              } \
              --set-default FIX_WHXM4_VOICE_GENERATE_SCRIPT $out/share/fix-whxm4-voice/generate.sh
          ''
          + pkgs.lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
            installShellCompletion --cmd fix-whxm4-voice \
              --bash <($out/bin/fix-whxm4-voice completions bash) \
              --zsh <($out/bin/fix-whxm4-voice completions zsh) \
              --fish <($out/bin/fix-whxm4-voice completions fish)
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
            pkgs.pkg-config
            pkgs.dbus
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });
    };
}
