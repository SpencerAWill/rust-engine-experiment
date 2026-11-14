---
name: Bug report
about: Something does not behave as documented
labels: bug
---

**What happened**

**What you expected** (cite the doc comment, spec section, or ADR if you can)

**Minimal reproduction** (a short program or failing test is ideal)

```rust

```

**Environment**

- `rustc --version`:
- OS and target (for example `aarch64-apple-darwin`, `wasm32-wasip1`):
- Commit or version:

**Does this touch a frozen format?** (PRNG output streams, `Handle::to_bits`, golden values.) These must be identical on every platform and in every build, so a difference between platforms or builds in them belongs here. If yes, say how.

If the same build on the same platform gave different results for the same inputs, use the **Determinism bug** template instead. For a security problem, do **not** open an issue; see SECURITY.md.
