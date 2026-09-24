import { PortError } from '../errors.js';
import { describeIoError } from '../fsutil.js';
import { sortedStringify } from '../json.js';
import type { JsonValue } from '../json.js';
import * as fs from 'node:fs';
import * as nodePath from 'node:path';
import { readLocalState } from '../local-state.js';
import type { Channel } from '../platform.js';
import { parseChannel } from '../platform.js';
import { writeAllFilters } from './filters.js';

// Note: bisect does not call check_brave_not_running before writing filters.
// This is intentional: the interactive loop explicitly asks the user to restart
// the browser between steps, so the tool writes while Brave may be running and
// relies on the user to restart for changes to take effect.

export function channelToStr(channel: Channel): string {
  return channel;
}

export function channelFromStr(s: string): Channel {
  const parsed = parseChannel(s);
  if (parsed === null) {
    throw new PortError(`unknown channel '${s}'`);
  }
  return parsed;
}

export interface BisectState {
  original_filters: string;
  channel: string;
  all_rules: string[];
  candidate_indices: number[];
  step: number;
}

export function bisectStateToJson(state: BisectState): JsonValue {
  return {
    original_filters: state.original_filters,
    channel: state.channel,
    all_rules: state.all_rules,
    candidate_indices: state.candidate_indices,
    step: state.step,
  } as JsonValue;
}

export function bisectStateFromJson(v: unknown): BisectState {
  if (v === null || typeof v !== 'object' || Array.isArray(v)) {
    throw new PortError('invalid bisect state file');
  }
  const record = v as Record<string, unknown>;

  const originalFilters = record['original_filters'];
  if (typeof originalFilters !== 'string') {
    throw new PortError('missing original_filters');
  }
  const channel = record['channel'];
  if (typeof channel !== 'string') {
    throw new PortError('missing channel');
  }
  const allRulesRaw = record['all_rules'];
  if (!Array.isArray(allRulesRaw)) {
    throw new PortError('missing all_rules');
  }
  const allRules: string[] = [];
  for (const r of allRulesRaw) {
    if (typeof r !== 'string') {
      throw new PortError('all_rules contains non-string value');
    }
    allRules.push(r);
  }
  const candidateIndicesRaw = record['candidate_indices'];
  if (!Array.isArray(candidateIndicesRaw)) {
    throw new PortError('missing candidate_indices');
  }
  const candidateIndices: number[] = [];
  for (const i of candidateIndicesRaw) {
    if (typeof i !== 'number' || !Number.isInteger(i) || i < 0) {
      throw new PortError('candidate_indices contains non-integer value');
    }
    candidateIndices.push(i);
  }
  const stepRaw = record['step'];
  if (typeof stepRaw !== 'number' || !Number.isInteger(stepRaw) || stepRaw < 0) {
    throw new PortError('missing step');
  }
  const step = stepRaw;

  for (const idx of candidateIndices) {
    if (idx >= allRules.length) {
      throw new PortError(`candidate index ${idx} out of range (${allRules.length})`);
    }
  }

  return {
    original_filters: originalFilters,
    channel,
    all_rules: allRules,
    candidate_indices: candidateIndices,
    step,
  };
}

type Answer = 'yes' | 'no' | 'quit';

/**
 * Injectable I/O surface so the interactive loop has no direct process access.
 * The CLI provides the stdin/stdout/stderr implementation; tests mock it.
 */
export interface BisectIo {
  /** Write a full line to stderr (Rust eprintln parity). */
  err(line: string): void;
  /** Write raw text to stderr without a newline (Rust eprint! parity). */
  errRaw(text: string): void;
  /** Write raw text to stdout without a newline (Rust print!/println! parity). */
  out(text: string): void;
  /** Ask for one line; resolves null on EOF (Rust `lines().next() == None`). */
  ask(message: string): Promise<string | null>;
  /** Cooperative interrupt flag (SIGINT in the CLI layer). */
  isInterrupted(): boolean;
}

export type BisectOutcome = 'done' | 'suspended';

export interface BisectOptions {
  filePath?: string;
  resumePath?: string;
  outputDir?: string;
  cwd: string;
  channel: Channel;
}

