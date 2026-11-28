# Release smoke tests

Create a stable Git tag that matches the version in Cargo.toml, then create a draft GitHub release for that tag. In Actions, dispatch **Release** with the tag and `publish_draft` enabled. The workflow pins the tag's commit, installs its packed crate on Windows, Linux and macOS, then builds and tests the native release executables. It publishes the draft after the matrix, signed Windows archive checks and checksums succeed. Crates.io, Homebrew and Scoop publication follow. Use this workflow to publish the draft.

Run **Packaged release smoke** on a candidate branch for preflight without publishing. The **Release** workflow with a blank tag also runs the packaging gate and native builds without uploading release assets. To recover Windows assets for an existing public release that supports the smoke suite, dispatch **Release** with its tag and leave `publish_draft` off.

To publish a tagged stable crate while Windows signing is pending, dispatch **Release** with the tag and `crate_only` enabled. The packaged smoke matrix still gates crates.io publication. Run the normal draft publication separately when signing is available. The release-drift check can report the missing public GitHub release in the interim.

Run locally from the source checkout with Python 3.12 or newer and the pinned Rust toolchain.

```sh
python3 -B scripts/release-smoke/install-package.py --source .
python3 -B scripts/release-smoke/smoke.py /absolute/path/to/tokensave
python3 -B scripts/release-smoke/install-package.py --registry-version 7.17.2
```

The first command packages a clean checkout, extracts the crate and installs from its contents. The second tests an existing binary, such as a source build or an extracted release archive. The third installs an already published version with Cargo's locked dependencies. Use `--lite` on the package installer for local iteration. The release gate tests default features.

The smoke runner uses temporary homes, Cargo state and Git repositories with spaces in their paths. It checks version output, initialization, repeated initialization with recovery instructions, sync before and after changes, symbol lookup, database verification, local Git hooks, and repeatable Cursor installation and uninstall with unrelated configuration present. `TOKENSAVE_USER_CONFIG_DIR` points machine config and lock files at the disposable home even when the OS resolves the profile directory independently of the environment.

Failed commands print the platform, arguments and output. CI keeps the relevant smoke log as an artifact on failure. The crates.io smoke job runs only after publication and reports registry or installation failures separately from the draft gate.
