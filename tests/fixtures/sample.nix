# Sample Nix file exercising extractor features.
{ pkgs ? import <nixpkgs> {} }:

let
  # Default port for the service.
  defaultPort = 8080;

  # Maximum retry count.
  maxRetries = 3;

  # Formats a log message.
  log = level: message:
    builtins.trace "[${level}] ${message}" null;

  # Builds a connection configuration.
  mkConnection = { host, port ? defaultPort, tls ? false }:
    {
      inherit host port tls;
      url = if tls
        then "https://${host}:${toString port}"
        else "http://${host}:${toString port}";
    };

  # Networking utilities.
  networking = {
    # Creates a connection pool.
    mkPool = { host, size ? 10 }:
      {
        inherit host size;
        connections = builtins.genList (_: mkConnection { inherit host; }) size;
      };

    # Validates a connection config.
    validateConfig = config:
      assert config.port > 0 && config.port < 65536;
      config;

    defaultConfig = {
      host = "localhost";
      port = defaultPort;
      tls = false;
    };
  };

in {
  inherit networking;
  inherit (networking) mkPool validateConfig;

  # Package definition.
  service = pkgs.stdenv.mkDerivation {
    pname = "my-service";
    version = "1.0.0";
    src = ./.;
  };
}
