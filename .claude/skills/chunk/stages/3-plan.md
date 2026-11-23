# Stage 3: plan

Goal: `plan.md`, from `docs/chunks/_template/plan.md`, with one step per commit. No wait follows.

1. List the paths the spec touches (files and globs) and its topics. Dispatch `lessons-researcher` with them. Put each returned ID in the plan's Lessons section as followed or deviated (with the reason); "None apply" if none.
2. Write the plan. Record only what a builder cannot decide alone: files, names and signatures, the test that fails first and how, the verify command and its expected output, the commit message. Do not restate the spec. Order steps so paths are final before files are written at them.
3. For each step, say who does it: `builder` for product and test code and scripts; main session for documentation, skills, agent files and configuration.
4. Self-check against the spec: every requirement in §5 has a step; every step's verify can fail.
5. Set the plan `Accepted (<date>)`; log; commit (`docs: add the <title> plan (<id>)`). Go to [stage 4](4-work.md).
