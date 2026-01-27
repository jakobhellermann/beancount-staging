{
  description = "Beancount-staging - A tool for reviewing and staging beancount transactions";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        nativeBuildInputs = with pkgs; [
          rustc
          cargo
          pkg-config
        ];
        buildInputs = with pkgs; [
          openssl
        ];
        devInputs = with pkgs; [
          just
          nodejs
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ devInputs;
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "beancount-staging";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "beancount-parser-2.2.1" = "sha256-saXIom7ftZegTUQ7IyTfAqofJG0Yhprye3qUg20Hpzg=";
            };
          };
          env = {
            PYO3_PYTHON = "${pkgs.python3}/bin/python";
          };

          nativeBuildInputs = nativeBuildInputs ++ [
            pkgs.nodejs
            pkgs.npmHooks.npmConfigHook
            pkgs.installShellFiles
          ];

          inherit buildInputs;

          npmDeps = pkgs.fetchNpmDeps {
            src = ./crates/beancount-staging-web/frontend;
            hash = "sha256-CPeM7pBXrD+fnlqz9Om6SBaqCbLHED+n+DmunBMcqJY=";
          };

          npmRoot = "crates/beancount-staging-web/frontend";

          preBuild = ''
            npm run build --prefix crates/beancount-staging-web/frontend
          '';

          postInstall = ''
            installShellCompletion --cmd beancount-staging \
              --bash <(COMPLETE=bash $out/bin/beancount-staging) \
              --fish <(COMPLETE=fish $out/bin/beancount-staging) \
              --zsh <(COMPLETE=zsh $out/bin/beancount-staging)
          '';

          meta = with pkgs.lib; {
            description = "A tool for reviewing and staging beancount transactions";
            homepage = "https://github.com/jakobhellermann/beancount-staging";
            license = licenses.mit;
            maintainers = [ ];
          };
        };
      }
    );
}
