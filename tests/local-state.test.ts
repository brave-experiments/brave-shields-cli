import { describe, expect, it } from 'vitest';
import {
  getCustomFilters,
  lockedReadModifyWrite,
  readLocalState,
} from '../src/local-state.js';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

function tmpdir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'brave-ls-'));
}

describe('getCustomFilters', () => {
  it('with entries', () => {
    const state = {
      brave: {
        ad_block: {
          custom_filters: '||example.com^\nexample.com##h1\n\n!comment',
        },
      },
    };
    expect(getCustomFilters(state)).toEqual([
      '||example.com^',
      'example.com##h1',
      '!comment',
    ]);
  });

  it('empty state', () => {
    expect(getCustomFilters({})).toEqual([]);
  });

  it('empty string', () => {
    expect(getCustomFilters({ brave: { ad_block: { custom_filters: '' } } })).toEqual(
      [],
    );
  });
});

describe('lockedReadModifyWrite', () => {
  it('round-trips a modification', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Local State');
    fs.writeFileSync(
      target,
      JSON.stringify({ brave: { ad_block: { custom_filters: '||old.com^' } } }, null, 2),
    );

    lockedReadModifyWrite(target, (state) => {
      const record = state as {
        brave: { ad_block: { custom_filters: string } };
      };
      record.brave.ad_block.custom_filters = '||old.com^\n||new.com^';
    });

    const result = readLocalState(tmp);
    expect(getCustomFilters(result)).toEqual(['||old.com^', '||new.com^']);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('creates a backup on first write', () => {
    const tmp = tmpdir();
    const target = path.join(tmp, 'Local State');
    fs.writeFileSync(target, JSON.stringify({ original: true }, null, 2));

    lockedReadModifyWrite(target, (state) => {
      (state as Record<string, unknown>)['modified'] = true;
    });

    const bakPath = path.join(tmp, 'Local State.bak');
    expect(fs.existsSync(bakPath)).toBe(true);
    expect(JSON.parse(fs.readFileSync(bakPath, 'utf8'))).toEqual({ original: true });
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('errors when file missing', () => {
    expect(() => lockedReadModifyWrite('/nonexistent/Local State', () => {})).toThrow(
      /Local State file not found at/,
    );
  });
});