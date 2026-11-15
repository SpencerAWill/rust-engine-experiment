---
name: bench-run
description: Run wall-clock benchmarks on the maintainer's Mac so the numbers can be trusted - AC power, no sleep, no builds during the run, power log checked afterwards. Use before quoting or saving any Criterion (cargo xtask bench) result.
---

# Benchmark run

Wall-clock numbers are noisy (about 5% between runs on a quiet machine). A run is only usable if the machine stayed on AC power, awake and otherwise idle for all of it. The rules for comparing and quoting results are in CONTRIBUTING.md, "Benchmarks".

1. **Before.** Tell the maintainer the run is starting and roughly how long it takes; ask them to keep the laptop plugged in and leave it alone. Check `pmset -g batt` says AC power, and the load average (`uptime`) is below about 2.5. Close nothing yourself.
2. **Run.** `cargo xtask bench --save <name>` (or `spike-bench <topic> --save <name>`). It refuses to start on battery and runs under `caffeinate -i`. Note the start and end times. Run no builds, tests or other heavy commands until it finishes.
3. **After.** Read `pmset -g log` for the run's window. Any power-source change, sleep or wake inside it spoils the results in that window: do not quote them; re-run the affected benchmarks (`-- '<regex>'` with the same `--save` name overwrites only those).
4. **Ask** the maintainer whether they unplugged or used the machine heavily during the run, and check what they say against the log.
5. **Compare** with `cargo xtask bench-report --baseline <new> --against <old>`; treat differences under about 5% as noise, and prefer ratios within one run to comparisons across runs.

Record the conditions (machine, power, the log check) with the numbers, in `log.md` and wherever the numbers are quoted.
