import { describe, expect, it } from 'vitest';
import { listProfiles, preferencesPath, resolveProfile } from '../src/profile.js';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

function tmpdir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'brave-profile-'));
}

function createFixture(tmp: string): void {
  const localState = {
    profile: {
      last_used: 'Default',
      info_cache: {
        Default: { name: 'Person 1' },
        'Profile 1': { name: 'Work' },
        'Profile 2': { name: 'Personal' },
      },
    },
  };
  fs.writeFileSync(path.join(tmp, 'Local State'), JSON.stringify(localState, null, 2));
}

describe('resolveProfile', () => {
  it('resolve by display name', () => {
    const tmp = tmpdir();
    createFixture(tmp);
    const profile = resolveProfile(tmp, 'Work');
    expect(profile.dirName).toBe('Profile 1');
    expect(profile.displayName).toBe('Work');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('resolve by dir name', () => {
    const tmp = tmpdir();
    createFixture(tmp);
    const profile = resolveProfile(tmp, 'Profile 1');
    expect(profile.dirName).toBe('Profile 1');
    expect(profile.displayName).toBe('Work');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('resolve default profile (last_used)', () => {
    const tmp = tmpdir();
    createFixture(tmp);
    const profile = resolveProfile(tmp);
    expect(profile.dirName).toBe('Default');
    expect(profile.displayName).toBe('Person 1');
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('unknown profile errors with available list', () => {
    const tmp = tmpdir();
    createFixture(tmp);
    expect(() => resolveProfile(tmp, 'Nonexistent')).toThrow(
      /profile 'Nonexistent' not found\. Available profiles: /,
    );
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('missing last_used errors', () => {
    const tmp = tmpdir();
    fs.writeFileSync(tmp + '/Local State', JSON.stringify({ profile: {} }));
    expect(() => resolveProfile(tmp)).toThrow(/Local State missing profile\.last_used/);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('missing info_cache errors', () => {
    const tmp = tmpdir();
    fs.writeFileSync(tmp + '/Local State', JSON.stringify({ profile: {} }));
    expect(() => resolveProfile(tmp, 'Work')).toThrow(
      /Local State missing profile\.info_cache/,
    );
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

describe('listProfiles', () => {
  it('lists all profiles', () => {
    const tmp = tmpdir();
    createFixture(tmp);
    const profiles = listProfiles(tmp);
    expect(profiles).toHaveLength(3);
    const names = profiles.map((p) => p.displayName);
    expect(names).toContain('Work');
    expect(names).toContain('Personal');
    expect(names).toContain('Person 1');
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

describe('preferencesPath', () => {
  it('joins brave dir, profile dir name, Preferences', () => {
    expect(preferencesPath('/brave', { dirName: 'Default', displayName: 'x' })).toBe(
      path.join('/brave', 'Default', 'Preferences'),
    );
  });
});