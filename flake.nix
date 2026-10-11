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
          # The audio tests convert clips with ffmpeg, as the tool does.
          nativeCheckInputs = [ pkgs.ffmpeg ];
          # The completion tests drive bash, zsh and fish, which the dev shell provides; installing the
          # package should not pull in three shells.
          checkFlags = [ "--skip=completes_by_the_rules" ];
          # The sound scripts stay scripts so users can read and adapt them; `generate` runs these copies.
          postInstall = ''
            install -Dm755 sounds/generate.sh $out/share/fix-whxm4-voice/generate.sh
            install -Dm755 sounds/battery.sh $out/share/fix-whxm4-voice/battery.sh
            wrapProgram $out/bin/fix-whxm4-voice \
              --prefix PATH : ${
                pkgs.lib.makeBinPath [
                  pkgs.ffmpeg
                  pkgs.bash
                  pkgs.gawk
                  pkgs.coreutils
                ]
              } \
              --set-default FIX_WHXM4_VOICE_GENERATE_SCRIPT $out/share/fix-whxm4-voice/generate.sh \
              --set-default FIX_WHXM4_VOICE_BATTERY_SCRIPT $out/share/fix-whxm4-voice/battery.sh
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
            pkgs.bashInteractive
            pkgs.zsh
            pkgs.fish
            pkgs.pkg-config
            pkgs.dbus
          ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });
    };
}
