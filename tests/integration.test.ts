/**
 * 1:1 port of tests/integration.rs — spawns the built CLI
 * (`node dist/cli.js`) instead of the Rust binary.
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

const CLI = path.resolve(__dirname, '../dist/cli.js');

beforeAll(() => {
  // Build once; integration tests spawn the built CLI.
  const { status, stderr } = spawnSync('npm', ['run', 'build'], { cwd: path.join(__dirname, '..') });
  expect(status, String(stderr)).toBe(0);
});

function tmpdir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'brave-it-'));
}

/** Set up a temp directory with Local State and a profile Preferences file. */
function setupFixture(prefs: object): string {
  const tmp = tmpdir();

  const localState = {
    profile: {
      last_used: 'Default',
      info_cache: {
        Default: { name: 'Test Profile' },
      },
    },
  };
  fs.writeFileSync(path.join(tmp, 'Local State'), JSON.stringify(localState, null, 2));

  const profileDir = path.join(tmp, 'Default');
  fs.mkdirSync(profileDir, { recursive: true });
  fs.writeFileSync(path.join(profileDir, 'Preferences'), JSON.stringify(prefs, null, 2));

  return tmp;
}

function setupFixtureWithLocalState(prefs: object, localState: object): string {
  const tmp = tmpdir();
  fs.writeFileSync(path.join(tmp, 'Local State'), JSON.stringify(localState, null, 2));

  const profileDir = path.join(tmp, 'Default');
  fs.mkdirSync(profileDir, { recursive: true });
  fs.writeFileSync(path.join(profileDir, 'Preferences'), JSON.stringify(prefs, null, 2));

  return tmp;
}

function runCmd(args: string[], braveDir: string): { stdout: string; stderr: string; success: boolean } {
  const result = spawnSync(
    process.execPath,
    [CLI, ...args, '--brave-dir', braveDir, '--force'],
    { encoding: 'utf8' },
  );
  return { stdout: result.stdout ?? '', stderr: result.stderr ?? '', success: result.status === 0 };
}

function runCmdWithStdin(
  args: string[],
  braveDir: string,
  stdinInput: string,
): { stdout: string; stderr: string; success: boolean } {
  const result = spawnSync(process.execPath, [CLI, ...args, '--brave-dir', braveDir, '--force'], {
    cwd: braveDir,
    input: stdinInput,
    encoding: 'utf8',
  });
  return { stdout: result.stdout ?? '', stderr: result.stderr ?? '', success: result.status === 0 };
}

function readLocalStateFile(braveDir: string): any {
  return JSON.parse(fs.readFileSync(path.join(braveDir, 'Local State'), 'utf8'));
}

function readPrefs(braveDir: string): any {
  return JSON.parse(fs.readFileSync(path.join(braveDir, 'Default', 'Preferences'), 'utf8'));
}

function at(obj: any, ...keys: (string | number)[]): any {
  let cur = obj;
  for (const key of keys) {
    if (cur === null || cur === undefined) return undefined;
    cur = cur[key];
  }
  return cur;
}

// ==================== GET command tests ====================

