{
  description = "Polynotes - Real-time multilingual lecture transcription";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cmake
            pkg-config
            clang
            llvm
            libclang
            cargo
            rustc
            rust-analyzer
            tauri-cli
          ];

          buildInputs = with pkgs; [
            bun
            nodejs_18
            openssl
            webkitgtk_4_1
            librsvg
            libappindicator
            at-spi2-atk
            at-spi2-core
            gtk3
            gnome.adwaita-icon-theme
            cairo
            pango
            gdk-pixbuf
            alsa-lib
          ];

          RUSTFLAGS = "-C target-cpu=native";
          CARGO_HOME = "${pkgs.std.homeManager.homeDirectory}/.cargo";
          TAURI_HOME = "${pkgs.std.homeManager.homeDirectory}/.local/share/tauri";

          shellHook = ''
            echo "═══════════════════════════════════════"
            echo "  Polynotes Development Environment"
            echo "═══════════════════════════════════════"
            
            # Initialize git submodule if needed
            if [ ! -d "core/whisper.cpp/.git" ]; then
              echo "Initializing whisper.cpp submodule..."
              git submodule update --init --recursive
            fi
            
            # Download models if not present
            if [ ! -f "core/whisper.cpp/models/ggml-base.en-q5_1.bin" ]; then
              echo "Downloading whisper models..."
              cd core/whisper.cpp/models
              bash download-ggml-model.sh tiny.en-q5_1 || true
              bash download-ggml-model.sh base.en-q5_1 || true
              bash download-ggml-model.sh tiny-q5_1 || true
              bash download-ggml-model.sh base-q5_1 || true
              cd ../../../..
              echo "Models downloaded!"
            else
              echo "Models already present!"
            fi
            
            echo ""
            echo "Ready! Commands:"
            echo "  bun tauri dev     - Start development"
            echo "  cargo run --release --bin benchmark - Run benchmark"
            echo "  bun tauri build  - Build for production"
            echo ""
          '';
        };
      }
    );
}
