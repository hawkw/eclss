# /!\ EXTREMELY SERIOUS WARNING /!\
#
# For some absurd reason, this will fail if a system-wide `clang` installation
# exists. See here for details:
# https://github.com/esp-rs/esp-idf-template/issues/64#issuecomment-1303669233
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    nixpkgs-esp-dev.url = "github:mirrexagon/nixpkgs-esp-dev";
  };

  description = "flake for ESP32-c3 Rust development";

  outputs = { self, flake-compat, nixpkgs, flake-utils, nixpkgs-esp-dev, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ nixpkgs-esp-dev.overlay ];
        pkgs = import nixpkgs { inherit system overlays; };
      in {
        devShell = pkgs.mkShell rec {
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          buildInputs = with pkgs; [
            # rust tools
            rustup
            rust-analyzer
            cargo-generate
            # rust ESP tools
            cargo-espflash
            cargo-espmonitor

            # esp-idf-sys dependencies:
            cmake
            ninja
            python3Packages.python
            python3Packages.pip
            python3Packages.virtualenv
            (gcc-riscv32-esp32c3-elf-bin.override {
              version = "2021r2-patch5";
              hash = "sha256-99c+X54t8+psqOLJXWym0j1rOP0QHqXTAS88s81Z858=";
            })
          ];
          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";
        };
      });
}
