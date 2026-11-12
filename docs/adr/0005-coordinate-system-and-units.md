# ADR 0005: Coordinate system and units

Status: Accepted (2026-11-12)

## Context

Every engine needs one internal convention, and every asset pipeline brings a different one.
Conventions differ in which axis is up, which direction is "forward", and handedness, and the word
"forward" is itself ambiguous: a camera's forward and an asset's front are different things in
several tools. Getting this wrong shows up as mirrored models, upside-down imports, and sign
errors in math and physics that are expensive to find later.

## Decision

**Internal convention (fixed, not configurable).** Right-handed, with the axes of glTF 2.0:
+X right, +Y up. Units are meters, seconds, radians, and kilograms. Matrices follow glam:
column-major storage, column vectors, `M * v`.

**Two meanings of "forward", both named and never conflated** (`engine_math::axis`):

- **View forward = -Z.** The direction a default camera looks, with +X to the viewer's right and
  +Y up (glam's `look_to_rh`).
- **Asset front = +Z.** The direction an imported asset's front faces. glTF defines the front of an
  asset as +Z, so assets face a default camera; from the asset's own point of view its right is
  -X (glTF: "+Z forward, -X right").

`CoordinateSystem` avoids the word "forward" entirely. It describes a source convention purely as a
basis: which signed source axis corresponds to each engine axis.

**Why glTF's convention.** It gives zero-conversion import of glTF (the primary planned asset
format), works with glam's `*_rh` helpers and the Rust graphics ecosystem, and makes right-handed
math and physics formulas apply directly.

**2D.** The 2D world is the XY plane of the 3D convention (+X right, +Y up, Z is depth or layer,
meters), and rotation is an angle about +Z, counter-clockwise.

**Screen and UI space** is a separate space: +Y down, pixel units, origin top-left. It is reached
only through explicit conversion, which needs camera and viewport data, so those functions ship
with the camera in the rendering milestone and not in M1.

**Conversion at the edges only.** `engine_math::convert` converts vectors, rotations, scales, and
transforms from another convention into engine space (and back). Rotations convert as `M * R * Mᵀ`,
a proper rotation even when handedness flips. Converting positions and rotations is not enough for
a left-handed source: the importer must also reverse triangle winding.

**Preset rule.** A preset maps the source's up axis to engine +Y and its asset-front axis to engine
+Z. The remaining axis follows from handedness; when handedness flips, the flipped axis is the
asset-right axis, matching common glTF importers.

**Presets are only provided when they can be verified.** Each preset cites vendor documentation
and the glTF importer or exporter behavior of the tool. A preset that cannot be verified is omitted
rather than guessed; any convention can still be built with `CoordinateSystem::new`.

| Preset | Mapping (`x`, `y`, `z`) | Handedness | Status |
|---|---|---|---|
| `ENGINE`, `GLTF` | `+X`, `+Y`, `+Z` (identity) | right | **Verified** |
| `UNITY` | `-X`, `+Y`, `+Z` | left | **Verified** |
| `BLENDER` | omitted | | **Not verified** |
| `UNREAL` | omitted | | **Not verified** |

**`GLTF`.** The glTF 2.0 specification defines a right-handed system with +Y up, +Z forward, and
-X right, with the front of an asset facing +Z
([spec](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)).

**`UNITY`.** Unity's manual states a left-handed system where +X is right, +Y is up, and +Z is
forward ([Quaternions and Euler rotations](https://docs.unity.cn/ja/Manual/QuaternionAndEulerRotationsInUnity.html);
[`Transform.forward`](https://docs.unity3d.com/ScriptReference/Transform-forward.html)). Unity's up
and front are therefore +Y and +Z, which are already the engine's up and asset front, and the
handedness flip negates the asset-right axis: Unity's +X is engine -X. That matches the Khronos
UnityGLTF importer, which handles the change by inverting the x component of positions and
directions ([UnityGLTF issue 257](https://github.com/KhronosGroup/UnityGLTF/issues/257)). The
mapping is therefore `x: NegX, y: PosY, z: PosZ`. A test checks that Unity's up and front land on
engine +Y and +Z and that the preset is left-handed.

**`BLENDER` (omitted).** Blender is right-handed with +Z up, and its glTF exporter's "+Y Up" option
converts to glTF's up axis by a rotation about X
([Blender glTF manual](https://docs.blender.org/manual/en/3.5/addons/import_export/scene_gltf2.html)).
What could not be verified is the direction an asset's front faces: the documentation I could reach
describes the Front viewpoint by axis (Y) without a sign
([viewpoint manual](https://docs.blender.org/manual/en/3.1/editors/3dview/navigate/viewpoint.html)),
and secondary sources contradict each other on its sign. The preset rule needs exactly that axis, so
the preset is omitted.

**`UNREAL` (omitted).** Unreal is left-handed with +Z up, which could be confirmed from secondary
sources, but Epic's glTF exporter documentation covers scale only
([exporter docs](https://dev.epicgames.com/documentation/unreal-engine/exporting-unreal-engine-content-to-gltf)),
and the exporter's axis handling (the source of truth for the asset-right axis) could not be found.
The preset is omitted.

**Adding a preset later.** Verify the up axis, the asset-front axis, and the handedness against
vendor documentation or by exporting a known asset through the tool's glTF path and inspecting the
result, then add the constant together with a test in `tests/convert.rs`, and record the evidence
here.

## Consequences

- glTF assets import with no conversion, and engine math reads the same as the glTF spec.
- Camera and asset directions have distinct names, which removes a common source of sign errors.
- Importers from left-handed tools must reverse triangle winding; `convert` documents this, but
  it cannot enforce it for mesh data it never sees.
- Two requested presets are absent until someone verifies them. Users of those tools can build the
  convention by hand with `CoordinateSystem::new`.
