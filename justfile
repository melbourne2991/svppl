default: build

copy_bin: build_bin
    cp ./target/debug/cli ./cli/bin/

sync_third_party:
    cd cli && ./sync_binaries.sh

build: copy_bin

clean_sandbox: cd sandbox && rm -rf ./*

build_bin:
    cargo build --verbose
