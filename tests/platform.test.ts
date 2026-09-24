import { afterEach, describe, expect, it } from 'vitest';
import { braveDirForChannel, resolveBraveDir } from '../src/platform.js';
import { PortError } from '../src/errors.js';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

const origHome = process.env['HOME'];

afterEach(() => {
  if (origHome === undefined) delete process.env['HOME'];
  else process.env['HOME'] = origHome;
});

describe('channel dir names', () => {
  it('uses the per-channel directory suffix', () => {
    // Only meaningful on supported desktop platforms.
    if (process.platform !== 'darwin' && process.platform !== 'linux') return;
    const tmp = fs.mkdtempSync('/tmp/brave-home-');
    process.env['HOME'] = tmp;

    const release = braveDirForChannel('release');
    const beta = braveDirForChannel('beta');
    const nightly = braveDirForChannel('nightly');
    const dev = braveDirForChannel('dev');
    expect(release.endsWith('Brave-Browser')).toBe(true);
    expect(beta.endsWith('Brave-Browser-Beta')).toBe(true);
    expect(nightly.endsWith('Brave-Browser-Nightly')).toBe(true);
    expect(dev.endsWith('Brave-Browser-Development')).toBe(true);

    if (process.platform === 'linux') {
      expect(release).toBe(path.join(tmp, '.config/BraveSoftware', 'Brave-Browser'));
    } else {
      expect(release).toBe(
        path.join(tmp, 'Library/Application Support/BraveSoftware', 'Brave-Browser'),
      );
    }
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});

describe('resolveBraveDir', () => {
  it('override dir', () => {
    const tmp = fs.mkdtempSync('/tmp/brave-override-');
    const result = resolveBraveDir(tmp, 'release');
    expect(result.path).toBe(tmp);
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('override dir nonexistent', () => {
    expect(() => resolveBraveDir('/nonexistent/path/12345', 'release')).toThrow(
      /specified --brave-dir does not exist/,
    );
  });

  it('missing home dir', () => {
    delete process.env['HOME'];
    expect(() => braveDirForChannel('release')).toThrow(
      /could not determine home directory/,
    );
  });

  it('not-installed error names the channel and path', () => {
    const tmp = fs.mkdtempSync('/tmp/brave-home2-');
    process.env['HOME'] = tmp;
    expect(() => resolveBraveDir(undefined, 'nightly')).toThrow(
      /Brave Nightly data directory not found at .+\. Is Brave Nightly installed\?/,
    );
    fs.rmSync(tmp, { recursive: true, force: true });
  });
});