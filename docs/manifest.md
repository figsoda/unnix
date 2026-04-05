# Manifest

`unnix.kdl` is unnix's manifest written in [KDL].

```kdl
// The list of systems to support
// Defaults to [aarch64-darwin, aarch64-linux, x86_64-linux]
systems {
  aarch64-darwin
  x86_64-linux
}

// The list of packages to include in the environment
packages {
  libclang // all outputs of libclang
  nix dev // only include the dev output of nix

  // Use the unwrapped version of pkg-config, but keep it under the same name
  // The name can be referenced in the env section or overwritten in a later packages block
  pkg-config package=pkg-config-unwrapped
}

// Environment variables
env {
  // {libclang.lib} expands to the path to the lib output of libclang
  LIBCLANG_PATH "{libclang.lib}/lib"
}

// By default, cache.nixos.org is the only cache included
caches {
  "https://nix-community.cachix.org"
  public-keys {
    "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
  }
}
// You can disable the default cache with default=#false
caches default=#false {
  "https://nix-community.cachix.org"
  public-keys {
    "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
  }
}

// All previous options can be system-specific
system linux {
  // Append git to the list of packages on linux
  packages {
    git
  }
}
system aarch64 {
  env {
    IS_AARCH64 "1"
  }
}
system aarch64-darwin {
  // Use lix instead of nix on aarch64-darwin
  packages {
    nix dev package=lix
  }
}
```

## Resolvers

Unnix uses resolvers to convert package names to store paths during lockfile generation.
By default, only one resolver named `default` is included,
which points to the [Hydra] jobset responsible for the [nixpkgs-unstable] branch.

```kdl
// https://hydra.nixos.org/jobset/nixpkgs/unstable
hydra default {
  base "https://hydra.nixos.org"
  project nixpkgs
  jobset unstable
  job "{package}.{system}"
}
```

You can override the default resolver to point to a different jobset,
or a different [Hydra] instance altogether.

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

You can also change the default resolver for a set of packages,
which can be useful for things like Python dependencies.

```kdl
packages resolver=python {
  numpy
  python
  scipy
}

hydra python {
  base "https://hydra.nixos.org"
  project nixpkgs
  jobset unstable
  job "python314Packages.{package}.{system}" // changed
}
```

### Devbox

[Devbox] is a third party tool built on top of Nix.
The command-line tool itself is open source, but the resolver API it uses is proprietary.
The `devbox` resolver used by unnix is based on this proprietary API.
It is usually significantly faster than the `hydra` resolver,
and also allows version pinning via the `@<version>` syntax.

> [!Note]
> Some packages may not have the correct latest version, e.g. `nix`.
> See [upstream issue](https://github.com/jetify-com/devbox/issues/2555)

```kdl
packages {
  libclang lib // latest version of libclang
  nix@2.31.3 dev // specifically Nix 2.31.3
  pkg-config-unwrapped@latest // latest version of pkg-config (@latest is optional)
}

// The same trick works for the devbox resolver too
packages resolver=python {
  matplotlib
  numpy
}

devbox default

devbox python {
  package "python314Packages.{package}"
}
```

[Devbox]: https://github.com/jetify-com/devbox
[Hydra]: https://github.com/nixos/hydra
[KDL]: https://kdl.dev/
[Nixhub]: https://www.nixhub.io/
[nixpkgs-unstable]: https://github.com/nixos/nixpkgs/tree/nixpkgs-unstable
