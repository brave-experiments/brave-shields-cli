import { afterEach, describe, expect, it } from 'vitest';
import {
  checkBraveNotRunning,
  getExceptionEntries,
  getExceptionEntriesMut,
  getExceptions,
  lockedReadModifyWrite,
  readPreferences,
  writePreferences,
} from '../src/preferences.js';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

function tmpdir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'brave-prefs-'));
}

describe('preferences navigation', () => {
  const prefs = {
    profile: {
      content_settings: {
        exceptions: {
          braveShields: {
            'example.com,*': { last_modified: '123', setting: 2 },
          },
        },
      },
    },
  };

  it('read exceptions', () => {
    const entries = getExceptionEntries(prefs, 'braveShields')!;
    expect('example.com,*' in entries).toBe(true);
    expect(entries['example.com,*']).toEqual({ last_modified: '123', setting: 2 });
  });

  it('write new entry', () => {
    const target: Record<string, unknown> = {};
    const entries = getExceptionEntriesMut(target, 'braveShields');
    entries['example.com,*'] = { setting: 2, last_modified: '123' };
    const read = getExceptionEntries(target, 'braveShields')!;
    expect(read['example.com,*']).toEqual({ setting: 2, last_modified: '123' });
  });

  it('write preserves existing', () => {
    const target = structuredClone(prefs);
    const entries = getExceptionEntriesMut(target, 'braveShields');
    entries['new.com,*'] = { setting: 2, last_modified: '200' };
    const read = getExceptionEntries(target, 'braveShields')!;
    expect(read['existing.com,*']).toBeUndefined();
    expect(read['example.com,*']).toEqual({ last_modified: '123', setting: 2 });
    expect(read['new.com,*']).toEqual({ setting: 2, last_modified: '200' });
  });

  it('create missing intermediates', () => {
    const target: Record<string, unknown> = { some_other_key: true };
    const entries = getExceptionEntriesMut(target, 'braveShields');
    entries['test.com,*'] = { setting: 1, last_modified: '0' };
    expect(
      (target as any).profile.content_settings.exceptions.braveShields['test.com,*'],
    ).toEqual({ setting: 1, last_modified: '0' });
    expect(target['some_other_key']).toBe(true);
  });

  it('getExceptions returns null when intermediates missing', () => {
    expect(getExceptions({})).toBeNull();
    expect(getExceptions({ profile: {} })).toBeNull();
  });
});

describe('atomic write pipeline', () => {
  it('atomic write round-trip', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Preferences');
    writePreferences(target, { test: true });
    expect(readPreferences(target)).toEqual({ test: true });
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('read nonexistent file errors', () => {
    expect(() => readPreferences('/nonexistent/path/Preferences')).toThrow(
      /failed to read Preferences at \/nonexistent\/path\/Preferences/,
    );
  });

  it('cosmetic filtering nested format', () => {
    const target: Record<string, unknown> = {};
    const entries = getExceptionEntriesMut(target, 'cosmeticFilteringV2');
    entries['example.com,*'] = {
      last_modified: '123',
      setting: { cosmeticFilteringV2: 2 },
    };
    const read = getExceptionEntries(target, 'cosmeticFilteringV2')!;
    const setting = read['example.com,*']!['setting'] as Record<string, unknown>;
    expect(setting['cosmeticFilteringV2']).toBe(2);
  });

  it('backup created on first write', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Preferences');
    fs.writeFileSync(target, JSON.stringify({ original: true }, null, 2));
    writePreferences(target, { updated: true });
    const bakPath = path.join(tmp, 'Preferences.bak');
    expect(fs.existsSync(bakPath)).toBe(true);
    expect(JSON.parse(fs.readFileSync(bakPath, 'utf8'))).toEqual({ original: true });
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('backup not overwritten on second write', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Preferences');
    fs.writeFileSync(target, JSON.stringify({ original: true }, null, 2));
    writePreferences(target, { second: true });
    writePreferences(target, { third: true });
    const bakPath = path.join(tmp, 'Preferences.bak');
    expect(JSON.parse(fs.readFileSync(bakPath, 'utf8'))).toEqual({ original: true });
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('post-write verification: file is valid JSON after write', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Preferences');
    writePreferences(target, { verify: true });
    const parsed = JSON.parse(fs.readFileSync(target, 'utf8'));
    expect(parsed['verify']).toBe(true);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('written file uses sorted keys with 2-space indent (serde_json parity)', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Preferences');
    writePreferences(target, { b: 1, a: { d: 2, c: 3 } });
    expect(fs.readFileSync(target, 'utf8')).toBe(
      `{\n  "a": {\n    "c": 3,\n    "d": 2\n  },\n  "b": 1\n}`,
    );
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

describe('lockedReadModifyWrite', () => {
  it('modifies under lock', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Preferences');
    fs.writeFileSync(target, JSON.stringify({ counter: 1 }, null, 2));

    lockedReadModifyWrite(target, (prefs) => {
      (prefs as Record<string, unknown>)['counter'] = 2;
    });

    expect(readPreferences(target)).toEqual({ counter: 2 });
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('errors when file missing', () => {
    expect(() =>
      lockedReadModifyWrite('/nonexistent/Preferences', () => {}),
    ).toThrow(/Preferences file not found at/);
  });
});

describe('checkBraveNotRunning', () => {
  const origPlatform = process.platform;

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: origPlatform });
  });

  it('never reports running on platforms with empty name tables', async () => {
    Object.defineProperty(process, 'platform', { value: 'freebsd' });
    await expect(checkBraveNotRunning('release', false)).resolves.toEqual({});
  });
});