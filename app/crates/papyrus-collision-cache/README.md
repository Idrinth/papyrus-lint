# papyrus-collision-cache

Content-hash cache used to detect conflicting script copies.

Files are `{game}-{sha256(filename)}.iplcc` next to the AST cache. A
stored digest is reused while its mtime still matches, so the `.psc` is
not opened just to hash it. Callers record hashes from source already in
memory and flush dirty files at parse-end.

```sh
cargo test --manifest-path app/crates/papyrus-collision-cache/Cargo.toml
```
