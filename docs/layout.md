# The unnix root

Unnix puts all of its runtime files under the unnix root,
usually `~/.cache/unnix` on Linux and `~/Library/Caches/unnix` on Darwin.
The unnix root contains 4 subdirectories:

- `lock/` - Since unnix does not have a daemon,
  this directory is used to make sure no duplicate downloads happen across multiple unnix instances.
  Files under `lock/` are safe to delete if no unnix instances are pulling dependencies.

- `references/` - Unnix downloads dependencies in an arbitrary order, so if unnix aborts early,
  some store entries might have references that are absent from the store.
  This directory caches the `References` field of `narinfo`s,
  and is used to make sure there are no missing references.
  Files under `references/` are safe to delete if no unnix instances are pulling dependencies.

- `store/` - The unnix store, which is unnix's equivalent to `/nix/store`.
  On Linux, `unnix env` uses [bubblewrap] to bind this directory to `/nix/store`.
  Files and directories directly under `store/` are safe to delete if no unnix instances are running.

- `tmp/` - Unnix creates temporary files for the atomicity of its filesystem operations.
  This directory is used instead of the operating system option to avoid cross-filesystem renames.
  Directories directly under `tmp/` are safe to delete if no unnix instances are pulling dependencies.

[bubblewrap]: https://github.com/containers/bubblewrap
