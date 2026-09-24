import { PortError } from '../errors.js';
import { describeIoError } from '../fsutil.js';
import { sortedStringify } from '../json.js';
import * as fs from 'node:fs';
import { checkBraveNotRunning } from '../preferences.js';
import type { Channel } from '../platform.js';
import {
  addScriptlet,
  clearScriptlets,
  readScriptlets,
  removeScriptlet,
} from '../scriptlets.js';
import type { OutputFormat, RunResult } from '../output.js';
import Table from 'cli-table3';

export async function scriptletsList(
  braveDir: string,
  format: OutputFormat,
): Promise<string> {
  const entries = await readScriptlets(braveDir);
  if (format === 'json') {
    const output = entries.map((s) => ({
      name: s.name,
      size: Buffer.byteLength(s.content, 'utf8'),
    }));
    return sortedStringify(output);
  }
  if (entries.length === 0) {
    return 'No custom scriptlets.';
  }
  const table = new Table({ head: ['Name', 'Size'] });
  for (const s of entries) {
    table.push([s.name, `${Buffer.byteLength(s.content, 'utf8')} bytes`]);
  }
  return table.toString();
}

export async function scriptletsGet(braveDir: string, name: string): Promise<string> {
  const entries = await readScriptlets(braveDir);
  const found = entries.find((s) => s.name === name);
  if (found === undefined) {
    throw new PortError(`scriptlet '${name}' not found`);
  }
  return found.content;
}

export async function scriptletsAdd(
  braveDir: string,
  name: string,
  jsFile: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  let content: string;
  try {
    content = fs.readFileSync(jsFile, 'utf8');
  } catch (err) {
    throw new PortError(`failed to read ${jsFile}: ${describeIoError(err)}`);
  }

  const { warning } = await checkBraveNotRunning(channel, force);
  await addScriptlet(braveDir, name, content);
  return { warning, message: `Added scriptlet: ${name}` };
}

export async function scriptletsRemove(
  braveDir: string,
  name: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const { warning } = await checkBraveNotRunning(channel, force);
  const removed = await removeScriptlet(braveDir, name);
  return {
    warning,
    message: removed ? `Removed scriptlet: ${name}` : `Scriptlet not found: ${name}`,
  };
}

export async function scriptletsClear(
  braveDir: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const { warning } = await checkBraveNotRunning(channel, force);
  await clearScriptlets(braveDir);
  return { warning, message: 'Cleared all custom scriptlets' };
}