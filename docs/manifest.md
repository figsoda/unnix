# Manifest

`unnix.kdl` is unnix's manifest written in [KDL],
a document language with XML-like node semantics.
Here is what a node looks like:

```kdl
node1 arg1 prop1=val1 {
  node2
  node3 arg2
}
```

- `node1` is the name of the node.
- `arg1` is an argument.
- `prop1` is a property, and `val1` is its value.
- `node2` and `node3` are the children of `node1`.
  Child nodes with a single argument like `node3` are sometimes also called fields.
- Each node can have an arbitrary amount of arguments, properties, and children.

The manifest file is defined by a list a nodes signified by their name.
The same node can occur multiple times, where the later occurrences will merge into the earlier ones.
The following nodes are supported:

- Environment
  - [`caches`](#caches) - Binary caches to pull from, also known as substituters
  - [`env`](#env) - Environment variables
  - [`packages`](#packages) - Packages to pull into the environment

- Resolvers
  - [`devbox`](#devbox) - Resolver powered by [Devbox][Nixhub]
  - [`hydra`](#hydra) - Resolver powered by [Hydra]

- System-related options
  - [`system`](#system) - Per-system options
  - [`systems`](#systems) - The set of systems to support

## Environment

### `caches`

The list of binary caches unnix pulls from,
similar to the [`substituters` Nix setting][substituters].
When nars are downloaded, they are checked against a set of public keys,
Public keys can be added with the `public-keys` field,
which is equivalent to the [`trusted-public-keys` Nix setting][trusted-public-keys].

```kdl
caches {
  "https://nix-community.cachix.org"
  public-keys {
    "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
  }
}
```

By default, `https://cache.nixos.org` and its public key is included.
You can disable this behavior with `default=#false`.

```kdl
caches default=#false {
  "https://nix-community.cachix.org"
  public-keys {
    "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
  }
}
```

### `env`

You can set environment variables with `env`.

```kdl
env {
  CC gcc
  RUSTFLAGS "--cfg tokio_unstable"
}
```

The value of an environment variable can contain interpolations delimited by curly brackets.
`{x.y}` will expand to the path of `y` output of the package named `x`.
For example, `{libclang.lib}` expands to the `lib` output of `libclang`.

```kdl
packages {
  libclang lib
}

env {
  LIBCLANG_PATH "{libclang.lib}/lib" // /nix/store/<hash>-clang-<version>-lib/lib
}
```

The literal characters `{` and `}` can be escaped by repeating the same character twice.

```kdl
env {
  STUFF_IN_BRACES "{{stuff}}" // {stuff}
}
```

### `packages`

The set of packages to include in the environment.
Each child is a package with the following optional properties:

- `package` (string)

  Having `x package=y` allows you to download the package `y`,
  while still being able to refer to this package with the name `x` in [`env`](#env),
  or overwrite it in a later `packages` node.

- `resolver` (string)

  By default, all packages are pulled from the [resolver](#resolvers) named `default`.
  This property allows you to override the resolver of a package to something else.

Each package can have a set of arguments used to filter its outputs.
`x out lib` will only keep the `out` and `lib` outputs of `x`,
while `x` without any arguments will keep all arguments of `x`.

```kdl
packages {
  libclang // all outputs of libclang
  nix dev package=lix // only include the dev output of lix
  ripgrep resolver=my-resolver // use ripgrep from my-resolver
}

// Devbox resolver named my-resolver
devbox my-resolver
```

You can also change the default resolver for all child packages of a `packages` node.
This can be handy if you have a lot of packages from non-default resolvers.

```kdl
// Avoid writing python314Packages.<name> everywhere
packages resolver=python {
  numpy
  python
  scipy
}

// Hydra resolver named python
hydra python {
  base "https://hydra.nixos.org"
  project nixpkgs
  jobset unstable
  job "python314Packages.{package}.{system}" // changed
}
```

## Resolvers

Unnix uses resolvers to convert package names to store paths during lockfile generation.
By default, only one resolver named `default` is included,
which points to the [Hydra] jobset responsible for the [nixpkgs-unstable] branch.
You can override the default resolver to point to a different [Hydra] jobset,
or a different resolver like [`devbox`](#devbox).

```kdl
packages {
  jq // use jq from unstable
  ripgrep resolver=stable // use ripgrep from 25.11
}

system darwin {
  // https://hydra.nixos.org/jobset/nixpkgs/nixpkgs-25.11-darwin
  hydra stable {
    base "https://hydra.nixos.org"
    project nixpkgs
    jobset nixpkgs-25.11-darwin // changed
    job "{package}.{system}"
  }
}

system linux {
  // https://hydra.nixos.org/jobset/nixos/release-25.11
  hydra stable {
    base "https://hydra.nixos.org"
    project nixos // changed
    jobset release-25.11 // changed
    job "nixpkgs.{package}.{system}" // changed
  }
}
```

### `devbox`

[Devbox] is a third party tool built on top of Nix.
The command-line tool itself is open source, but the resolver API it uses is proprietary.
The `devbox` resolver used by unnix is based on this proprietary API.
It is usually significantly faster than the `hydra` resolver,
and also allows version pinning via the `@<version>` syntax.
You can look up what versions of packages are available on [Nixhub].

> [!Note]
> Some packages may not have the correct latest version, e.g. `nix`.
> See [upstream issue](https://github.com/jetify-com/devbox/issues/2555)

```kdl
packages {
  libclang lib // latest version of libclang
  nix@2.31.3 dev // specifically Nix 2.31.3
  pkg-config-unwrapped@latest // latest version of pkg-config (@latest is optional)
}

// overwrite the default resolver
devbox default
```

You can change the template string of the package with the `package` field.
Any occurrence of `{package}` gets expanded to the name of the package,
and `{system}` gets expanded to the system unnix is running on, e.g. `x86_64-linux`.

```kdl
packages resolver=python {
  matplotlib
  numpy
}

devbox python {
  package "python314Packages.{package}"
}
```

### `hydra`

A `hydra` resolver requires an argument for its name, and accepts the following fields:

- `base` (string) - URL base for the Hydra instance, e.g. `https://hydra.nixos.org`

- `project` (string) - Name of the Hydra project, e.g. [`nixpkgs`](https://hydra.nixos.org/project/nixpkgs)

- `jobset` (string) - Name of the Hydra jobset, e.g. [`unstable`](https://hydra.nixos.org/jobset/nixpkgs/unstable)

- `job` (optional string) - Template string for Hydra jobs, defaulting to `{package}.{system}` if unset

  Any occurrence of `{package}` gets expanded to the name of the package,
  and `{system}` gets expanded to the system unnix is running on, e.g. `x86_64-linux`.

This is the default resolver if no resolvers are specified.

```kdl
// https://hydra.nixos.org/jobset/nixpkgs/unstable
hydra default {
  base "https://hydra.nixos.org"
  project nixpkgs
  jobset unstable
  job "{package}.{system}"
}
```

For example, for the package `nix-init` on `x86_64-linux`,
the `job` will expand to `nix-init.x86_64-linux`,
and the resolver will look at <https://hydra.nixos.org/job/nixpkgs/unstable/nix-init.x86_64-linux>.

## System-related options

### `system`

`system` takes a system predicate as an argument,
and applies all child nodes to the systems specified by the system predicate.
A system predicate is a string that can be one of the following:

- architecture - `aarch64` or `x86_64`
- kernel - `darwin` or `linux`
- system (`<architecture>-<kernel>`), e.g. `x86_64-linux`

All [environment](#environment) and [resolver](#resolver) nodes are accepted.
Here is an example using [`packages`](#packages).

```kdl
packages {
  nix dev
}

// Use lix instead of nix on Linux
system linux {
  packages {
    nix dev package=lix
  }
}

// Only use faketty on aarch64-darwin
system aarch64-darwin {
  packages {
    faketty
  }
}
```

### `systems`

The set of systems to support, which is used during lockfile generation.
Defaults to {`aarch64-darwin`, `aarch64-linux`, `x86_64-linux`}.

```kdl
// Disable aarch64-linux support
systems {
  aarch64-darwin
  x86_64-linux
}
```

[Devbox]: https://github.com/jetify-com/devbox
[Hydra]: https://github.com/nixos/hydra
[KDL]: https://kdl.dev/
[Nixhub]: https://www.nixhub.io/
[nixpkgs-unstable]: https://github.com/nixos/nixpkgs/tree/nixpkgs-unstable
[substituters]: https://nix.dev/manual/nix/stable/command-ref/conf-file.html#conf-substituters
[trusted-public-keys]: https://nix.dev/manual/nix/stable/command-ref/conf-file.html#conf-trusted-public-keys
