import { describe, expect, it } from 'vitest';
import {
  bisectStateFromJson,
  makeException,
  readRulesFromFile,
  saveState,
  writeAndLoadFilters,
} from '../src/commands/bisect.js';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

function tmpdir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'brave-bisect-'));
}

describe('readRulesFromFile', () => {
  it('filters blank lines and comments', () => {
    const tmp = tmpdir();
    const path = tmp + '/rules.txt';
    fs.writeFileSync(path, '||a.com^\n\n||b.com^\n!comment\n\n');
    expect(readRulesFromFile(path)).toEqual(['||a.com^', '||b.com^']);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('skips comment lines', () => {
    const tmp = tmpdir();
    const path = tmp + '/rules.txt';
    fs.writeFileSync(path, '!title\n||a.com^\n! another comment\n||b.com^');
    expect(readRulesFromFile(path)).toEqual(['||a.com^', '||b.com^']);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('empty file', () => {
    const tmp = tmpdir();
    const path = tmp + '/rules.txt';
    fs.writeFileSync(path, '\n\n');
    expect(readRulesFromFile(path)).toEqual([]);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('only comments', () => {
    const tmp = tmpdir();
    const path = tmp + '/rules.txt';
    fs.writeFileSync(path, '! comment 1\n! comment 2\n');
    expect(readRulesFromFile(path)).toEqual([]);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('nonexistent file errors', () => {
    expect(() => readRulesFromFile('/nonexistent/file.txt')).toThrow(/failed to read/);
  });
});

describe('bisect state save/load', () => {
  it('round-trips state', () => {
    const tmp = tmpdir();
    const path = tmp + '/state.json';
    const state = {
      original_filters: '||original.com^',
      channel: 'release',
      all_rules: ['||a.com^', '||b.com^'],
      candidate_indices: [0, 1],
      step: 0,
    };
    saveState(path, state);

    const loaded = bisectStateFromJson(JSON.parse(fs.readFileSync(path, 'utf8')));
    expect(loaded.original_filters).toBe('||original.com^');
    expect(loaded.all_rules).toHaveLength(2);
    expect(loaded.candidate_indices).toEqual([0, 1]);
    expect(loaded.step).toBe(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('preserves step and channel', () => {
    const tmp = tmpdir();
    const path = tmp + '/state.json';
    const state = {
      original_filters: '',
      channel: 'nightly',
      all_rules: ['||a.com^'],
      candidate_indices: [0],
      step: 5,
    };
    saveState(path, state);

    const loaded = bisectStateFromJson(JSON.parse(fs.readFileSync(path, 'utf8')));
    expect(loaded.step).toBe(5);
    expect(loaded.channel).toBe('nightly');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('rejects non-integer indices', () => {
    expect(() =>
      bisectStateFromJson({
        original_filters: '',
        channel: 'release',
        all_rules: ['||a.com^'],
        candidate_indices: [0, 'bad', 2],
        step: 0,
      }),
    ).toThrow(/non-integer/);
  });

  it('rejects out-of-bounds indices', () => {
    expect(() =>
      bisectStateFromJson({
        original_filters: '',
        channel: 'release',
        all_rules: ['||a.com^'],
        candidate_indices: [0, 99],
        step: 0,
      }),
    ).toThrow(/out of range/);
  });

  it('rejects non-string rules', () => {
    expect(() =>
      bisectStateFromJson({
        original_filters: '',
        channel: 'release',
        all_rules: ['||a.com^', 42],
        candidate_indices: [0],
        step: 0,
      }),
    ).toThrow(/non-string/);
  });

  it('rejects missing step', () => {
    expect(() =>
      bisectStateFromJson({
        original_filters: '',
        channel: 'release',
        all_rules: ['||a.com^'],
        candidate_indices: [0],
      }),
    ).toThrow(/missing step/);
  });
});

describe('writeAndLoadFilters', () => {
  it('no exceptions', () => {
    const tmp = tmpdir();
    const working = tmp + '/bisect-filters.txt';
    const lsPath = tmp + '/Local State';
    fs.writeFileSync(lsPath, '{}');

    writeAndLoadFilters(working, lsPath, ['||a.com^', '||b.com^'], []);

    expect(fs.readFileSync(working, 'utf8')).toBe('||a.com^\n||b.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('with exceptions', () => {
    const tmp = tmpdir();
    const working = tmp + '/bisect-filters.txt';
    const lsPath = tmp + '/Local State';
    fs.writeFileSync(lsPath, '{}');

    const rules = ['||a.com^', '||b.com^', '||c.com^', '||d.com^'];
    writeAndLoadFilters(working, lsPath, rules, [0, 1]);

    const content = fs.readFileSync(working, 'utf8');
    expect(content).toBe('@@||a.com^\n@@||b.com^\n||c.com^\n||d.com^');

    // Verify it was also loaded into Local State
    const ls = JSON.parse(fs.readFileSync(lsPath, 'utf8'));
    expect(ls['brave']['ad_block']['custom_filters']).toBe(content);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('does not double-prefix exceptions', () => {
    const tmp = tmpdir();
    const working = tmp + '/bisect-filters.txt';
    const lsPath = tmp + '/Local State';
    fs.writeFileSync(lsPath, '{}');

    const rules = ['||a.com^', '@@||b.com^', '||c.com^'];
    writeAndLoadFilters(working, lsPath, rules, [0, 1]);

    const content = fs.readFileSync(working, 'utf8');
    // Index 0 gets @@, index 1 already has @@ so stays as-is
    expect(content).toBe('@@||a.com^\n@@||b.com^\n||c.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('all excepted', () => {
    const tmp = tmpdir();
    const working = tmp + '/bisect-filters.txt';
    const lsPath = tmp + '/Local State';
    fs.writeFileSync(lsPath, '{}');

    writeAndLoadFilters(working, lsPath, ['||a.com^', '||b.com^'], [0, 1]);

    expect(fs.readFileSync(working, 'utf8')).toBe('@@||a.com^\n@@||b.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('single exception', () => {
    const tmp = tmpdir();
    const working = tmp + '/bisect-filters.txt';
    const lsPath = tmp + '/Local State';
    fs.writeFileSync(lsPath, '{}');

    writeAndLoadFilters(working, lsPath, ['||a.com^', '||b.com^', '||c.com^'], [1]);

    expect(fs.readFileSync(working, 'utf8')).toBe('||a.com^\n@@||b.com^\n||c.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('preserves other Local State keys', () => {
    const tmp = tmpdir();
    const working = tmp + '/bisect-filters.txt';
    const lsPath = tmp + '/Local State';
    fs.writeFileSync(
      lsPath,
      JSON.stringify({
        profile: { last_used: 'Default' },
        brave: { ad_block: { custom_filters: '||old.com^' } },
      }),
    );

    writeAndLoadFilters(working, lsPath, ['||new.com^'], []);

    const ls = JSON.parse(fs.readFileSync(lsPath, 'utf8'));
    expect(ls['profile']['last_used']).toBe('Default');
    expect(ls['brave']['ad_block']['custom_filters']).toBe('||new.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

describe('bisect split halves', () => {
  it('8 rules', () => {
    const candidates = Array.from({ length: 8 }, (_, i) => i);
    const mid = Math.floor(candidates.length / 2);
    expect(candidates.slice(0, mid)).toEqual([0, 1, 2, 3]);
    expect(candidates.slice(mid)).toEqual([4, 5, 6, 7]);
  });

  it('odd count', () => {
    const candidates = Array.from({ length: 5 }, (_, i) => i);
    const mid = Math.floor(candidates.length / 2);
    // First half is smaller
    expect(candidates.slice(0, mid)).toEqual([0, 1]);
    expect(candidates.slice(mid)).toEqual([2, 3, 4]);
  });

  it('2 candidates', () => {
    const candidates = [3, 7];
    const mid = Math.floor(candidates.length / 2);
    expect(candidates.slice(0, mid)).toEqual([3]);
    expect(candidates.slice(mid)).toEqual([7]);
  });
});

describe('makeException', () => {
  it('network rule', () => {
    expect(makeException('||example.com^')).toBe('@@||example.com^');
  });

  it('cosmetic rule', () => {
    expect(makeException('example.com##.ad-banner')).toBe('example.com#@#.ad-banner');
  });

  it('already exception network', () => {
    expect(makeException('@@||example.com^')).toBe('@@||example.com^');
  });

  it('already exception cosmetic', () => {
    expect(makeException('#@#.ad-banner')).toBe('#@#.ad-banner');
  });

  it('cosmetic no domain', () => {
    // ##.ad is a global cosmetic rule (no domain), still uses #@# exception
    expect(makeException('##.ad')).toBe('#@#.ad');
  });

  it('scriptlet rule', () => {
    expect(makeException('example.com##+js(abort-on-property-read, foo)')).toBe(
      'example.com#@#+js(abort-on-property-read, foo)',
    );
  });

  it('scriptlet rule no domain', () => {
    expect(makeException('##+js(abort-on-property-read, foo)')).toBe(
      '#@#+js(abort-on-property-read, foo)',
    );
  });

  it('cosmetic with subdomain', () => {
    expect(makeException('sub.example.com##div.ad')).toBe('sub.example.com#@#div.ad');
  });
});