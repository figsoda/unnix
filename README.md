# unnix

Use Nix packages without installing Nix

[![release](https://img.shields.io/github/v/release/figsoda/unnix?logo=github&style=flat-square)](https://github.com/figsoda/unnix/releases)
[![version](https://img.shields.io/crates/v/unnix?logo=rust&style=flat-square)](https://crates.io/crates/unnix)
[![deps](https://deps.rs/repo/github/figsoda/unnix/status.svg?style=flat-square&compact=true)](https://deps.rs/repo/github/figsoda/unnix)
[![license](https://img.shields.io/badge/license-MPL--2.0-blue?style=flat-square)](https://www.mozilla.org/en-US/MPL/2.0)
[![ci](https://img.shields.io/github/actions/workflow/status/figsoda/unnix/ci.yml?label=ci&logo=github-actions&style=flat-square)](https://github.com/figsoda/unnix/actions/workflows/ci.yml)

## Usage

To use unnix, start by creating `unnix.kdl`, unnix's manifest file written in [KDL]

```
unnix init -p jq ripgrep
```

Now you can enter the environment with

```bash
unnix env
```

This will generate `unnix.lock.json` and put you in a shell with `jq` and `rg`.
Make sure to commit the lockfile to your VCS to keep your environment reproducible.

## How it works

This is a very simplified view of what happens when you run `nix develop`.

```mermaid
flowchart TB

nix(nix) --> |&nbsp;download&nbsp;| expr --> |&nbsp;evaluate&nbsp;| drv
drv --> |&nbsp;outputs&nbsp;| path --> |&nbsp;query&nbsp;| cache

cache -->|&nbsp;hit&nbsp;| download(download to the nix store)
cache -->|&nbsp;miss&nbsp;| build(build the derivation)
drv -.- build

cache(
  binary cache
  e.g. cache.nixos.org
)

drv(derivations)

expr(
  nix expressions
  e.g. nixpkgs
)

path(store paths)
```

Downloading and evaluating nixpkgs can take a long time,
especially in ephemeral environments like CI pipelines.
Unnix avoids that by removing derivations from the picture altogether,
and getting the store paths directly from CI systems like [hydra].

```mermaid
flowchart TB
unnix --> |&nbsp;lockfile absent or outdated&nbsp;| update --> |&nbsp;hydra&nbsp;| paths
unnix --> |&nbsp;up-to-date lockfile&nbsp;| paths
paths --> |&nbsp;query&nbsp;| cache

cache --> |&nbsp;hit&nbsp;| download(download to the unnix store)
cache --> |&nbsp;miss&nbsp;| miss

cache(
  binary cache
  e.g. cache.nixos.org
)

miss(
  fail
)

paths(store paths)

unnix(unnix)

update(update lockfile)
```

## Limitations

- No setup hooks

  Unnix does not have access to stdenv, and therefore cannot run the setup hooks
  or any user-specified `shellHook`s.
  Instead, it tries its best to emulate the behavior of setup hooks like `pkg-config`,
  so that dependencies can be picked up without executing any hooks.

- No evaluation

  Unnix does not evaluate Nix expressions.
  You cannot use `.override` or `.overrideAttrs` on packages,
  and are limited to the attributes your CI systems expose, e.g. [hydra] jobs.

- No builds

  Unnix cannot build anything, and strictly relies on binary caches.
  This means no unfree packages if you are using the default set of caches.

## Related projects

- [nix-bundle](https://github.com/nix-community/nix-bundle)

- [runix](https://github.com/timbertson/runix)

## Changelog

See [CHANGELOG.md](CHANGELOG.md)

[KDL]: https://kdl.dev/
[hydra]: https://github.com/nixos/hydra
