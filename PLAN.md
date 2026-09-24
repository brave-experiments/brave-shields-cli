# PLAN: Port `brave-shields-cli` from Rust to JavaScript

 Full rewrite of the Rust CLI (`src/`, ~3,300 lines + ~1,500 lines of integration tests) in
 TypeScript/ESM on Node.js. **No user-facing behavior may change**: identical commands, flags,
 output (JSON byte-for-byte where practical, table format up to minor rendering differences),
 error messages, and exit behavior. Slightly different `--help` formatting is acceptable.

 Out of scope for this plan but a design constraint throughout: the next task will expose the
 tool as an **ES module library**, so the port must separate pure logic and I/O from process
 concerns (stdout/stderr, exit codes, `process.exit`) from day one.

 ---

 ## 1. Target stack

 | Decision | Choice | Rationale |
 |---|---|---|
 | Runtime | Node.js >= 24 | Stable ESM, `node:test`, wide availability on macOS/Linux |
 | Language | TypeScript (strict) | Types mirror Rust's enums/optionality; compiles to clean ESM |
 | Module system | ESM only (`"type": "module"`) | Required for the upcoming library use case |
 | CLI parser | `commander` | Supports nested subcommands, global options, env-var fallbacks; `--help` differences acceptable |
 | Table renderer | `cli-table3` | Closest to `comfy-table` output; minor formatting drift allowed |
 | Test runner | `vitest` | Native TS/ESM, works for both unit and spawn-based integration tests |
 | Build | `tsc` → `dist/` | Zero magic, emits plain ESM consumable as a library later |

 Everything else is implemented with Node built-ins (`node:fs`, `node:crypto`,
 `node:readline`, `node:process`) or small focused deps (see mapping below). Keep the
 dependency list as small as possible.

 ## 2. Rust → JS module map

 One-to-one port. Keep file names and responsibilities parallel so review is easy.

 | Rust | JS (`src/`) | Notes |
 |---|---|---|
 | `src/main.rs` | `src/cli.ts` (bin) | Argument parsing + dispatch only; catches errors, prints, sets exit code. No logic. |
 | `src/platform.rs` | `src/platform.ts` | Channel enum, dir resolution, `resolveBraveDir`. Windows bail preserved. |
 | `src/chromium_time.rs` | `src/chromium-time.ts` | **BigInt** required — see §4.1 |
 | `src/output.rs` | `src/output.ts` | Keep JSON output byte-identical (§4.2); table format may drift slightly |
 | `src/preferences.rs` | `src/preferences.ts` | Read/navigate/atomic-write/lock/RMW, `checkBraveNotRunning` |
 | `src/local_state.rs` | `src/local-state.ts` | Mirror of preferences layer for `Local State` |
 | `src/profile.rs` | `src/profile.ts` | `listProfiles`, `resolveProfile`, `preferencesPath` |
 | `src/shields.rs` | `src/shields.ts` | Pure logic; port unit tests verbatim |
 | `src/scriptlets.rs` | `src/scriptlets.ts` | LevelDB access — see §4.4 |
 | `src/commands/*.rs` | `src/commands/*.ts` | `get`, `set`, `list`, `reset`, `profiles`, `filters`, `scriptlets`, `bisect` |
 | `tests/integration.rs` | `tests/integration.test.ts` | Port 1:1, spawn the built CLI instead of the Rust binary |

 ### Dependency mapping

 | Rust crate | Replacement | Notes |
 |---|---|---|
 | `clap` | `commander` | Replicate env var `BRAVE_SHIELDS_CLI_CHANNEL`, global flags, required positionals, `--pattern` flags, nested `filters`/`scriptlets` subcommands. Command/flag validation errors go to stderr with non-zero exit (details §4.6). |
 | `serde_json` | `JSON.parse` / `JSON.stringify` + sorted-key serializer | Must replicate BTreeMap key ordering — see §4.2 |
 | `chrono` | `Date.now()` + `BigInt` | §4.1 |
 | `comfy-table` | `cli-table3` | Table output may differ cosmetically (allowed) |
 | `sysinfo` | `ps-list` | `checkBraveNotRunning`: Exact-name match against the same process-name tables from `preferences.rs`. |
 | `fs2` | `fs-ext` | Node has no native `flock` utility. Used by the shared `lockedReadModifyWrite` helper and the scriptlets DB lock. |
 | `tempfile` | Manual temp-file + `fs.renameSync` | Write to `.<name>.tmp-<pid>-<rand>` in the same directory, then `renameSync` (atomic on POSIX). Matches `NamedTempFile::persist`. |
 | `ctrlc` | `process.on('SIGINT')` | Bisect's cooperative-interrupt flag |
 | `rusty-leveldb` | `classic-level` | Real LevelDB C++ bindings — interoperable with Chromium's DB (§4.4) |
 | `base64` | `Buffer` | `Buffer.from(s).toString('base64')` and back |
 | `anyhow` | `Error` subclasses | Preserve message strings exactly; CLI prints `Error: <message>` (§4.3) |

 ## 3. Port order and work breakdown

 ### Phase 0 — Scaffold
 - `package.json` (`type: module`, `bin`, `exports` map stub, scripts: `build`, `test`, `lint`), `tsconfig.json` (strict, `NodeNext`), `vitest.config.ts`.
 - Decision recorded in package.json: `engines.node >= 24`.
 - CI/build parity: `npm test` must be able to run unit tests without building the CLI; integration tests spawn `node dist/cli.js` after build.

 **Acceptance:** empty test run passes; `npx tsc --noEmit` clean.

 ### Phase 1 — Pure modules (no I/O)
 Port and unit-test: `shields.ts` (setting enum, `toStored`/`fromStored` tables, `resolvePattern`, `domainFromPattern`, domain validation incl. ASCII-only check), `chromium-time.ts`, `platform.ts` (channel dir names; mock `$HOME`), plus `output.ts` string builders.

 Rules established here, carried through the whole port:
 - Library modules **never** print, never `process.exit`, never read env at import time.
 - All output helpers build strings / take a writer; the CLI layer prints them.
 - Errors are thrown `PortError` (base class) subclasses with the *exact* Rust message strings.

 **Acceptance:** every `#[cfg(test)]` unit test from `platform.rs`, `shields.rs`, `chromium_time.rs`, and `profile.rs` ported and passing; `get`-format JSON snapshots match Rust output on identical fixtures.

 ### Phase 2 — Preferences / Local State layer
 - `readPreferences`, `getExceptions`/`getExceptionEntries` (+ `Mut` variants that create intermediates), `writePreferences` with the exact safety pipeline: create `Preferences.bak` only if absent → temp file → rename → read-back parse verification.
 - `lockedReadModifyWrite(path, modifyFn)` using `fs-ext`'s `flock()`; keep the existing documented caveat comment about lock-vs-rename semantics.
 - `checkBraveNotRunning(channel, force)`: process scan per §4.5, `--force` warning path with identical stderr text.
 - Same for `local-state.ts` (`readLocalState`, `getCustomFilters`, `writeLocalState`, `lockedReadModifyWrite`).

 **Acceptance:** port all unit tests from `preferences.rs` and `local_state.rs` (backup semantics, nested `cosmeticFilteringV2` format, RMW, missing-file errors).

 ### Phase 3 — Profile resolution + LevelDB scriptlets
 - `profile.ts`: last-used fallback, dir-name then display-name matching, error message listing available profiles verbatim.
 - `scriptlets.ts` on `classic-level`:
   - `open(path, { createIfMissing })`; read key `SCRIPTLETS` → JSON array of `{name, content(b64), kind:{mime}}`; base64 via `Buffer`.
   - Keep the sibling lock file (`AdBlock Custom Resources.lock`) around DB access — replicate `with_db_lock`, since we must coexist with the Chromium-side DB and the previous Rust behavior.
   - `writeScriptlets` replicates entry shape exactly (`kind: { mime: "application/javascript" }`, base64 content, compact JSON via `JSON.stringify` without spaces — Rust used `to_string`, i.e. compact).
   - `validateName` ported verbatim (empty/ASCII/path-traversal/`user-` prefix/`.js` suffix) with identical messages.

 **Acceptance:** unit tests from `scriptlets.rs` ported, including corrupted-DB and missing-field error cases. Bonus check: open a LevelDB created by the Rust build (`rusty-leveldb`) in the JS port and vice versa — write compatibility must be verified both directions before calling this phase done.

 ### Phase 4 — Commands
 Port in dependency order: `profiles`, `get`, `list`, `set`, `reset`, `filters`, `scriptlets`, then `bisect` last.

 - `filters.ts`: `run_list/add/remove/load/clear` + `writeAllFilters` helper. Preserve duplicate-detection (trimmed line comparison), "Filter not found" vs "Removed custom filter" messages, `load` counting non-empty lines.
 - `bisect.ts`: full interactive loop — state file schema (`bisect-state.json`: `original_filters`, `channel`, `all_rules`, `candidate_indices`, `step`), `@@`/`#@#` exception conversion (`makeException`), channel-match validation on resume, y/n/q prompting via `node:readline` with the same re-prompt on invalid input, SIGINT → save state + suspend, restore flow at the end. Note in code comments: bisect intentionally skips the
 not-running check.
 - Interactive stdin: use `node:readline` on `/dev/stdin`-equivalent (`process.stdin`); EOF ⇒ treat as `q` (matches Rust `lines().next() == None`).

 **Acceptance:** all unit tests from `bisect.rs` (`readRulesFromfile`, state save/load, `writeAndLoadFilters` variants) ported and passing.

 ### Phase 5 — CLI wiring
 - `commander` program mirroring `main.rs`:
   - Global flags on the program: `--profile`, `--channel` (env `BRAVE_SHIELDS_CLI_CHANNEL`), `--brave-dir`, `--format <json|table>` (default `json`, **validated manually** to reproduce the exact `unknown format '<x>'. Valid: json, table` error), `--force`.
   - Subcommands: `get <domain> [--pattern]`, `set <domain> <setting> <value> [--pattern]`, `list`, `profiles`, `reset <domain> [setting] [--pattern]`, `filters <list|add|remove|clear|load|bisect>`, `scriptlets <list|get|add|remove|clear>`.
   - Dispatch order preserved: `profiles` / `filters` / `scriptlets` skip profile resolution; the rest resolve profile then `preferencesPath`.
   - `resolveBraveDir` stderr messages ("Using Brave Release (...)", "Using custom Brave directory: ...") and precedence (`--brave-dir` > `--channel` > release).
 - Error/exit handling in one place: thrown errors → `Error: <message>` on stderr, exit 1; commander parse errors → usage on stderr, exit 2 (clap parity; see §4.6).

 **Acceptance:** `--help` output lists all commands/options; every command routes correctly (covered by integration tests).

 ### Phase 6 — Integration tests
 Port `tests/integration.rs` 1:1 into `tests/integration.test.ts`:
 - `setupFixture` builders (Local State + Preferences JSON) copied as-is.
 - `runCmd` / `runCmdWithStdin` spawn `node dist/cli.js` with `--brave-dir`/`--force`.
 - All ~90 tests: get/set/list/reset round-trips, ads triple-key writes, `last_modified` generation, `Preferences.bak` semantics, filters list/add/remove/load/clear, scriptlets add/get/remove/clear + LevelDB fixtures, bisect scripted sessions (stdin-fed y/n/q), pattern-mode, unknown-setting/value errors, `--format` handling.
 - Helper for running both output formats where the Rust suite does.

 **Acceptance:** full suite green; stderr/stdout assertions from the Rust suite pass unchanged (table-format assertions updated only where comfy-table rendering is inherently different).

 ### Phase 7 — Cleanup & docs
 - Update `README.md` install/usage section (`npm install -g` / `npx` instead of `cargo install`).
 - Remove Rust sources or move them out of the build path once the JS port is validated; archive `Cargo.toml`/`Cargo.lock` if we want a reference, otherwise delete `target/`.
 - Final manual smoke pass against a real Brave profile (read-only commands) on macOS and Linux.

 ## 4. Behavioral parity gotchas (must-handle list)

 1. **Chromium timestamps need BigInt.** Values are microseconds since 1601 (~1.3 × 10¹⁶), beyond `Number.MAX_SAFE_INTEGER` (9.0 × 10¹⁵). Compute `BigInt(Date.now()) * 1000n + 11644473600000000n` and `.toString()` — never use floating-point math.
 2. **JSON key ordering.** Rust `serde_json`'s default map is a `BTreeMap`, so everything the tool writes — Preferences/Local State files and `json!`-macro stdout output — has **alphabetically sorted keys**, and `to_string_pretty` uses 2-space indent. JS objects preserve insertion order, so implement `sortedStringify(value)` (recursively sort object keys, `JSON.stringify(v, null, 2)`) and use it for all file writes and all
 JSON stdout. This keeps rewritten files byte-compatible with the Rust tool (important: user-visible diffs and tests).
    - `get` output: build `settings` as a sorted map (cli names sort as: ads, fingerprinting, https-upgrade, scripts, shields).
 3. **Error messages and streams.** All user-facing strings (including `context(...)` chains flattened to their final message) must be reproduced exactly; diagnostics go to **stderr**, the found-rule/result payload of `bisect` and scriptlet content of `scriptlets get` go to **stdout**. Top-level failure prints `Error: <message>` and exits 1.
 4. **LevelDB interop.** Chromium reads this DB, so the JS port must use real LevelDB-compatible storage (`classic-level`), not a JS-only store. Handle: DB dir may not exist (return empty), corrupted `SCRIPTLETS` JSON must error, missing `name`/`content` fields must error, empty content stays empty string. Keep the sibling `.lock` file protocol (name: `AdBlock Custom Resources.lock`) so behavior matches the Rust tool.
 5. **Process detection.** Replicate `sysinfo` semantics: compare the process name **exactly** against the per-channel/per-OS name list (`Brave Browser` vs `brave-browser` etc.). On Linux, `/proc/<pid>/comm` is truncated to 15 chars — same source sysinfo reads, so a comm-based comparison reproduces current behavior (including its quirks for long names like `brave-browser-beta`). On macOS use `ps -axo comm` (first token) or
 `ps -axo command` basename matching; document which. Windows: names list is empty ⇒ never "running" (matches Rust `#[cfg]` behavior).
 6. **Exit codes.** clap exits `2` on usage errors and `1` on runtime errors; anyhow top-level errors print `Error: ...`. Match: commander `.exitOverride()` → print usage, `process.exitCode = 2`; caught app errors → stderr + `process.exitCode = 1`; success → 0. (Tests assert only success/failure, but keep codes aligned anyway.)
 7. **Bisect files.** `bisect-filters.txt` and `bisect-state.json` written to CWD (or `--output-dir`), identical names and JSON schema; state file written after every step and on quit/interrupt; cleanup on completion (restore → delete both files).
 8. **Atomic write details.** Temp file in the *same directory* as target (rename atomicity), create `.bak` only if it doesn't already exist, post-write re-read + JSON parse verification with the exact context messages.
 9. **`--format` validation happens in app code, not the parser** (Rust parses it as a plain `String` then calls `OutputFormat::from_str`). Don't let commander validate the choice, or the error text changes.
 10. **Env var only affects channel default.** `--channel` flag wins over `BRAVE_SHIELDS_CLI_CHANNEL`; an invalid env value must fail the same way clap's env handling does (error naming the variable). Precedence overall: `--brave-dir` > `--channel` > env > release-default.

 ## 5. Groundwork for the library conversion (next task)

 Design constraints to honor now so the follow-up is a refactor, not a rewrite:

 - **Two entry points**: `src/cli.ts` (bin, owns argv parsing, process streams, exit codes) and `src/index.ts` (public API surface). Nothing under `src/commands/` imports `process.exit`, reads `process.argv`, or writes to `process.stdout/stderr` directly — they *return* data or throw; `output.ts` renders strings.
 - **Interactive bisect** is the one intrinsically I/O-bound command: isolate its `prompt()` behind a small injectable `Asker` interface (stdin/stdout implementation in CLI, mockable in tests, replaceable by library consumers later).
 - Keep `resolveBraveDir` / `resolveProfile` as exported, separately callable steps rather than burying them inside `main()`.
 - Preserve pure-function seams that already exist in Rust (`shields.ts`, `chromium-time.ts`, pattern helpers) — those port directly to the library API.
 - `package.json` `exports` field defined from Phase 0 (pointing at `dist/index.js` + types), even before `src/index.ts` exists, so the packaging shape is settled early.

 ## 6. Risks

 | Risk | Mitigation |
 |---|---|
 | JSON byte-level diffs (key order, float formatting, unicode escaping) | Central `sortedStringify` used everywhere; integration tests assert exact file contents for writes |
 | Table formatting drift breaks tests | Rust suite's table assertions are mostly substring/structural; where exact, relax to structural assertions (explicitly allowed by the porting rules) |
 | Interactive bisect under vitest spawns | Port the stdin-scripting approach already used in `tests/integration.rs` (`runCmdWithStdin`) |
