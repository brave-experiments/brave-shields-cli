import { afterAll, describe, expect, it } from 'vitest';
import {
  addScriptlet,
  clearScriptlets,
  dbPath,
  readScriptlets,
  removeScriptlet,
} from '../src/scriptlets.js';
import { ClassicLevel } from 'classic-level';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

const tmps: string[] = [];
function tmpdir(): string {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'brave-scl-'));
  tmps.push(tmp);
  return tmp;
}

afterAll(() => {
  for (const tmp of tmps) fs.rmSync(tmp, { recursive: true, force: true });
});

function setupBraveDir(): string {
  const tmp = tmpdir();
  fs.mkdirSync(path.join(tmp, 'Default'));
  return tmp;
}

describe('validateName', () => {
  it('accepts valid names', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, 'user-test.js', 'x')).resolves.toBeUndefined();
    await expect(addScriptlet(tmp, 'user-my-script.js', 'x')).resolves.toBeUndefined();
  });

  it('rejects missing prefix', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, 'test.js', '')).rejects.toThrow(
      /must start with 'user-'/,
    );
  });

  it('rejects missing suffix', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, 'user-test', '')).rejects.toThrow(/must end with '\.js'/);
  });

  it('rejects empty', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, '', '')).rejects.toThrow(/must not be empty/);
  });

  it('rejects non-ascii', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, 'user-tést.js', '')).rejects.toThrow(/must be ASCII/);
  });

  it('rejects path separators and traversal', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, 'user-../../etc/passwd.js', '')).rejects.toThrow(
      /path separators or traversal/,
    );
    await expect(addScriptlet(tmp, 'user-foo/bar.js', '')).rejects.toThrow(
      /path separators or traversal sequences/,
    );
    await expect(addScriptlet(tmp, 'user-foo\\bar.js', '')).rejects.toThrow(
      /path separators or traversal sequences/,
    );
  });
});

describe('read/write LevelDB', () => {
  it('read empty db', async () => {
    const tmp = setupBraveDir();
    expect(await readScriptlets(tmp)).toEqual([]);
  });

  it('add and read', async () => {
    const tmp = setupBraveDir();
    await addScriptlet(tmp, 'user-test.js', "console.log('hi')");
    const scriptlets = await readScriptlets(tmp);
    expect(scriptlets).toHaveLength(1);
    expect(scriptlets[0]!.name).toBe('user-test.js');
    expect(scriptlets[0]!.content).toBe("console.log('hi')");
  });

  it('add multiple', async () => {
    const tmp = setupBraveDir();
    await addScriptlet(tmp, 'user-a.js', '// a');
    await addScriptlet(tmp, 'user-b.js', '// b');
    expect(await readScriptlets(tmp)).toHaveLength(2);
  });

  it('add replaces existing', async () => {
    const tmp = setupBraveDir();
    await addScriptlet(tmp, 'user-test.js', 'old');
    await addScriptlet(tmp, 'user-test.js', 'new');
    const scriptlets = await readScriptlets(tmp);
    expect(scriptlets).toHaveLength(1);
    expect(scriptlets[0]!.content).toBe('new');
  });

  it('remove', async () => {
    const tmp = setupBraveDir();
    await addScriptlet(tmp, 'user-a.js', '// a');
    await addScriptlet(tmp, 'user-b.js', '// b');
    expect(await removeScriptlet(tmp, 'user-a.js')).toBe(true);
    const scriptlets = await readScriptlets(tmp);
    expect(scriptlets).toHaveLength(1);
    expect(scriptlets[0]!.name).toBe('user-b.js');
  });

  it('remove nonexistent', async () => {
    const tmp = setupBraveDir();
    expect(await removeScriptlet(tmp, 'user-nope.js')).toBe(false);
  });

  it('add invalid name rejected', async () => {
    const tmp = setupBraveDir();
    await expect(addScriptlet(tmp, 'bad-name.js', '')).rejects.toThrow();
    await expect(addScriptlet(tmp, 'user-test', '')).rejects.toThrow();
  });

  it('nonexistent db dir returns empty', async () => {
    const tmp = tmpdir();
    expect(await readScriptlets(tmp)).toEqual([]);
  });

  it('add empty content', async () => {
    const tmp = setupBraveDir();
    await addScriptlet(tmp, 'user-empty.js', '');
    const scriptlets = await readScriptlets(tmp);
    expect(scriptlets).toHaveLength(1);
    expect(scriptlets[0]!.name).toBe('user-empty.js');
    expect(scriptlets[0]!.content).toBe('');
  });

  it('clear scriptlets', async () => {
    const tmp = setupBraveDir();
    await addScriptlet(tmp, 'user-a.js', '// a');
    await addScriptlet(tmp, 'user-b.js', '// b');
    await clearScriptlets(tmp);
    expect(await readScriptlets(tmp)).toEqual([]);
  });

  it('clear empty db', async () => {
    const tmp = setupBraveDir();
    await clearScriptlets(tmp);
    expect(await readScriptlets(tmp)).toEqual([]);
  });

  it('read corrupted db', async () => {
    const tmp = setupBraveDir();
    const target = dbPath(tmp);
    fs.mkdirSync(target, { recursive: true });
    const db = new ClassicLevel(target, { createIfMissing: true });
    await db.open();
    await db.put('SCRIPTLETS', 'not valid json');
    await db.close();
    await expect(readScriptlets(tmp)).rejects.toThrow(/failed to parse SCRIPTLETS JSON/);
  });

  it('read missing name field', async () => {
    const tmp = setupBraveDir();
    const target = dbPath(tmp);
    fs.mkdirSync(target, { recursive: true });
    const db = new ClassicLevel(target, { createIfMissing: true });
    await db.open();
    await db.put(
      'SCRIPTLETS',
      JSON.stringify([{ content: 'Y29uc29sZS5sb2coJ2hpJyk=' }]),
    );
    await db.close();
    await expect(readScriptlets(tmp)).rejects.toThrow(/scriptlet missing name/);
  });

  it('read missing content field', async () => {
    const tmp = setupBraveDir();
    const target = dbPath(tmp);
    fs.mkdirSync(target, { recursive: true });
    const db = new ClassicLevel(target, { createIfMissing: true });
    await db.open();
    await db.put('SCRIPTLETS', JSON.stringify([{ name: 'user-x.js' }]));
    await db.close();
    await expect(readScriptlets(tmp)).rejects.toThrow(/scriptlet missing content/);
  });
});