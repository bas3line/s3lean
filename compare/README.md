# The comparison behind the numbers

Two programs that each sign and send one `PutObject` on tokio with rustls.
`aws/` uses the official SDK configured as lean as it gets; `lean/` uses this
crate with its `reqwest` feature. Build each with `cargo build --release`,
then compare `cargo tree -e normal --prefix none | sort -u | wc -l` and the
size of the binary in `target/release/`.

They are not part of the published package and not part of the workspace, so
they build only when you ask them to.
