# Rust HTTPS certificate distrust maintenance

Adapted from upstream [149ea51c / #5622](https://github.com/trycua/cua/commit/149ea51c613cc840c3760dec822c57b7e65df05a), authored by ddupont. Keep the driver Rust lock on rustls-webpki 0.103.16 and webpki-roots 1.0.10. This covers the shipped ureq/rustls HTTPS client (telemetry, update/skill fetch); browser, Node and native-tls trust engines are separate. No system trust-store modification.

The original public-certificate fixture and three applicable owners are retained in the driver integration test. Sixteen intermediate certificates cover eight distinct SPKIs and both issuer forms. The old lock fails the distrust and root-removal assertions. The updated lock passes all three owners and the 329 driver binary tests on macOS. Reverted unrelated tempfile/getrandom resolver churn. The upstream roots-0.26 owner is inapplicable to this driver's roots-1.x graph.

The fixtures contain public certificates, no private keys. Presenting these intermediates verifies the actual verifier policy denial (`CertDistrusted`) before issuer/role checks; this is not a live forged-leaf exploit reproduction. Fresh Linux archive qualification and stable-signed Mac runtime checks must precede OpenSky release publication. Existing 0.2.36 archives remain immutable and use the previous TLS lock until superseded.

Reproduce hermetically:

```sh
cd libs/cua-driver/rust
cargo test --locked -p cua-driver --test emudhra_distrust --bin cua-driver -- --test-threads=1
```
