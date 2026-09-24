import { chromiumTimeNow } from '../chromium-time.js';
import { renderSet } from '../output.js';
import type { RunResult } from '../output.js';
import {
  checkBraveNotRunning,
  getExceptionEntriesMut,
  lockedReadModifyWrite,
} from '../preferences.js';
import type { Channel } from '../platform.js';
import { PortError } from '../errors.js';
import { resolvePattern, shieldSettingFromCli, toStored, validValues } from '../shields.js';

export async function setRun(
  prefsPath: string,
  domain: string,
  rawPattern: boolean,
  settingName: string,
  value: string,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const setting = shieldSettingFromCli(settingName);

  // Validate the value
  if (!validValues(setting).includes(value)) {
    throw new PortError(
      `invalid value '${value}' for ${settingName}. Valid values: ${validValues(setting).join(', ')}`,
    );
  }

  const pattern = resolvePattern(domain, rawPattern);
  const timestamp = chromiumTimeNow();
  const entries = toStored(setting, value, timestamp);

  const { warning } = await checkBraveNotRunning(channel, force);

  lockedReadModifyWrite(prefsPath, (prefs) => {
    for (const [jsonKey, entryValue] of entries) {
      const exceptionEntries = getExceptionEntriesMut(prefs, jsonKey);
      exceptionEntries[pattern] = structuredClone(entryValue);
    }
  });

  return { warning, message: renderSet(domain, settingName, value) };
}