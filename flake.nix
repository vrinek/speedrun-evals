{
  description = "Speedrun Evals: how fast and how well local LLMs finish benchmarks on your hardware";

  # flake.lock pins nixpkgs, and with it the exact llama.cpp build every
  # contributor runs. Updating it changes the runtime for all future results.
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
      packages = forAllSystems (
        pkgs:
        let
          # ponytail: Vulkan is the one GPU backend that covers AMD, Intel and NVIDIA
          # from a single build. Add CUDA/ROCm variants when Vulkan results prove unfair.
          llama = pkgs.llama-cpp.override { vulkanSupport = true; };
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "speedrun-evals";
            version = "0.1.0";
            # Only the files the build reads, so new result files do not trigger rebuilds.
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
                ./benchmarks
                ./models.toml
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            # Vulkan needs a userspace driver. The host's cannot be loaded into a Nix
            # program, so ship Mesa's (AMD, Intel, nouveau), pinned like everything else.
            # lvp is left out: it is a CPU rasteriser and would pose as a GPU.
            # ponytail: the proprietary NVIDIA driver is not covered; it has to match the
            # host kernel module, so it needs a nixGL-style wrapper. VK_DRIVER_FILES overrides.
            postInstall = ''
              drivers=$(ls ${pkgs.mesa}/share/vulkan/icd.d/*.json | grep -v lvp_ | paste -sd:)
              wrapProgram $out/bin/speedrun \
                --prefix PATH : ${pkgs.lib.makeBinPath [ llama ]} \
                --set-default VK_DRIVER_FILES "$drivers"
            '';
            meta = {
              license = pkgs.lib.licenses.agpl3Plus;
              mainProgram = "speedrun";
            };
          };
        }
      );

      apps = forAllSystems (pkgs: {
        site = {
          type = "app";
          program = "${self.packages.${pkgs.system}.default}/bin/speedrun-site";
        };
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
          ];
        };
      });
    };
}
