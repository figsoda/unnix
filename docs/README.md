# Documentation

- [layout.md](./layout.md) explains the layout of the unnix root, where unnix puts its files.

- [manifest.md](./manifest.md) is a reference for `unnix.kdl`, the manifest file for unnix.

## Examples

Unnix has a few [end-to-end tests](../e2e) that can be used as examples.
They are tested from the repository root, with their entry points being `unnix.kdl` and an executable `test`.

- [binary](../e2e/binary) - Example using CLI tools like `deno`, `jq`, and `ripgrep`

- [library](../e2e/library) - Rust program using pkg-config and system libraries like curl

- [python](../e2e/python) - Python script with external dependencies like `numpy`
