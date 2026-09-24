import { ClassicLevel } from 'classic-level';
import { constants, flockSync } from 'fs-ext';
import * as fs from 'node:fs';
import * as nodePath from 'node:path';
import { PortError } from './errors.js';
import { describeIoError } from './fsutil.js';
import { sortedStringifyCompact } from './json.js';
import type { JsonObject, JsonValue } from './json.js';

const SCRIPTLETS_KEY = 'SCRIPTLETS';
const DB_DIR_NAME = 'AdBlock Custom Resources';

/** A single custom scriptlet. */
export interface Scriptlet {
  name: string;
  content: string;
}

/** Resolve the path to the AdBlock Custom Resources LevelDB.
 * This is always under Default/, regardless of active profile. */
export function dbPath(braveDir: string): string {
  return nodePath.join(braveDir, 'Default', DB_DIR_NAME);
}

/** Sibling lock file (rusty-leveldb/C++ LevelDB LOCK-file protocol parity:
 * we must coexist with the Chromium-side DB and the previous Rust behavior). */
function lockPath(braveDir: string): string {
  return `${dbPath(braveDir)}.lock`;
}

/**
 * Acquire an exclusive file lock on a sibling lock file before accessing the
 * DB, run `fn`, then release the lock.
 */
async function withDbLock<T>(braveDir: string, fn: () => Promise<T>): Promise<T> {
  const lockFile = lockPath(braveDir);
  const parent = nodePath.dirname(lockFile);
  try {
    fs.mkdirSync(parent, { recursive: true });
  } catch (err) {
    throw new PortError(`failed to create directory ${parent}: ${describeIoError(err)}`);
  }
  let fd: number;
  try {
    fd = fs.openSync(lockFile, 'w');
  } catch (err) {
    throw new PortError(`failed to create lock file at ${lockFile}: ${describeIoError(err)}`);
  }
  try {
    try {
      flockSync(fd, constants.LOCK_EX);
    } catch (err) {
      throw new PortError(
        `failed to acquire exclusive lock on ${lockFile}: ${describeIoError(err)}`,
      );
    }
    return await fn();
  } finally {
    try {
      flockSync(fd, constants.LOCK_UN);
    } catch {
      // ignore
    }
    fs.closeSync(fd);
  }
}

interface OpenOptions {
  createIfMissing: boolean;
}

async function openDb(target: string, opts: OpenOptions): Promise<ClassicLevel> {
  const db = new ClassicLevel(target, {
    createIfMissing: opts.createIfMissing,
  });
  try {
    await db.open();
  } catch (err) {
    try {
      await db.close();
    } catch {
      // ignore
    }
    throw new PortError(`failed to open LevelDB at ${target}: ${describeIoError(err)}`);
  }
  return db;
}

/**
 * Read all custom scriptlets from the LevelDB.
 * A missing DB directory yields an empty list.
 */
export async function readScriptlets(braveDir: string): Promise<Scriptlet[]> {
  const target = dbPath(braveDir);
  if (!fs.existsSync(target)) {
    return [];
  }
  return withDbLock(braveDir, () => readScriptletsUnlocked(target));
}

/** Read scriptlets without acquiring the lock. Called from within `withDbLock`. */
async function readScriptletsUnlocked(target: string): Promise<Scriptlet[]> {
  if (!fs.existsSync(target)) {
    return [];
  }

  const db = await openDb(target, { createIfMissing: false });
  try {
    let raw: string | undefined;
    try {
      raw = await db.get(SCRIPTLETS_KEY);
    } catch {
      return []; // key not present
    }
    if (raw === undefined) {
      return [];
    }

    // SCRIPTLETS value is not valid UTF-8 — use a fatal decoder.
    const utf8Fatal = new TextDecoder('utf-8', { fatal: true });
    let jsonStr: string;
    try {
      jsonStr = utf8Fatal.decode(Buffer.from(raw, 'latin1'));
    } catch {
      throw new PortError('SCRIPTLETS value is not valid UTF-8');
    }

    let entries: JsonValue;
    try {
      entries = JSON.parse(jsonStr) as JsonValue;
    } catch (err) {
      throw new PortError(`failed to parse SCRIPTLETS JSON: ${describeIoError(err)}`);
    }
    if (!Array.isArray(entries)) {
      throw new PortError('failed to parse SCRIPTLETS JSON: invalid type');
    }

    const scriptlets: Scriptlet[] = [];
    for (const entry of entries) {
      let name: string | undefined;
      let contentB64: string | undefined;
      if (entry !== null && typeof entry === 'object' && !Array.isArray(entry)) {
        const record = entry as JsonObject;
        if (typeof record['name'] === 'string') name = record['name'];
        if (typeof record['content'] === 'string') contentB64 = record['content'];
      }
      if (name === undefined) {
        throw new PortError('scriptlet missing name');
      }
      if (contentB64 === undefined) {
        throw new PortError('scriptlet missing content');
      }
      let content: string;
      if (contentB64.length === 0) {
        content = '';
      } else {
        let bytes: Buffer;
        try {
          bytes = Buffer.from(contentB64, 'base64');
        } catch {
          throw new PortError('failed to decode scriptlet content');
        }
        try {
          content = utf8Fatal.decode(bytes);
        } catch {
          throw new PortError('scriptlet content is not valid UTF-8');
        }
      }
      scriptlets.push({ name, content });
    }
    return scriptlets;
  } finally {
    try {
      await db.close();
    } catch {
      // ignore
    }
  }
}

