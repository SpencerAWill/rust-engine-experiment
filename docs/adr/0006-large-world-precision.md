# ADR 0006: Large-world precision

Status: Accepted (2026-11-12)

## Context

Planet-scale and space simulations need positions far from the origin. A single-precision float has
24 bits of mantissa, so its spacing grows with magnitude: at 1e9 m it is about 64 m, and objects
visibly jitter. Using `f64` for everything wastes memory and bandwidth for data that does not need
it (rotations and scales are bounded in magnitude, and GPU data is single precision), and changes
the layout of every type.

## Decision

**Split by role.** Absolute placement uses `WorldTransform`, whose translation is `f64`; rotation
and scale stay `f32`, because their magnitudes are bounded and do not lose precision far from the
origin. Local and relative data (parent-to-child offsets, GPU data, physics-local space) uses
`Transform`, which is entirely `f32`. The 2D types mirror this: `WorldTransform2d` and
`Transform2d`.

**Narrowing is explicit.** `f64` to `f32` happens only through
`WorldTransform::relative_to(origin)`, which subtracts a nearby origin in `f64` and then narrows.
There is no `From` impl, so precision loss is always visible at the call site. The inverse is
`Transform::to_world(origin)`.

**Precision guarantee (property-tested).** For any absolute position up to 1e9 m from the origin
(past the Earth-Moon distance) and a local offset within 10 km of the chosen origin, `relative_to`
followed by `to_world` round-trips with an error of at most 1 mm. At a 10 km offset the `f32`
spacing is about 1 mm, so this is essentially the limit of the representation. The same guarantee
holds in 2D.

**Layouts are pinned.** `engine_math` asserts `size_of` and `align_of` of the four transform types
with `const` assertions, so a change fails the build instead of silently changing snapshot or FFI
layouts. The values are the same on every target measured:

| Type | Size | Align | Measured on |
|---|---|---|---|
| `Transform` | 48 | 16 | aarch64 (native), wasm32 |
| `WorldTransform` | 64 | 16 | aarch64 (native), wasm32 |
| `Transform2d` | 20 | 4 | aarch64 (native), wasm32 |
| `WorldTransform2d` | 32 | 8 | aarch64 (native), wasm32 |

glam keeps `Quat` 16-byte aligned on every target unless its `scalar-math` feature is on, which is
why the 3D sizes do not vary between SIMD and non-SIMD targets. x86_64 is pinned by the same
assertions, which CI compiles on ubuntu-latest. The `const` assertions were checked to fail when
glam's `scalar-math` is enabled.

**The `scalar-math` hazard and upgrade path.** Cross-platform bit-exact determinism (ADR 0003) will
need three changes together: back `engine_math::scalar` with the pure-Rust `libm` crate, enable
glam's `libm` feature, and enable glam's `scalar-math` feature. `scalar-math` changes the size and
alignment of `Vec3A`, `Vec4`, `Quat`, `Mat3A`, and `Mat4`, and therefore of every transform here.
That breaks anything that depends on the layout (C# bindings, snapshots, serialized data), the same
failure mode as Unreal's Large World Coordinates migration. It is a deliberate, versioned,
engine-wide decision, not a Cargo feature, and the layout assertions make an accidental flip loud.
glam's `fast-math` cannot be detected and is documented as unsupported.

**What M1 does not decide.** Which of these types the ECS stores per entity is decided in M2. M1
provides the vocabulary.

**Guidance for M2 (not a decision).** `Transform` is a composition and interchange type, not a
good default component for every entity. It is 48 bytes with 16-byte alignment, so an entity that
only has a position (a particle, a pickup, a 2D agent) pays four to six times the memory of a bare
`Vec3` or `Vec2`, and a memory-bound loop over many such entities reads that much more per frame.
Data-oriented layouts split components by access pattern, so the expected shape is:

- a small `Position` component (`Vec3`, or `Vec2` in 2D) for entities that only have a location;
- separate `Rotation` and `Scale` components added only to entities that use them;
- `Transform` (or `WorldTransform`) built on demand where composition is needed: scene graphs,
  parent-to-child offsets, GPU upload, and import/export.

M2 should confirm this with its position-only benchmark (the 2D-heavy workload already planned),
because the right split depends on how often systems read the parts together.

## Consequences

- Positions stay precise at planetary scale without paying `f64` cost for rotation, scale, or
  local data.
- Code that mixes frames must choose an origin and call `relative_to` deliberately.
- A glam upgrade or feature flip that changes layout fails to compile until this ADR and the
  assertions are updated together.
