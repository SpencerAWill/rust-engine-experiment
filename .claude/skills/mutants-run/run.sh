#!/bin/sh
# Usage: run.sh [--dry-run] [cargo xtask mutants flags...]
# Runs `cargo xtask mutants -j 1 <flags>` in a memory-capped Docker container with a host-side
# watchdog. --dry-run prints the docker commands and exits 0 without running them.

set -u

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
name=rust-engine-mutants

dry=0
if [ "${1:-}" = "--dry-run" ]; then
    dry=1
    shift
fi

# Single sources of truth: the toolchain file and the constant in xtask.
toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$root/rust-toolchain.toml")
mutants_version=$(sed -n 's/^pub(crate) const MUTANTS_VERSION: &str = "\(.*\)";/\1/p' "$root/xtask/src/mutants.rs")
if [ -z "$toolchain" ] || [ -z "$mutants_version" ]; then
    echo "run.sh: could not read the toolchain ('$toolchain') or the cargo-mutants version ('$mutants_version')" >&2
    exit 1
fi
image="rust-engine-mutants:$toolchain"

dockerfile() {
    cat <<DOCKERFILE
FROM debian:trixie
ARG TOOLCHAIN
ARG MUTANTS_VERSION
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:\$PATH
RUN apt-get update \\
 && apt-get install -y --no-install-recommends build-essential curl ca-certificates git pkg-config \\
 && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "\$TOOLCHAIN" \\
 && rustup component add rustfmt clippy
RUN case "\$(uname -m)" in aarch64) plat=linux-arm ;; *) plat=linux ;; esac \\
 && curl -LsSf "https://get.nexte.st/latest/\$plat" | tar zxf - -C /usr/local/cargo/bin \\
 || cargo install --locked cargo-nextest
RUN cargo install --locked cargo-mutants --version "\$MUTANTS_VERSION"
WORKDIR /workspace
DOCKERFILE
}

# show: print a command the way a shell would need it (arguments with spaces are not expected).
build_cmd="docker build --build-arg TOOLCHAIN=$toolchain --build-arg MUTANTS_VERSION=$mutants_version -t $image -"

if [ "$dry" = 1 ]; then
    echo "# image cached? docker image inspect $image"
    echo "$build_cmd   # Dockerfile piped on stdin by run.sh"
    echo "sh $here/watchdog.sh $name &"
    echo "docker run --rm --name $name --memory=4g --memory-swap=4g \\"
    echo "  -v $root:/workspace -v rust-engine-mutants-registry:/usr/local/cargo/registry \\"
    echo "  -v rust-engine-mutants-target:/cargo-target -e CARGO_TARGET_DIR=/cargo-target \\"
    echo "  -w /workspace $image cargo xtask mutants -j 1 $*"
    exit 0
fi

if ! docker info >/dev/null 2>&1; then
    echo "run.sh: Docker is not running (start Docker Desktop)" >&2
    exit 1
fi

if ! docker image inspect "$image" >/dev/null 2>&1; then
    echo "run.sh: building $image (first time only)" >&2
    dockerfile | $build_cmd || exit 1
fi

# A stale container with the fixed name would make `docker run` fail.
docker rm -f "$name" >/dev/null 2>&1

sh "$here/watchdog.sh" "$name" &
wd=$!
trap 'kill $wd 2>/dev/null' EXIT INT TERM

# The repo is mounted at /workspace, so xtask writes target/mutants straight into the host's
# target/mutants. CARGO_TARGET_DIR is a named volume, so builds never touch the host's target/.
docker run --rm --name "$name" --memory=4g --memory-swap=4g \
    -v "$root:/workspace" \
    -v rust-engine-mutants-registry:/usr/local/cargo/registry \
    -v rust-engine-mutants-target:/cargo-target \
    -e CARGO_TARGET_DIR=/cargo-target \
    -w /workspace "$image" \
    cargo xtask mutants -j 1 "$@"
status=$?

kill "$wd" 2>/dev/null
trap - EXIT INT TERM
exit "$status"
