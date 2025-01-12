# Installation

The most simple way to try it out is to download precompiled binary for your OS.

Shell completion rules can be generated using `investments completion` command.

**For Windows users**: please run the program under [Windows Terminal](https://aka.ms/terminal) instead of ancient [Command Prompt](https://en.wikipedia.org/wiki/Cmd.exe) because it doesn't support color output.

## Precompiled binaries

You can find precompiled binaries on [Releases](https://github.com/KonishchevDmitry/investments/releases) page.

[binup](https://github.com/KonishchevDmitry/binup) can be used to keep your installed version up-to-date.

## Cargo

1. Install Rust — https://www.rust-lang.org/tools/install
2. Install or upgrade the package:
```bash
cargo install investments
```
If it fails to compile and you installed Rust a long time ago, try `rustup update` to update Rust to the latest version.

If you want to install the package from sources, use:
```bash
git clone https://github.com/KonishchevDmitry/investments.git
cd investments
cargo install --path . --force
```

## Docker

1. Install or upgrade:
```bash
DOCKER_BUILDKIT=1 docker build --pull --build-arg CACHE_DATE="$(date)" -t investments https://raw.githubusercontent.com/KonishchevDmitry/investments/master/install.dockerfile
```
2. Run:
```bash
docker run --rm -t --user "$(id -u):$(id -g)" -v ~/.investments:/.investments investments
```