export async function bisectRun(
  braveDir: string,
  opts: BisectOptions,
  io: BisectIo,
): Promise<void> {
  const outDir = opts.outputDir ?? opts.cwd;
  const statePath = nodePath.join(outDir, 'bisect-state.json');
  const workingFile = nodePath.join(outDir, 'bisect-filters.txt');

  let state: BisectState;
  if (opts.resumePath !== undefined) {
    const path = opts.resumePath;
    let content: string;
    try {
      content = fs.readFileSync(path, 'utf8');
    } catch (err) {
      throw new PortError(
        `failed to read bisect state from ${path}: ${describeIoError(err)}`,
      );
    }
    let v: unknown;
    try {
      v = JSON.parse(content);
    } catch (err) {
      throw new PortError(`failed to parse bisect state file: ${describeIoError(err)}`);
    }
    const parsed = withContext(() => bisectStateFromJson(v), 'invalid bisect state file');
    const savedChannel = channelFromStr(parsed.channel);
    if (channelToStr(savedChannel) !== channelToStr(opts.channel)) {
      throw new PortError(
        `state file channel (${parsed.channel}) does not match current channel (${channelToStr(opts.channel)})`,
      );
    }
    if (parsed.candidate_indices.length === 0) {
      throw new PortError('bisect state has no candidates');
    }
    io.err(
      `Resuming bisect: ${parsed.candidate_indices.length} candidates remaining (step ${parsed.step})`,
    );
    state = parsed;
  } else {
    if (opts.filePath === undefined) {
      throw new PortError(
        'a filter rules file is required (or use --resume to continue a previous session)',
      );
    }
    const rules = readRulesFromFile(opts.filePath);
    if (rules.length === 0) {
      throw new PortError(`no filter rules found in ${opts.filePath}`);
    }

    const localState = readLocalState(braveDir);
    const original = pointerCustomFilters(localState);

    state = {
      original_filters: original,
      channel: channelToStr(opts.channel),
      all_rules: rules,
      candidate_indices: [], // filled below
      step: 0,
    };
  }

  // For new sessions, populate candidate_indices (skipping resume which already has them)
  if (state.candidate_indices.length === 0 && state.step === 0) {
    state.candidate_indices = state.all_rules.map((_, i) => i);
    saveState(statePath, state);
  }

  const steps =
    state.candidate_indices.length <= 1
      ? 1
      : Math.ceil(Math.log2(state.candidate_indices.length));
  io.err(`Bisecting ${state.candidate_indices.length} rules (~${steps} steps)`);
  io.err(`Working file: ${workingFile}`);

  const outcome = await bisectLoop(braveDir, state, statePath, workingFile, io);

  if (outcome === 'done') {
    io.err('Quit the browser before restoring original filters.');
    const answer = await prompt('Ready to restore? [y]es / [n]o / [q]uit: ', io);
    if (answer === 'yes') {
      io.err('Restoring original custom filters...');
      const lsPath = nodePath.join(braveDir, 'Local State');
      try {
        writeAllFilters(lsPath, state.original_filters);
      } catch (err) {
        io.err(`Warning: failed to restore filters: ${describeIoError(err)}`);
      }
      try {
        fs.rmSync(statePath, { force: true });
      } catch {
        // ignore
      }
      try {
        fs.rmSync(workingFile, { force: true });
      } catch {
        // ignore
      }
    } else {
      // 'no' | 'quit'
      io.err('Skipping restore. Original filters are saved in the state file.');
      saveState(statePath, state);
      io.err(`State saved to ${statePath}`);
    }
  }
  // 'suspended': leave state and working file for --resume
}

// --- helpers -------------------------------------------------------------

/**
 * Convert a filter rule into its exception form.
 * - Network rules (`||example.com^`) get `@@` prefix
 * - Cosmetic rules (`example.com##.ad`) get `##` replaced with `#@#`
 * - Rules already in exception form are returned as-is
 * Note: some rule types ($removeparam, $redirect, etc.) may not be fully
 * neutralized by this approach. This is a known limitation.
 */
export function makeException(rule: string): string {
  if (rule.startsWith('@@') || rule.startsWith('#@#')) {
    return rule;
  }
  // Cosmetic or scriptlet filter: contains ##
  // Both cosmetic (##.ad) and scriptlet (##+js(...)) use #@# for exceptions.
  // The @@ prefix is only for network rules.
  const pos = rule.indexOf('##');
  if (pos !== -1) {
    const domain = rule.slice(0, pos);
    const selector = rule.slice(pos + 2);
    return `${domain}#@#${selector}`;
  }
  return `@@${rule}`;
}

/** Write the working filter file and load it into Local State. */
export function writeAndLoadFilters(
  workingFile: string,
  lsPath: string,
  allRules: string[],
  exceptedIndices: number[],
): void {
  const excepted = new Set(exceptedIndices);
  const lines = allRules.map((rule, i) =>
    excepted.has(i) ? makeException(rule) : rule,
  );
  const content = lines.join('\n');

  try {
    fs.writeFileSync(workingFile, content);
  } catch (err) {
    throw new PortError(`failed to write ${workingFile}: ${describeIoError(err)}`);
  }

  writeAllFilters(lsPath, content);
}

