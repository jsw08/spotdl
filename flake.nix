{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = {
    self,
    flake-utils,
    naersk,
    nixpkgs,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = (import nixpkgs) {
          inherit system;
        };
        lib = pkgs.lib;

        naersk' = pkgs.callPackage naersk {};
      in let
        deps = [pkgs.pkg-config pkgs.systemd pkgs.alsa-lib];
      in {
        # For `nix build` & `nix run`:
        defaultPackage = naersk'.buildPackage {
          name = "guiders";
          src = ./.;
          buildInputs = deps;

          meta = {
            description = "Listens for joystick home button events.";
            mainProgram = "guiders";
            homepage = "https://github.com/jsw08/guiders";
            license = lib.licenses.mit;
            maintainers = [
              {
                email = "jurnwubben@gmail.com";
                github = "jsw08";
                githubId = "46420489";
                name = "Jurn Wubben";
              }
            ];
          };
        };

        # For `nix develop` (optional, can be skipped):
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [rustc cargo] ++ deps;
        };
      }
    );
}
