# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate
follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] — 2026-09-21

First release.

- `PutObject`, `GetObject`, `HeadObject` and `DeleteObject`, signed with
  AWS Signature Version 4 in its S3 form.
- Presigned URLs for any of the four, with an expiry.
- Session tokens for temporary credentials.
- Path-style and virtual-hosted addressing, against any endpoint.
- `x-amz-checksum-sha256` on upload, and a caller-supplied payload hash so a
  body already hashed is not hashed again.
- Optional `reqwest` (async) and `ureq` (sync) transports; the core has no
  HTTP dependency.
- The signer reproduces the worked examples in the S3 documentation, and CI
  runs every operation against a MinIO server.

[Unreleased]: https://github.com/bas3line/s3lean/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bas3line/s3lean/releases/tag/v0.1.0
