let
  nixpkgs-esp-dev = builtins.fetchGit {
    url = "https://github.com/mirrexagon/nixpkgs-esp-dev.git";

    # Optionally pin to a specific commit of `nixpkgs-esp-dev`.
    # rev = "<commit hash>";
  };

  esp-dev = final: prev:
    let
      # mach-nix is used to set up the ESP-IDF Python environment.
      mach-nix = import (builtins.fetchGit {
        url = "https://github.com/DavHau/mach-nix";
        ref = "refs/tags/3.5.0";
      }) {
        # optionally bring your own nixpkgs
        pkgs = final;

        # optionally specify the python version
        # python = "python38";

        # optionally update pypi data revision from https://github.com/DavHau/pypi-deps-db
        # pypiDataRev = "some_revision";
        # pypiDataSha256 = "some_sha256";
      };
    in {
      # ESP32C3
      gcc-riscv32-esp32c3-elf-bin =
        prev.callPackage "${nixpkgs-esp-dev}/pkgs/esp32c3-toolchain-bin.nix"
        { };
      # ESP32
      gcc-xtensa-esp32-elf-bin =
        prev.callPackage "${nixpkgs-esp-dev}/pkgs/esp32-toolchain-bin.nix" { };
      openocd-esp32-bin =
        prev.callPackage "${nixpkgs-esp-dev}/pkgs/openocd-esp32-bin.nix" { };

      esp-idf = prev.callPackage "${nixpkgs-esp-dev}/pkgs/esp-idf" {
        inherit mach-nix;
      };

      # ESP8266
      gcc-xtensa-lx106-elf-bin =
        prev.callPackage "${nixpkgs-esp-dev}/pkgs/esp8266-toolchain-bin.nix"
        { };
      crosstool-ng-xtensa =
        prev.callPackage "${nixpkgs-esp-dev}/pkgs/crosstool-ng-xtensa.nix" { };
      gcc-xtensa-lx106-elf =
        prev.callPackage "${nixpkgs-esp-dev}/pkgs/gcc-xtensa-lx106-elf" { };
    };

  pkgs = import <nixos-22.05> { overlays = [ esp-dev ]; };
in pkgs.mkShell {
  name = "esp-idf-rust";

  buildInputs = with pkgs; [
    gcc-xtensa-esp32-elf-bin
    esp-idf

    esptool

    # Tools required to use ESP-IDF.
    git
    wget
    gnumake

    flex
    bison
    gperf
    pkgconfig

    cmake
    ninja

    ncurses5

    # rust
    rustup
    rust-analyzer
    cargo-nextest
    cargo-generate

    # rust ESP32 tools
    cargo-espflash
    cargo-espmonitor

    # other devtools
    just
  ];
}