/**
 * Write the full set of scriptlets to the LevelDB.
 * Entry shape must match Chromium's expectations exactly:
 * `kind: { mime: "application/javascript" }`, base64 content, compact JSON
 * with sorted keys (Rust used `serde_json::to_string`, i.e. compact).
 */
async function writeScriptlets(braveDir: string, scriptlets: Scriptlet[]): Promise<void> {
  const target = dbPath(braveDir);

  // Ensure the Default profile directory exists
  const parent = nodePath.dirname(target);
  try {
    fs.mkdirSync(parent, { recursive: true });
  } catch (err) {
    throw new PortError(`failed to create directory ${parent}: ${describeIoError(err)}`);
  }

  const entries: JsonValue = scriptlets.map((s) => ({
    name: s.name,
    content: Buffer.from(s.content, 'utf8').toString('base64'),
    kind: { mime: 'application/javascript' },
  }));

  const jsonStr = sortedStringifyCompact(entries);

  const db = await openDb(target, { createIfMissing: true });
  try {
    try {
      await db.put(SCRIPTLETS_KEY, jsonStr);
    } catch (err) {
      throw new PortError(`failed to write SCRIPTLETS: ${describeIoError(err)}`);
    }
    // Note: classic-level has no explicit flush; durability is guaranteed by
    // the LevelDB write-ahead log, and db.close() in the finally block below
    // syncs any remaining state (Rust called db.flush()).
  } finally {
    try {
      await db.close();
    } catch {
      // ignore
    }
  }
}

/** Validate a scriptlet name matches Brave's requirements. */
function validateName(name: string): void {
  if (name.length === 0) {
    throw new PortError('scriptlet name must not be empty');
  }
  // eslint-disable-next-line no-control-regex
  if (!/^[\x00-\x7F]*$/.test(name)) {
    throw new PortError('scriptlet name must be ASCII');
  }
  if (name.includes('/') || name.includes('\\') || name.includes('..')) {
    throw new PortError(
      `scriptlet name must not contain path separators or traversal sequences (got '${name}')`,
    );
  }
  if (!name.startsWith('user-')) {
    throw new PortError(`scriptlet name must start with 'user-' (got '${name}')`);
  }
  if (!name.endsWith('.js')) {
    throw new PortError(`scriptlet name must end with '.js' (got '${name}')`);
  }
}

/** Add a scriptlet. If one with the same name exists, it is replaced. */
export async function addScriptlet(
  braveDir: string,
  name: string,
  content: string,
): Promise<void> {
  validateName(name);
  const target = dbPath(braveDir);
  await withDbLock(braveDir, async () => {
    const scriptlets = await readScriptletsUnlocked(target);
    const kept = scriptlets.filter((s) => s.name !== name);
    kept.push({ name, content });
    await writeScriptlets(braveDir, kept);
  });
}

/** Remove a scriptlet by name. Returns true if it was found. */
export async function removeScriptlet(braveDir: string, name: string): Promise<boolean> {
  const target = dbPath(braveDir);
  return withDbLock(braveDir, async () => {
    const scriptlets = await readScriptletsUnlocked(target);
    const kept = scriptlets.filter((s) => s.name !== name);
    if (kept.length === scriptlets.length) {
      return false;
    }
    await writeScriptlets(braveDir, kept);
    return true;
  });
}

/** Remove all custom scriptlets. */
export async function clearScriptlets(braveDir: string): Promise<void> {
  await withDbLock(braveDir, async () => {
    await writeScriptlets(braveDir, []);
  });
}