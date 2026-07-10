#!/bin/bash
set -euo pipefail
source /mnt/disk1/projects/aegis-http/native-host-rust/parts/aegis-host/run/environment.sh
set -x
if cargo read-manifest --manifest-path "."/Cargo.toml > /dev/null; then
    cargo install -f --locked --path "." --root "/mnt/disk1/projects/aegis-http/native-host-rust/parts/aegis-host/install" 
    # remove the installation metadata
    rm -f "/mnt/disk1/projects/aegis-http/native-host-rust/parts/aegis-host/install"/.crates{.toml,2.json}
else
    # virtual workspace is a bit tricky,
    # we need to build the whole workspace and then copy the binaries ourselves
    pushd "."
    cargo build --workspace --release 
    # install the final binaries
    find ./target/release -maxdepth 1 -executable -exec install -Dvm755 {} "/mnt/disk1/projects/aegis-http/native-host-rust/parts/aegis-host/install" ';'
    # remove proc_macro objects
    for i in "/mnt/disk1/projects/aegis-http/native-host-rust/parts/aegis-host/install"/*.so; do
        readelf --wide --dyn-syms "$i" | grep -q '__rustc_proc_macro_decls_[0-9a-f]*__' &&                         rm -fv "$i"
    done
    popd
fi                
