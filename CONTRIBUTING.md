# Contributing to Native Host Rust

Thank you for considering contributing to the `native-host-rust` project! This native messaging host is a critical component of the Aegis Http project, securely handling cryptographic operations via standard PC execution of GnuPG. Below are the basic guidelines for contributing.

## Development Environment Setup

1. **Install Rust:** Please install the latest stable version of [Rust](https://www.rust-lang.org/tools/install).
2. **Setup GnuPG:** You need GnuPG (`gpg`) available in your terminal `/PATH` to test functions.
3. **Build the Project:** Run standard Cargo build routines:
   ```bash
   cargo build
   ```

## Workflow

1. **Fork & Clone:** Fork the repository and clone to your local environment.
2. **Create a Branch:** `git checkout -b feature/your-feature-name`
3. **Implement Feature / Bug Fix:** When making changes to `main.rs`, be mindful that the application uses `stdin` and `stdout` directly to implement the Google Chrome Native Messaging protocol. Standard `println!` debugging will corrupt the messaging protocol and break the browser extension connection.
    - **Tip:** When debugging locally, use standard error `eprintln!` and write logs to `/tmp/aegis_http.log` or similar rather than standard output.
4. **Testing:** A test suite ensures the message protocol structure remains intact. Ensure you run:
   ```bash
   cargo test
   ```
   *Note: Integration tests might simulate a GnuPG environment. Check the `tests/` directory.*
5. **Code Quality:** Use standard formatting before committing:
   ```bash
   cargo fmt
   cargo clippy
   ```
6. **Submit a Pull Request:** Open a PR detailing the problem your change solves, how it works, and any relevant edge cases.

## Debugging Extension Issues

Since standard IO corresponds to the communication tunnel:
1. Load the extension in Chrome via `chrome://extensions` with Developer Mode enabled.
2. If the connection drops immediately, the browser likely failed to execute the binary (e.g., missing executable permissions via `chmod +x` or an invalid JSON manifest mapping in `/etc/opt/chrome/native-messaging-hosts/`).
3. If an action fails silently, inspect the extension's Background Worker Console for error status messages returned from Rust.
