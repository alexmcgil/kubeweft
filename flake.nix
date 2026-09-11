{
  description = "Kubeweft development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          QT_PLUGIN_PATH = "${pkgs.qt6.qtbase}/lib/qt-6/plugins";
          QML_IMPORT_PATH = "${pkgs.qt6.qtdeclarative}/lib/qt-6/qml";
          QML2_IMPORT_PATH = "${pkgs.qt6.qtdeclarative}/lib/qt-6/qml";
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            protobuf
            just
            pkg-config
            jq
            jdk17
            qt6.qtbase
            qt6.qtdeclarative
            check-jsonschema
          ];
        };
      });
}
