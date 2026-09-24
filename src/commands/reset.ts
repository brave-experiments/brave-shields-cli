import { renderReset } from '../output.js';
import type { RunResult } from '../output.js';
import {
  checkBraveNotRunning,
  getExceptionEntries,
  getExceptionEntriesMut,
  lockedReadModifyWrite,
} from '../preferences.js';
import type { Channel } from '../platform.js';
import { ALL_SETTINGS, jsonKeys, resolvePattern, shieldSettingFromCli } from '../shields.js';
import type { ShieldSetting } from '../shields.js';

export async function resetRun(
  prefsPath: string,
  domain: string,
  rawPattern: boolean,
  settingName: string | undefined,
  channel: Channel,
  force: boolean,
): Promise<RunResult> {
  const pattern = resolvePattern(domain, rawPattern);

  const settingsToReset: ShieldSetting[] =
    settingName === undefined ? [...ALL_SETTINGS] : [shieldSettingFromCli(settingName)];

  const { warning } = await checkBraveNotRunning(channel, force);

  lockedReadModifyWrite(prefsPath, (prefs) => {
    for (const setting of settingsToReset) {
      for (const jsonKey of jsonKeys(setting)) {
        const entries = getExceptionEntries(prefs, jsonKey);
        if (entries !== null && pattern in entries) {
          const entriesMut = getExceptionEntriesMut(prefs, jsonKey);
          delete entriesMut[pattern];
        }
      }
    }
  });

  return { warning, message: renderReset(domain, settingName) };
}