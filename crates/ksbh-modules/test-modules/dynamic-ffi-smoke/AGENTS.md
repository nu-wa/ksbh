# dynamic-ffi-smoke

Test-only `cdylib` module used to exercise the real host-side dynamic loading path.

## Purpose

- built as a real shared library
- loaded by `ksbh-core` integration tests through `ModuleHost`
- exercises request parsing, session callbacks, metrics callbacks, response allocation, and response freeing

## Notes

- This crate is not part of the production module set.
- Keep behavior simple and deterministic so host-side tests can assert exact outputs.
- If the module type or exported behavior changes, update the matching host integration test in `crates/ksbh-core/tests/`.
