import { describe, expect, it } from 'vitest';
import {
  outputFormatFromStr,
  renderFilters,
  renderGet,
  renderList,
  renderProfiles,
  renderReset,
  renderSet,
} from '../src/output.js';
import { PortError } from '../src/errors.js';

describe('output format parsing', () => {
  it('accepts json and table', () => {
    expect(outputFormatFromStr('json')).toBe('json');
    expect(outputFormatFromStr('table')).toBe('table');
  });

  it('rejects unknown formats with the exact message', () => {
    expect(() => outputFormatFromStr('yaml')).toThrow(
      "unknown format 'yaml'. Valid: json, table",
    );
    expect(() => outputFormatFromStr('yaml')).toThrow(PortError);
  });
});

describe('renderGet', () => {
  it('json snapshot matches Rust serde_json output (sorted keys, 2-space)', () => {
    const settings: Record<string, string> = {
      shields: 'on',
      ads: 'standard',
      fingerprinting: 'standard',
      'https-upgrade': 'strict',
      scripts: 'allow',
    };
    const out = renderGet('example.com', 'Default', settings, 'json');
    // BTreeMap ordering: ads, fingerprinting, https-upgrade, scripts, shields.
    expect(out).toBe(
      `{
  "domain": "example.com",
  "profile": "Default",
  "settings": {
    "ads": "standard",
    "fingerprinting": "standard",
    "https-upgrade": "strict",
    "scripts": "allow",
    "shields": "on"
  }
}`,
    );
  });

  it('table format includes domain/profile line and rows', () => {
    const out = renderGet('example.com', 'Default', { shields: 'on', ads: 'off' }, 'table');
    expect(out.startsWith('Domain: example.com  Profile: Default')).toBe(true);
    expect(out).toContain('Setting');
    expect(out).toContain('Value');
    expect(out).toContain('shields');
    expect(out).toContain('on');
  });
});

describe('renderList', () => {
  it('json output', () => {
    const out = renderList(
      [
        { domain: 'a.com', overrides: { shields: 'off' } },
        { domain: 'b.com', overrides: { ads: 'aggressive', shields: 'on' } },
      ],
      'json',
    );
    expect(JSON.parse(out)).toEqual([
      { domain: 'a.com', overrides: { shields: 'off' } },
      { domain: 'b.com', overrides: { ads: 'aggressive', shields: 'on' } },
    ]);
    expect(out).toContain('"overrides"');
    // overrides keys sorted alphabetically within the second entry
    const secondEntry = out.slice(out.indexOf('"b.com"'));
    expect(secondEntry.indexOf('"ads"')).toBeLessThan(secondEntry.indexOf('"shields"'));
  });

  it('empty table output', () => {
    expect(renderList([], 'table')).toBe('No per-site shield overrides found.');
  });

  it('table lists domains and joined overrides', () => {
    const out = renderList([{ domain: 'a.com', overrides: { shields: 'off' } }], 'table');
    expect(out).toContain('a.com');
    expect(out).toContain('shields=off');
  });
});

describe('renderProfiles', () => {
  it('json output uses dir/name keys', () => {
    const out = renderProfiles(
      [{ dirName: 'Default', displayName: 'Person 1' }],
      'json',
    );
    expect(JSON.parse(out)).toEqual([{ dir: 'Default', name: 'Person 1' }]);
  });

  it('empty table output', () => {
    expect(renderProfiles([], 'table')).toBe('No profiles found.');
  });
});

describe('renderFilters', () => {
  it('json output uses custom_filters key', () => {
    const out = renderFilters(['||example.com^'], 'json');
    expect(out).toBe(`{
  "custom_filters": [
    "||example.com^"
  ]
}`);
  });

  it('empty table output', () => {
    expect(renderFilters([], 'table')).toBe('No custom filter rules.');
  });
});

describe('stderr message builders', () => {
  it('set', () => {
    expect(renderSet('example.com', 'shields', 'on')).toBe(
      'Set shields = on for example.com',
    );
  });

  it('reset with setting', () => {
    expect(renderReset('example.com', 'ads')).toBe('Reset ads for example.com');
  });

  it('reset all', () => {
    expect(renderReset('example.com')).toBe('Reset all shield settings for example.com');
  });
});