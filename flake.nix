{
  description = "COSMIC panel applet for system and Flatpak updates";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          # Loaded at runtime by winit/wgpu, needed for `just run`.
          runtimeLibs = with pkgs; [
            wayland
            libxkbcommon
            libglvnd
            vulkan-loader
          ];
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              cargo
              rustc
              clippy
              rustfmt
              rust-analyzer
              just
              pkg-config
            ];

            buildInputs = runtimeLibs;

            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
          };
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
