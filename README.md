# Native Host Rust | En / [TR](README.tr.md)

![Logo](assets/logo.png)

This repository contains the Rust-based native messaging host for the Aegis Http browser extension. It acts as a secure bridge between the browser extension and the local GnuPG (`gpg`) command-line tool, enabling end-to-end encryption, decryption, and message signing capabilities without ever exposing private keys (GnuPG private keys) to the browser environment.

## Features

- **Native Messaging Protocol**: Communicates with Google Chrome, Chromium-based, and Firefox-based browsers using standard input/output with length-prefixed JSON payloads.
- **GPG Integration**: Wraps the local `gpg` executable to perform cryptographic operations.
- **Chunked Payload Delivery**: Reassembles large payloads sent from the extension in chunks (to bypass browser messaging limits) and can also reply in chunks.
- **Concurrency Control**: Implements file-based mutex locking (`/tmp/aegis_http_gpg.lock`) to prevent race conditions when multiple GPG processes attempt to run simultaneously.

## Supported Actions

- `list-keys`: Lists available GPG secret keys for signing/decryption (includes subkey encryption capability checks).
- `add-subkey`: Generates a new encryption subkey for a given GPG key to support secure E2E decryption.
- `sign`: Clear-signs a given challenge/payload using the local user's GPG key.
- `encrypt`: Encrypts a payload for a recipient's public key (supports stateless encryption using `public_key` armored block via `--recipient-file`).
- `decrypt`: Decrypts a GPG-encrypted payload.

## Prerequisites

- **Rust / Cargo**: You need Rust and Cargo installed to build this project.
- **GnuPG (`gpg`)**: The `gpg` command-line executable must be installed and available in your system's `PATH`.

## Building

To build the project in release mode:

```bash
cargo build --release
```

## Installation

To register the native messaging host with Google Chrome:

```bash
chmod +x install.sh
./install.sh
```

This will copy the built binary and the native messaging JSON manifest to the appropriate Google Chrome configuration directory.

## Architecture

1. The browser extension sends a JSON payload containing an `action` and data (or chunks for large data).
2. The Rust binary reads exactly the number of bytes specified by the 4-byte header.
3. The binary accumulates chunks if necessary.
4. It spawns a `gpg` process with the appropriate flags, passing payload data via pipes.
5. It captures the output and returns it formatted as a JSON response to the browser extension.
