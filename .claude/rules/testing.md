# Testing rules

- Unit tests sit next to the code in `#[cfg(test)] mod tests`; integration tests live in the
  crate's `tests/` and use only the public API. Fixtures resolve through
  `env!("CARGO_MANIFEST_DIR")`, never a relative path.
- Name a test after the behaviour it pins down: `empty_mesh_has_no_aabb`, not `test_aabb`.
- Every error variant a function can return has a test that triggers it, matching on the
  variant and never on the text of a message.
- Geometry is tested against bodies with a closed-form answer — a unit cube, a sphere of radius
  `r`, a right triangle, a regular tetrahedron — with an explicit tolerance and an assertion
  message saying where the expected value comes from.
- `proptest` covers invariants: a closed mesh slices into closed contours, reversing a contour
  flips its signed area, a transform followed by its inverse is the identity.
- A hot path gets a `criterion` benchmark in `benches/` *before* it is optimised. Without a
  before-and-after number from it, no performance claim and no optimisation.
- A bug fix comes with a test that fails before it. A reader of bytes somebody else wrote gets
  a fuzz target in `fuzz/`, and a crash it finds lands as a fixture and a test in the owning
  crate: `fuzz/README.md`.
- Test what the behaviour needs and stop. No test that only restates a getter.
- `cargo test --workspace` passes before a step is done. Never `#[ignore]` a test to get there:
  fix it, or delete it and say so.
