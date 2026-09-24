import { renderProfiles } from '../output.js';
import type { OutputFormat } from '../output.js';
import { listProfiles } from '../profile.js';

export function profilesRun(braveDir: string, format: OutputFormat): string {
  return renderProfiles(listProfiles(braveDir), format);
}