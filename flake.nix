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
          # nixpkgs' llama.cpp is older than some models need (Inkling 2 Mini support
          # landed upstream on 2026-11-20), so the source is pinned here instead.
          # Bumping the build, commit or hash changes the runtime for all future results.
          llamaBuild = "12633";
          llamaCommit = "9c10c94e16ebc4f3ed7c06264a93bf7e019c3f26";
          llama = (pkgs.llama-cpp.override { vulkanSupport = true; }).overrideAttrs (old: {
            version = "b${llamaBuild}";
            src = pkgs.fetchFromGitHub {
              owner = "ggml-org";
              repo = "llama.cpp";
              rev = llamaCommit;
              hash = "sha256-IvorVwS9e2YKmgXPBUByL3T6ZLeNFEBCymA0M4YNORo=";
            };
            # The embedded chat app moved and is off by default upstream; the CLI only
            # needs the API, so skip the npm build nixpkgs still expects.
            npmDeps = null;
            # ponytail: upstream's test suite now downloads a model, which the build
            # sandbox forbids. Skip it; our own smoke test is `speedrun list` in CI.
            doCheck = false;
            preConfigure = "";
            nativeBuildInputs = builtins.filter (
              i: !(pkgs.lib.hasInfix "npm" (i.name or "") || pkgs.lib.hasInfix "nodejs" (i.name or ""))
            ) old.nativeBuildInputs;
            # nixpkgs bakes its own build number and commit into `--version`; results
            # record that string, so it has to name the source actually built.
            cmakeFlags =
              builtins.filter (f: !(pkgs.lib.hasInfix "LLAMA_BUILD_" f)) old.cmakeFlags
              ++ [
                (pkgs.lib.cmakeFeature "LLAMA_BUILD_NUMBER" llamaBuild)
                (pkgs.lib.cmakeFeature "LLAMA_BUILD_COMMIT" (builtins.substring 0 7 llamaCommit))
              ];
          });
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