describe('get', () => {
  it('domain with overrides', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: { 'example.com,*': { setting: 2, last_modified: '123' } },
            fingerprintingV2: { 'example.com,*': { setting: 1, last_modified: '123' } },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { stdout, success } = runCmd(['get', 'example.com'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output['settings']['shields']).toBe('off');
    expect(output['settings']['fingerprinting']).toBe('disabled');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('domain no overrides', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['get', 'example.com'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output['settings']['shields']).toBe('default');
    expect(output['settings']['ads']).toBe('default');
    expect(output['settings']['fingerprinting']).toBe('default');
    expect(output['settings']['https-upgrade']).toBe('default');
    expect(output['settings']['scripts']).toBe('default');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('json output format', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['get', 'example.com', '--format', 'json'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output['domain']).toBe('example.com');
    expect(typeof output['settings']).toBe('object');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('table output format', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['get', 'example.com', '--format', 'table'], tmp);
    expect(success).toBe(true);
    expect(stdout).toContain('Domain: example.com');
    expect(stdout).toContain('Setting');
    expect(stdout).toContain('Value');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== SET command tests ====================

function exceptionsOf(prefs: any): any {
  return prefs['profile']['content_settings']['exceptions'];
}

describe('set', () => {
  const cases: Array<[string[], string, string, unknown]> = [
    [['set', 'example.com', 'shields', 'off'], 'braveShields', 'setting', 2],
    [['set', 'example.com', 'shields', 'on'], 'braveShields', 'setting', 1],
    [['set', 'example.com', 'fingerprinting', 'disabled'], 'fingerprintingV2', 'setting', 1],
    [['set', 'example.com', 'fingerprinting', 'aggressive'], 'fingerprintingV2', 'setting', 2],
    [['set', 'example.com', 'fingerprinting', 'standard'], 'fingerprintingV2', 'setting', 3],
    [['set', 'example.com', 'https-upgrade', 'strict'], 'httpsUpgrades', 'setting', 2],
    [['set', 'example.com', 'scripts', 'block'], 'javascript', 'setting', 2],
  ];
  for (const [args, key, field, expected] of cases) {
    it(args.join(' '), () => {
      const tmp = setupFixture({});
      const { success } = runCmd(args, tmp);
      expect(success).toBe(true);
      const prefs = readPrefs(tmp);
      expect(at(exceptionsOf(prefs), key, 'example.com,*')[field]).toBe(expected);
      fs.rmSync(tmp, { recursive: true, force: true });
    });
  }

  it('ads standard triple-key write', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['set', 'example.com', 'ads', 'standard'], tmp);
    expect(success).toBe(true);
    const prefs = readPrefs(tmp);
    const exceptions = exceptionsOf(prefs);
    expect(at(exceptions, 'shieldsAds', 'example.com,*')['setting']).toBe(2);
    expect(at(exceptions, 'trackers', 'example.com,*')['setting']).toBe(2);
    expect(
      at(exceptions, 'cosmeticFilteringV2', 'example.com,*')['setting']['cosmeticFilteringV2'],
    ).toBe(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('ads aggressive triple-key write', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['set', 'example.com', 'ads', 'aggressive'], tmp);
    expect(success).toBe(true);
    const exceptions = exceptionsOf(readPrefs(tmp));
    expect(at(exceptions, 'shieldsAds', 'example.com,*')['setting']).toBe(2);
    expect(at(exceptions, 'trackers', 'example.com,*')['setting']).toBe(2);
    expect(
      at(exceptions, 'cosmeticFilteringV2', 'example.com,*')['setting']['cosmeticFilteringV2'],
    ).toBe(1);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('ads disabled triple-key write', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['set', 'example.com', 'ads', 'disabled'], tmp);
    expect(success).toBe(true);
    const exceptions = exceptionsOf(readPrefs(tmp));
    expect(at(exceptions, 'shieldsAds', 'example.com,*')['setting']).toBe(1);
    expect(at(exceptions, 'trackers', 'example.com,*')['setting']).toBe(1);
    expect(
      at(exceptions, 'cosmeticFilteringV2', 'example.com,*')['setting']['cosmeticFilteringV2'],
    ).toBe(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('overwrites existing', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: { 'example.com,*': { setting: 1, last_modified: '100' } },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['set', 'example.com', 'shields', 'off'], tmp);
    expect(success).toBe(true);
    const entry = at(exceptionsOf(readPrefs(tmp)), 'braveShields', 'example.com,*');
    expect(entry['setting']).toBe(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('generates last_modified', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['set', 'example.com', 'shields', 'on'], tmp);
    expect(success).toBe(true);
    const entry = at(exceptionsOf(readPrefs(tmp)), 'braveShields', 'example.com,*');
    const lm: string = entry['last_modified'];
    expect(lm.length).toBeGreaterThan(0);
    expect(lm).toMatch(/^\d+$/);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('preserves other domains', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: {
              'other.com,*': { setting: 1, last_modified: '100' },
            },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['set', 'example.com', 'shields', 'off'], tmp);
    expect(success).toBe(true);
    const shields = at(exceptionsOf(readPrefs(tmp)), 'braveShields');
    expect(shields['other.com,*']['setting']).toBe(1);
    expect(shields['example.com,*']['setting']).toBe(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== LIST command tests ====================

describe('list', () => {
  it('empty', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)).toHaveLength(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('multiple domains', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: {
              'a.com,*': { setting: 2, last_modified: '1' },
              'b.com,*': { setting: 1, last_modified: '2' },
            },
            fingerprintingV2: {
              'c.com,*': { setting: 1, last_modified: '3' },
            },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { stdout, success } = runCmd(['list'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output).toHaveLength(3);
    const domains = output.map((e: any) => e['domain']);
    expect(domains).toContain('a.com');
    expect(domains).toContain('b.com');
    expect(domains).toContain('c.com');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('deduplicates domains', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: { 'example.com,*': { setting: 2, last_modified: '1' } },
            fingerprintingV2: { 'example.com,*': { setting: 1, last_modified: '2' } },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { stdout, success } = runCmd(['list'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output).toHaveLength(1);
    expect(output[0]['domain']).toBe('example.com');
    expect(output[0]['overrides']['shields']).toBe('off');
    expect(output[0]['overrides']['fingerprinting']).toBe('disabled');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('json format', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['list', '--format', 'json'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)).toBeInstanceOf(Array);
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== RESET command tests ====================

describe('reset', () => {
  it('all for domain', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: { 'example.com,*': { setting: 2, last_modified: '1' } },
            fingerprintingV2: { 'example.com,*': { setting: 1, last_modified: '2' } },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['reset', 'example.com'], tmp);
    expect(success).toBe(true);
    const exceptions = exceptionsOf(readPrefs(tmp));
    expect(exceptions['braveShields']['example.com,*']).toBeUndefined();
    expect(exceptions['fingerprintingV2']['example.com,*']).toBeUndefined();
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('single setting', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: { 'example.com,*': { setting: 2, last_modified: '1' } },
            fingerprintingV2: { 'example.com,*': { setting: 1, last_modified: '2' } },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['reset', 'example.com', 'fingerprinting'], tmp);
    expect(success).toBe(true);
    const exceptions = exceptionsOf(readPrefs(tmp));
    expect(exceptions['fingerprintingV2']['example.com,*']).toBeUndefined();
    expect(exceptions['braveShields']['example.com,*']['setting']).toBe(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('ads removes all three', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            shieldsAds: { 'example.com,*': { setting: 2, last_modified: '1' } },
            trackers: { 'example.com,*': { setting: 2, last_modified: '1' } },
            cosmeticFilteringV2: {
              'example.com,*': { setting: { cosmeticFilteringV2: 2 }, last_modified: '1' },
            },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['reset', 'example.com', 'ads'], tmp);
    expect(success).toBe(true);
    const exceptions = exceptionsOf(readPrefs(tmp));
    expect(exceptions['shieldsAds']['example.com,*']).toBeUndefined();
    expect(exceptions['trackers']['example.com,*']).toBeUndefined();
    expect(exceptions['cosmeticFilteringV2']['example.com,*']).toBeUndefined();
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('nonexistent domain succeeds', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['reset', 'nonexistent.com'], tmp);
    expect(success).toBe(true);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('preserves other domains', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: {
            braveShields: {
              'example.com,*': { setting: 2, last_modified: '1' },
              'other.com,*': { setting: 1, last_modified: '2' },
            },
          },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['reset', 'example.com'], tmp);
    expect(success).toBe(true);
    const shields = at(exceptionsOf(readPrefs(tmp)), 'braveShields');
    expect(shields['example.com,*']).toBeUndefined();
    expect(shields['other.com,*']['setting']).toBe(1);
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== Round-trip tests ====================

describe('round-trips', () => {
  it('set then get', () => {
    const tmp = setupFixture({});
    runCmd(['set', 'example.com', 'shields', 'off'], tmp);
    const { stdout, success } = runCmd(['get', 'example.com'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['settings']['shields']).toBe('off');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('set then list', () => {
    const tmp = setupFixture({});
    runCmd(['set', 'a.com', 'shields', 'off'], tmp);
    runCmd(['set', 'b.com', 'fingerprinting', 'disabled'], tmp);
    const { stdout, success } = runCmd(['list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)).toHaveLength(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('set then reset then get', () => {
    const tmp = setupFixture({});
    runCmd(['set', 'example.com', 'shields', 'off'], tmp);
    runCmd(['reset', 'example.com'], tmp);
    const { stdout, success } = runCmd(['get', 'example.com'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['settings']['shields']).toBe('default');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('multiple sets then list', () => {
    const tmp = setupFixture({});
    runCmd(['set', 'example.com', 'shields', 'off'], tmp);
    runCmd(['set', 'example.com', 'fingerprinting', 'aggressive'], tmp);
    runCmd(['set', 'example.com', 'scripts', 'block'], tmp);
    const { stdout, success } = runCmd(['list'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output).toHaveLength(1);
    expect(output[0]['domain']).toBe('example.com');
    expect(output[0]['overrides']['shields']).toBe('off');
    expect(output[0]['overrides']['fingerprinting']).toBe('aggressive');
    expect(output[0]['overrides']['scripts']).toBe('block');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== FILTERS command tests ====================

function localStateWithFilters(filters: string): object {
  return {
    profile: {
      last_used: 'Default',
      info_cache: {
        Default: { name: 'Test Profile' },
      },
    },
    brave: {
      ad_block: {
        custom_filters: filters,
      },
    },
  };
}

describe('filters', () => {
  it('list empty', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['custom_filters']).toHaveLength(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('list with entries', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||example.com^\nexample.com##h1'));
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    const filters = JSON.parse(stdout)['custom_filters'];
    expect(filters).toHaveLength(2);
    expect(filters[0]).toBe('||example.com^');
    expect(filters[1]).toBe('example.com##h1');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('list skips blank lines', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||a.com^\n\n||b.com^\n\n'));
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['custom_filters']).toHaveLength(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['filters', 'add', '||example.com^'], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toContain('||example.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add to existing', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||old.com^'));
    const { success } = runCmd(['filters', 'add', '||new.com^'], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toContain('||old.com^');
    expect(filters).toContain('||new.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add no duplicate', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||example.com^'));
    const { success } = runCmd(['filters', 'add', '||example.com^'], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    const count = filters.split('\n').filter((l: string) => l.trim() === '||example.com^').length;
    expect(count).toBe(1);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('remove', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||a.com^\n||b.com^\n||c.com^'));
    const { success } = runCmd(['filters', 'remove', '||b.com^'], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toContain('||a.com^');
    expect(filters).not.toContain('||b.com^');
    expect(filters).toContain('||c.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('remove nonexistent', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||a.com^'));
    const { stderr, success } = runCmd(['filters', 'remove', '||nope.com^'], tmp);
    expect(success).toBe(true);
    expect(stderr).toContain('not found');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('clear', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||a.com^\n||b.com^'));
    const { success } = runCmd(['filters', 'clear'], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toBe('');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add then list', () => {
    const tmp = setupFixture({});
    runCmd(['filters', 'add', '||example.com^'], tmp);
    runCmd(['filters', 'add', 'example.com##h1'], tmp);
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['custom_filters']).toHaveLength(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add then remove then list', () => {
    const tmp = setupFixture({});
    runCmd(['filters', 'add', '||a.com^'], tmp);
    runCmd(['filters', 'add', '||b.com^'], tmp);
    runCmd(['filters', 'remove', '||a.com^'], tmp);
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    const filters = JSON.parse(stdout)['custom_filters'];
    expect(filters).toHaveLength(1);
    expect(filters[0]).toBe('||b.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('clear then list', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||a.com^\n||b.com^'));
    runCmd(['filters', 'clear'], tmp);
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['custom_filters']).toHaveLength(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('preserves other local state', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||old.com^'));
    runCmd(['filters', 'add', '||new.com^'], tmp);
    expect(readLocalStateFile(tmp)['profile']['last_used']).toBe('Default');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== --pattern flag tests ====================

describe('--pattern flag', () => {
  it('set with raw pattern', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['set', '*,*', 'shields', 'off', '--pattern'], tmp);
    expect(success).toBe(true);
    const entry = at(exceptionsOf(readPrefs(tmp)), 'braveShields', '*,*');
    expect(entry['setting']).toBe(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('get with raw pattern', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: { braveShields: { '*,*': { setting: 2, last_modified: '123' } } },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { stdout, success } = runCmd(['get', '*,*', '--pattern'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['settings']['shields']).toBe('off');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('reset with raw pattern', () => {
    const prefs = {
      profile: {
        content_settings: {
          exceptions: { braveShields: { '*,*': { setting: 2, last_modified: '123' } } },
        },
      },
    };
    const tmp = setupFixture(prefs);
    const { success } = runCmd(['reset', '*,*', '--pattern'], tmp);
    expect(success).toBe(true);
    expect(at(exceptionsOf(readPrefs(tmp)), 'braveShields', '*,*')).toBeUndefined();
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('set then get with pattern', () => {
    const tmp = setupFixture({});
    runCmd(['set', '*,*', 'shields', 'off', '--pattern'], tmp);
    const { stdout, success } = runCmd(['get', '*,*', '--pattern'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['settings']['shields']).toBe('off');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== FILTERS LOAD command tests ====================

function writeRulesFile(dir: string, rules: string[]): string {
  const filePath = path.join(dir, 'rules.txt');
  fs.writeFileSync(filePath, rules.join('\n'));
  return filePath;
}

describe('filters load', () => {
  it('loads rules', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||example.com^', 'example.com##h1']);
    const { success } = runCmd(['filters', 'load', rulesPath], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toContain('||example.com^');
    expect(filters).toContain('example.com##h1');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('replaces existing', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||old.com^'));
    const rulesPath = writeRulesFile(tmp, ['||new.com^']);
    const { success } = runCmd(['filters', 'load', rulesPath], tmp);
    expect(success).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).not.toContain('||old.com^');
    expect(filters).toContain('||new.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('skips blank lines', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '', '||b.com^', '', '']);
    const { success } = runCmd(['filters', 'load', rulesPath], tmp);
    expect(success).toBe(true);
    const { stdout } = runCmd(['filters', 'list'], tmp);
    expect(JSON.parse(stdout)['custom_filters']).toHaveLength(2);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('load then list', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^', '||c.com^']);
    runCmd(['filters', 'load', rulesPath], tmp);
    const { stdout, success } = runCmd(['filters', 'list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['custom_filters']).toHaveLength(3);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('nonexistent file fails', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['filters', 'load', '/nonexistent/rules.txt'], tmp);
    expect(success).toBe(false);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('preserves other local state', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||old.com^'));
    const rulesPath = writeRulesFile(tmp, ['||new.com^']);
    runCmd(['filters', 'load', rulesPath], tmp);
    expect(readLocalStateFile(tmp)['profile']['last_used']).toBe('Default');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== BISECT command tests ====================

describe('bisect', () => {
  it('nonexistent file fails', () => {
    const tmp = setupFixture({});
    const { success } = runCmdWithStdin(['filters', 'bisect', '/nonexistent/rules.txt'], tmp, '');
    expect(success).toBe(false);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('empty file fails', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, []);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, '');
    expect(success).toBe(false);
    expect(stderr).toContain('no filter rules found');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('initial no', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { stdout, stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'n\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stderr).toContain('Nothing to bisect');
    expect(stdout.trim()).toBe('');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('single rule', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||bad.com^']);
    const { stdout, stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stderr).toContain('Only one rule');
    expect(stdout.trim()).toBe('||bad.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('two rules bad is second', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||good.com^', '||bad.com^']);
    const { stdout, stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stdout.trim()).toBe('||bad.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('two rules bad is first', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||bad.com^', '||good.com^']);
    const { stdout, stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\nn\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stdout.trim()).toBe('||bad.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('four rules finds third', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^', '||c.com^', '||d.com^']);
    const { stdout, stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\nn\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stdout.trim()).toBe('||c.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('eight rules', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, [
      '||r0.com^', '||r1.com^', '||r2.com^', '||r3.com^',
      '||r4.com^', '||r5.com^', '||r6.com^', '||r7.com^',
    ]);
    const { stdout, stderr, success } = runCmdWithStdin(
      ['filters', 'bisect', rulesPath], tmp, 'y\ny\nn\ny\n',
    );
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stdout.trim()).toBe('||r5.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('restores original filters', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||original.com^'));
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\ny\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toBe('||original.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('restores filters on initial no', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||original.com^'));
    const rulesPath = writeRulesFile(tmp, ['||a.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'n\ny\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toBe('||original.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('cleans up state files', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\ny\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(fs.existsSync(path.join(tmp, 'bisect-state.json'))).toBe(false);
    expect(fs.existsSync(path.join(tmp, 'bisect-filters.txt'))).toBe(false);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('quit', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { stdout, stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\nq\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stderr).toContain('Quitting bisect');
    expect(stdout.trim()).toBe('');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('quit does not restore filters', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters('||original.com^'));
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\nq\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).not.toBe('||original.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('eof treated as quit', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, '');
    expect(success).toBe(true);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('resume', () => {
    const tmp = setupFixture({});
    const state = {
      original_filters: '',
      channel: 'release',
      all_rules: ['||a.com^', '||b.com^', '||c.com^'],
      candidate_indices: [1, 2],
      step: 1,
    };
    const statePath = path.join(tmp, 'saved-state.json');
    fs.writeFileSync(statePath, JSON.stringify(state, null, 2));

    const { stdout, stderr, success } = runCmdWithStdin(
      ['filters', 'bisect', '--resume', statePath], tmp, 'y\ny\n',
    );
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stderr).toContain('Resuming bisect');
    expect(stdout.trim()).toBe('||c.com^');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('resume channel mismatch fails', () => {
    const tmp = setupFixture({});
    const state = {
      original_filters: '',
      channel: 'nightly',
      all_rules: ['||a.com^'],
      candidate_indices: [0],
      step: 0,
    };
    const statePath = path.join(tmp, 'saved-state.json');
    fs.writeFileSync(statePath, JSON.stringify(state, null, 2));

    const { stderr, success } = runCmdWithStdin(
      ['filters', 'bisect', '--resume', statePath], tmp, '',
    );
    expect(success).toBe(false);
    expect(stderr).toContain('does not match');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('resume invalid state file fails', () => {
    const tmp = setupFixture({});
    const statePath = path.join(tmp, 'bad-state.json');
    fs.writeFileSync(statePath, 'not valid json');

    const { stderr, success } = runCmdWithStdin(
      ['filters', 'bisect', '--resume', statePath], tmp, '',
    );
    expect(success).toBe(false);
    expect(stderr).toContain('failed to parse bisect state');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('loads filters into local state', () => {
    const tmp = setupFixtureWithLocalState({}, localStateWithFilters(''));
    const rulesPath = writeRulesFile(tmp, ['||only.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    const filters = readLocalStateFile(tmp)['brave']['ad_block']['custom_filters'];
    expect(filters).toBe('');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('stderr shows step count', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^', '||c.com^', '||d.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\nn\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stderr).toContain('Bisecting 4 rules');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('found message in stderr', () => {
    const tmp = setupFixture({});
    const rulesPath = writeRulesFile(tmp, ['||a.com^', '||b.com^']);
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect', rulesPath], tmp, 'y\ny\n');
    expect(success, `stderr: ${stderr}`).toBe(true);
    expect(stderr).toContain('Found problematic rule');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('no file without resume fails', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmdWithStdin(['filters', 'bisect'], tmp, '');
    expect(success).toBe(false);
    expect(stderr).toContain('filter rules file is required');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('resume corrupted all_rules fails', () => {
    const tmp = setupFixture({});
    const state = {
      original_filters: '',
      channel: 'release',
      all_rules: ['||a.com^', 42, '||c.com^'],
      candidate_indices: [0, 1, 2],
      step: 1,
    };
    const statePath = path.join(tmp, 'bad-rules-state.json');
    fs.writeFileSync(statePath, JSON.stringify(state, null, 2));

    const { stderr, success } = runCmdWithStdin(
      ['filters', 'bisect', '--resume', statePath], tmp, '',
    );
    expect(success).toBe(false);
    expect(stderr).toContain('non-string');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== SCRIPTLETS command tests ====================

function writeJsFile(dir: string, filename: string, content: string): string {
  const filePath = path.join(dir, filename);
  fs.writeFileSync(filePath, content);
  return filePath;
}

describe('scriptlets', () => {
  it('list empty', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['scriptlets', 'list'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)).toHaveLength(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add and list', () => {
    const tmp = setupFixture({});
    const jsPath = writeJsFile(tmp, 'test.js', "console.log('hi')");
    let { success } = runCmd(['scriptlets', 'add', 'user-test.js', jsPath], tmp);
    expect(success).toBe(true);
    const { stdout, success: ok } = runCmd(['scriptlets', 'list'], tmp);
    expect(ok).toBe(true);
    const output = JSON.parse(stdout);
    expect(output).toHaveLength(1);
    expect(output[0]['name']).toBe('user-test.js');
    expect(output[0]['size']).toBe(17);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('get', () => {
    const tmp = setupFixture({});
    const jsPath = writeJsFile(tmp, 'test.js', "console.log('hello')");
    runCmd(['scriptlets', 'add', 'user-test.js', jsPath], tmp);
    const { stdout, success } = runCmd(['scriptlets', 'get', 'user-test.js'], tmp);
    expect(success).toBe(true);
    expect(stdout).toBe("console.log('hello')");
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('get nonexistent fails', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(['scriptlets', 'get', 'user-nope.js'], tmp);
    expect(success).toBe(false);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add replaces existing', () => {
    const tmp = setupFixture({});
    const js1 = writeJsFile(tmp, 'v1.js', '// version 1');
    runCmd(['scriptlets', 'add', 'user-test.js', js1], tmp);
    const js2 = writeJsFile(tmp, 'v2.js', '// version 2');
    runCmd(['scriptlets', 'add', 'user-test.js', js2], tmp);
    const { stdout, success } = runCmd(['scriptlets', 'list'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output).toHaveLength(1);
    expect(output[0]['name']).toBe('user-test.js');
    expect(output[0]['size']).toBe(12);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('remove', () => {
    const tmp = setupFixture({});
    const jsPath = writeJsFile(tmp, 'test.js', '// test');
    runCmd(['scriptlets', 'add', 'user-test.js', jsPath], tmp);
    const { stderr, success } = runCmd(['scriptlets', 'remove', 'user-test.js'], tmp);
    expect(success).toBe(true);
    expect(stderr).toContain('Removed');
    const output = JSON.parse(runCmd(['scriptlets', 'list'], tmp).stdout);
    expect(output).toHaveLength(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('remove nonexistent', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmd(['scriptlets', 'remove', 'user-nope.js'], tmp);
    expect(success).toBe(true);
    expect(stderr).toContain('not found');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('clear', () => {
    const tmp = setupFixture({});
    const js1 = writeJsFile(tmp, 'a.js', '// a');
    const js2 = writeJsFile(tmp, 'b.js', '// b');
    runCmd(['scriptlets', 'add', 'user-a.js', js1], tmp);
    runCmd(['scriptlets', 'add', 'user-b.js', js2], tmp);
    const { stderr, success } = runCmd(['scriptlets', 'clear'], tmp);
    expect(success).toBe(true);
    expect(stderr).toContain('Cleared');
    const output = JSON.parse(runCmd(['scriptlets', 'list'], tmp).stdout);
    expect(output).toHaveLength(0);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add invalid name fails', () => {
    const tmp = setupFixture({});
    const jsPath = writeJsFile(tmp, 'test.js', '// test');
    const { success } = runCmd(['scriptlets', 'add', 'bad-name.js', jsPath], tmp);
    expect(success).toBe(false);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add nonexistent file fails', () => {
    const tmp = setupFixture({});
    const { success } = runCmd(
      ['scriptlets', 'add', 'user-test.js', '/nonexistent/script.js'], tmp,
    );
    expect(success).toBe(false);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('add path separator rejected', () => {
    const tmp = setupFixture({});
    const jsPath = writeJsFile(tmp, 'test.js', '// test');
    const { stderr, success } = runCmd(
      ['scriptlets', 'add', 'user-../../etc/passwd.js', jsPath], tmp,
    );
    expect(success).toBe(false);
    expect(stderr).toContain('path separator');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('list table format', () => {
    const tmp = setupFixture({});
    const jsPath = writeJsFile(tmp, 'test.js', "console.log('hi')");
    runCmd(['scriptlets', 'add', 'user-test.js', jsPath], tmp);
    const { stdout, success } = runCmd(['scriptlets', 'list', '--format', 'table'], tmp);
    expect(success).toBe(true);
    expect(stdout).toContain('user-test.js');
    expect(stdout).toContain('bytes');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== PROFILES command ====================

describe('profiles', () => {
  it('lists profiles from Local State', () => {
    const tmp = setupFixture({});
    const { stdout, success } = runCmd(['profiles'], tmp);
    expect(success).toBe(true);
    const output = JSON.parse(stdout);
    expect(output).toHaveLength(1);
    expect(output[0]['dir']).toBe('Default');
    expect(output[0]['name']).toBe('Test Profile');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('respects --profile dir name', () => {
    const tmp = setupFixtureWithLocalState(
      {},
      {
        profile: {
          last_used: 'Profile 1',
          info_cache: {
            Default: { name: 'Person 1' },
            'Profile 1': { name: 'Work' },
          },
        },
      },
    );
    // Preferences for the resolved profile dir
    fs.mkdirSync(path.join(tmp, 'Profile 1'), { recursive: true });
    fs.writeFileSync(path.join(tmp, 'Profile 1', 'Preferences'), JSON.stringify({}));
    const { stdout, success } = runCmd(['--profile', 'Work', 'get', 'example.com'], tmp);
    expect(success).toBe(true);
    expect(JSON.parse(stdout)['profile']).toBe('Work');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== FORMAT handling ====================

describe('format handling', () => {
  it('unknown format errors', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmd(['get', 'example.com', '--format', 'yaml'], tmp);
    expect(success).toBe(false);
    expect(stderr).toContain("unknown format 'yaml'");
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('unknown setting errors', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmd(['set', 'example.com', 'nope', 'on'], tmp);
    expect(success).toBe(false);
    expect(stderr).toContain("unknown setting 'nope'");
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('unknown value errors', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmd(['set', 'example.com', 'shields', 'maybe'], tmp);
    expect(success).toBe(false);
    expect(stderr).toContain("invalid value 'maybe' for shields");
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

// ==================== error messages / profile resolution ====================

describe('error handling', () => {
  it('invalid domain errors', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmd(['set', 'exämple.com', 'shields', 'on'], tmp);
    expect(success).toBe(false);
    expect(stderr).toContain('invalid domain');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('unknown profile errors listing available', () => {
    const tmp = setupFixture({});
    const { stderr, success } = runCmd(['--profile', 'Nope', 'get', 'example.com'], tmp);
    expect(success).toBe(false);
    expect(stderr).toContain("profile 'Nope' not found. Available profiles: 'Test Profile' (Default)");
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});