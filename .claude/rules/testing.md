# Testing rules

- Unit tests in `#[cfg(test)] mod tests`, moved to `foo/tests.rs` past ~150 lines; integration
  tests in `tests/`, public API only. Fixtures via `env!("CARGO_MANIFEST_DIR")`.
- Test bodies come from `core_geometry`'s primitives (`cuboid`, `ball`, …) and the crate's
  `#[cfg(test)] mod fixtures` — never built by hand, never in `lib.rs`.
- A test is named for the behaviour: `empty_mesh_has_no_aabb`, not `test_aabb`.
- Every error variant a function returns has a test matching the variant, never the message.
- Geometry is checked against closed-form bodies (unit cube, sphere of radius `r`, right
  triangle, regular tetrahedron), with a tolerance and a message citing the expected value.
- `proptest` for invariants: a closed mesh slices into closed contours, reversing a contour
  flips its area, a transform then its inverse is the identity.
- A hot path gets a `criterion` bench in `benches/` before it is optimised; no performance
  claim without its before and after.
- A bug fix comes with a test that fails before it. A reader of foreign bytes has a fuzz
  target; a crash lands as a fixture and a test in the owning crate (`fuzz/README.md`).
- A visual change retakes and commits the panel pictures:
  `cargo test -p encrust-app --features snapshots --lib snapshots` (ADR 0215).
- Test the behaviour, then stop: no test restating a getter. Never `#[ignore]` to get green —
  fix it, or delete it and say so.
- A run with nothing changed takes ~23 s; minutes means a needless rebuild — a library missing
  `[lib] doctest = false`, or a build script watching a missing path (ADR 0207).
