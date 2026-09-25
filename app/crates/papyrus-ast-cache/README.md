# papyrus-ast-cache

Disk and bundled-base-script cache for parsed ASTs and token streams.

On-disk names are `{game}-{path-md5}.iplatc`, namespaced by target game.
Entries are keyed by content MD5, mtime, and `MIN_COMPATIBLE_VERSION`.
Bump that floor only when the binary entry layout or the embedded AST
changes. The files are an internal cache, not a published interchange
format.

```sh
cargo test --manifest-path app/crates/papyrus-ast-cache/Cargo.toml
```