async function bisectLoop(
  braveDir: string,
  state: BisectState,
  statePath: string,
  workingFile: string,
  io: BisectIo,
): Promise<BisectOutcome> {
  const lsPath = nodePath.join(braveDir, 'Local State');

  // On a fresh session, load all rules and confirm the issue is present.
  // On resume (step > 0), skip this since the user already confirmed.
  if (state.step === 0) {
    io.err(`Loading all ${state.all_rules.length} rules as custom filters...`);
    writeAndLoadFilters(workingFile, lsPath, state.all_rules, []);
    io.err(
      'Custom filters loaded. Restart the browser and confirm the issue is present.',
    );

    const initial = await prompt('Is the issue present? [y]es / [n]o / [q]uit: ', io);
    if (initial === 'yes') {
      // fall through
    } else if (initial === 'no') {
      io.err('Issue is not present with these filters. Nothing to bisect.');
      return 'done';
    } else {
      saveState(statePath, state);
      io.err(`State saved to ${statePath}`);
      return 'suspended';
    }
  }

  if (state.candidate_indices.length === 1) {
    const rule = state.all_rules[state.candidate_indices[0]!]!;
    io.err(`Only one rule; it must be: ${rule}`);
    io.out(`${rule}\n`);
    return 'done';
  }

  for (;;) {
    if (io.isInterrupted()) {
      io.err('\nInterrupted.');
      saveState(statePath, state);
      io.err(`State saved to ${statePath}`);
      return 'suspended';
    }

    state.step += 1;
    const mid = Math.floor(state.candidate_indices.length / 2);
    const firstHalf = state.candidate_indices.slice(0, mid);
    const secondHalf = state.candidate_indices.slice(mid);

    io.err(
      `\nStep ${state.step} | Excepting out ${firstHalf.length} of ${state.candidate_indices.length} candidate rules (keeping ${secondHalf.length} active)`,
    );

    writeAndLoadFilters(workingFile, lsPath, state.all_rules, firstHalf);
    io.err(
      `Custom filters updated (see ${workingFile}). Restart the browser and check if the issue persists.`,
    );

    const answer = await prompt('Issue still present? [y]es / [n]o / [q]uit: ', io);

    if (answer === 'yes') {
      // Issue still present; the excepted rules (first half) are innocent.
      // The bad rule is in the second half (still active).
      state.candidate_indices = secondHalf;
    } else if (answer === 'no') {
      // Issue gone; the excepted rules (first half) contain the bad one.
      state.candidate_indices = firstHalf;
    } else {
      io.err('Quitting bisect.');
      saveState(statePath, state);
      io.err(`State saved to ${statePath}`);
      return 'suspended';
    }

    saveState(statePath, state);

    if (state.candidate_indices.length === 1) {
      const rule = state.all_rules[state.candidate_indices[0]!]!;
      io.err('\nFound problematic rule:');
      io.out(`${rule}\n`);
      return 'done';
    }
  }
}

export function readRulesFromFile(path: string): string[] {
  let content: string;
  try {
    content = fs.readFileSync(path, 'utf8');
  } catch (err) {
    throw new PortError(`failed to read ${path}: ${describeIoError(err)}`);
  }
  return content
    .split('\n')
    .map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line))
    .filter((line) => {
      const trimmed = line.trim();
      return trimmed.length > 0 && !trimmed.startsWith('!');
    });
}

export function saveState(path: string, state: BisectState): void {
  const content = sortedStringify(bisectStateToJson(state));
  try {
    fs.writeFileSync(path, content);
  } catch (err) {
    throw new PortError(
      `failed to save bisect state to ${path}: ${describeIoError(err)}`,
    );
  }
}

async function prompt(message: string, io: BisectIo): Promise<Answer> {
  for (;;) {
    io.errRaw(message);

    const line = await io.ask(message);

    if (io.isInterrupted()) {
      return 'quit';
    }

    if (line === null) {
      // EOF ⇒ treat as quit (matches Rust `lines().next() == None`)
      return 'quit';
    }

    const trimmed = line.trim().toLowerCase();
    if (trimmed === 'y' || trimmed === 'yes') return 'yes';
    if (trimmed === 'n' || trimmed === 'no') return 'no';
    if (trimmed === 'q' || trimmed === 'quit') return 'quit';
    io.err('Please answer y, n, or q.');
  }
}

function pointerCustomFilters(value: unknown): string {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return '';
  const brave = (value as Record<string, unknown>)['brave'];
  if (brave === null || typeof brave !== 'object' || Array.isArray(brave)) return '';
  const adBlock = (brave as Record<string, unknown>)['ad_block'];
  if (adBlock === null || typeof adBlock !== 'object' || Array.isArray(adBlock)) {
    return '';
  }
  const cf = (adBlock as Record<string, unknown>)['custom_filters'];
  return typeof cf === 'string' ? cf : '';
}

function withContext<T>(fn: () => T, context: string): T {
  try {
    return fn();
  } catch (err) {
    throw new PortError(`${context}: ${describeIoError(err)}`);
  }
}