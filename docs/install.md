# Installation

* The most simple way to try it out - to download precompiled binary for your OS.
* The most convenient way (in terms of regular updates) - to use Cargo.

## Precompiled binaries

You can find precompiled binaries on [Releases](https://github.com/KonishchevDmitry/investments/releases) page.

## Cargo

1. Install Rust — https://www.rust-lang.org/tools/install
2. Install or upgrade the package:
```
cargo install investments
```
If it fails to compile and you installed Rust a long time ago, try `rustup update` to update Rust to the latest version.

If you want to install the package from sources, use:
```
git clone https://github.com/KonishchevDmitry/investments.git
cd investments
cargo install --path . --force
```

## Docker

1. Install or upgrade:
```
DOCKER_BUILDKIT=1 docker build --pull --build-arg CACHE_DATE="$(date)" -t investments https://raw.githubusercontent.com/KonishchevDmitry/investments/master/install.dockerfile
```
2. Run:
```
docker run --rm -t --user "$(id -u):$(id -g)" -v ~/.investments:/.investments investments
```
