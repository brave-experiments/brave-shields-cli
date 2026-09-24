#!/usr/bin/env node
/**
 * CLI entry point. Owns argv parsing, process streams, and exit codes.
 * All logic lives in library modules; this file only parses, dispatches,
 * and prints. Exit codes: 0 success, 1 app error (`Error: <msg>` on stderr),
 * 2 usage error (clap parity).
 */
import { Command, CommanderError } from 'commander';
import * as readline from 'node:readline';
import { PortError } from './errors.js';
import { outputFormatFromStr } from './output.js';
import type { OutputFormat, RunResult } from './output.js';
import { DEFAULT_CHANNEL, parseChannel, resolveBraveDir } from './platform.js';
import type { Channel } from './platform.js';
import { preferencesPath, resolveProfile } from './profile.js';
import { getRun } from './commands/get.js';
import { listRun } from './commands/list.js';
import { profilesRun } from './commands/profiles.js';
import { resetRun } from './commands/reset.js';
import { setRun } from './commands/set.js';
import {
  filtersAdd,
  filtersClear,
  filtersList,
  filtersLoad,
  filtersRemove,
} from './commands/filters.js';
import {
  scriptletsAdd,
  scriptletsClear,
  scriptletsGet,
  scriptletsList,
  scriptletsRemove,
} from './commands/scriptlets.js';
import { bisectRun } from './commands/bisect.js';
import type { BisectIo } from './commands/bisect.js';

interface GlobalOpts {
  profile?: string;
  channel?: string;
  braveDir?: string;
  format: string;
  force?: boolean;
}

const CHANNEL_POSSIBLE_VALUES = 'release, beta, nightly, dev';

function printUsageError(message: string, hint: boolean): void {
  process.stderr.write(`${message}\n`);
  if (hint) {
    process.stderr.write('\nFor more information, try \'--help\'.\n');
  }
  process.exitCode = 2;
}

function validateChannel(
  raw: string | undefined,
  fromEnv: boolean,
): Channel | undefined {
  if (raw === undefined) return undefined;
  const parsed = parseChannel(raw);
  if (parsed === null) {
    const name = fromEnv ? 'BRAVE_SHIELDS_CLI_CHANNEL' : '--channel <CHANNEL>';
    printUsageError(
      `error: invalid value '${raw}' for '${name}'\n  [possible values: ${CHANNEL_POSSIBLE_VALUES}]`,
      true,
    );
    throw new ExitNow(2);
  }
  return parsed;
}

class ExitNow extends Error {
  constructor(public readonly code: number) {
    super(`exit ${code}`);
  }
}

function logOut(body: string): void {
  process.stdout.write(`${body}\n`);
}

function logErr(line: string): void {
  process.stderr.write(`${line}\n`);
}

async function main(argv: readonly string[]): Promise<void> {
  const program = new Command();
  program
    .exitOverride()
    .name('brave-shields-cli')
    .description('Read and write Brave Shields per-site preferences')
    .option('--profile <name>', 'Profile display name or directory name (default: last used)')
    .option('--channel <channel>', 'Brave channel: release, beta, nightly, dev')
    .option('--brave-dir <dir>', 'Override Brave data directory (takes precedence over --channel)')
    .option('--format <format>', 'Output format: json or table', 'json')
    .option('--force', 'Write even if Brave is running (changes may be overwritten by the browser)');

  program
    .command('get <domain>')
    .description('Show shield settings for a domain')
    .option('--pattern', 'Treat the domain argument as a raw Chromium match pattern (e.g. "*,*" for global)')
    .action(async (domain: string, opts: { pattern?: boolean }) => {
      await dispatch(program, opts, (ctx) => {
        const body = getRun(
          ctx.prefsPath!,
          domain,
          opts.pattern === true,
          ctx.profileInfo!.displayName,
          ctx.format,
        );
        logOut(body);
      });
    });

  program
    .command('set <domain> <setting> <value>')
    .description('Set a shield setting for a domain')
    .option('--pattern', 'Treat the domain argument as a raw Chromium match pattern (e.g. "*,*" for global)')
    .action(
      async (
        domain: string,
        setting: string,
        value: string,
        opts: { pattern?: boolean },
      ) => {
        await dispatch(program, opts, async (ctx) => {
          await printResult(
            setRun(
              ctx.prefsPath!,
              domain,
              opts.pattern === true,
              setting,
              value,
              ctx.channel,
              ctx.force,
            ),
          );
        });
      },
    );

  program
    .command('list')
    .description('List all per-site shield overrides')
    .action(async () => {
      await dispatch(program, {}, (ctx) => {
        logOut(listRun(ctx.prefsPath!, ctx.format));
      });
    });

  program
    .command('profiles')
    .description('List available browser profiles')
    .action(async () => {
      await dispatch(program, {}, (ctx) => {
        logOut(profilesRun(ctx.braveDir, ctx.format));
      });
    });

  program
    .command('reset <domain> [setting]')
    .description('Reset shield settings for a domain')
    .option('--pattern', 'Treat the domain argument as a raw Chromium match pattern (e.g. "*,*" for global)')
    .action(async (domain: string, setting: string | undefined, opts: { pattern?: boolean }) => {
      await dispatch(program, opts, async (ctx) => {
        await printResult(
          resetRun(ctx.prefsPath!, domain, opts.pattern === true, setting, ctx.channel, ctx.force),
        );
      });
    });

  const filters = program.command('filters').description('Manage custom adblock filter rules');
  filters
    .command('list')
    .description('List all custom filter rules')
    .action(async () => {
      await dispatch(program, {}, (ctx) => {
        logOut(filtersList(ctx.braveDir, ctx.format));
      });
    });
  filters
    .command('add <rule>')
    .description('Add a custom filter rule')
    .action(async (rule: string) => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(filtersAdd(ctx.braveDir, rule, ctx.channel, ctx.force));
      });
    });
  filters
    .command('remove <rule>')
    .description('Remove a custom filter rule')
    .action(async (rule: string) => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(filtersRemove(ctx.braveDir, rule, ctx.channel, ctx.force));
      });
    });
  filters
    .command('clear')
    .description('Remove all custom filter rules')
    .action(async () => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(filtersClear(ctx.braveDir, ctx.channel, ctx.force));
      });
    });
  filters
    .command('load <file>')
    .description('Load custom filter rules from a file (replaces existing filters)')
    .action(async (file: string) => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(filtersLoad(ctx.braveDir, file, ctx.channel, ctx.force));
      });
    });
  filters
    .command('bisect [file]')
    .description('Binary search to find which filter rule causes a site issue')
    .option('--resume <path>', 'Resume a previous bisect session from its state file')
    .option('--output-dir <dir>', 'Directory to write state and working files (default: current directory)')
    .action(
      async (file: string | undefined, opts: { resume?: string; outputDir?: string }) => {
        await dispatch(program, {}, async (ctx) => {
          await runBisect(ctx.braveDir, {
            filePath: file,
            resumePath: opts.resume,
            outputDir: opts.outputDir,
            cwd: process.cwd(),
            channel: ctx.channel,
          });
        });
      },
    );

  const scriptlets = program
    .command('scriptlets')
    .description('Manage custom scriptlets (user-defined JavaScript for adblock injection)');
  scriptlets
    .command('list')
    .description('List all custom scriptlets')
    .action(async () => {
      await dispatch(program, {}, async (ctx) => {
        logOut(await scriptletsList(ctx.braveDir, ctx.format));
      });
    });
  scriptlets
    .command('get <name>')
    .description('Show the content of a specific scriptlet')
    .action(async (name: string) => {
      await dispatch(program, {}, async (ctx) => {
        process.stdout.write(await scriptletsGet(ctx.braveDir, name));
      });
    });
  scriptlets
    .command('add <name> <file>')
    .description('Add or update a custom scriptlet from a JavaScript file')
    .action(async (name: string, file: string) => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(scriptletsAdd(ctx.braveDir, name, file, ctx.channel, ctx.force));
      });
    });
  scriptlets
    .command('remove <name>')
    .description('Remove a custom scriptlet')
    .action(async (name: string) => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(scriptletsRemove(ctx.braveDir, name, ctx.channel, ctx.force));
      });
    });
  scriptlets
    .command('clear')
    .description('Remove all custom scriptlets')
    .action(async () => {
      await dispatch(program, {}, async (ctx) => {
        await printResult(scriptletsClear(ctx.braveDir, ctx.channel, ctx.force));
      });
    });

  try {
    await program.parseAsync(argv, { from: 'node' });
  } catch (err) {
    if (err instanceof CommanderError) {
      // commander already wrote the usage error/help to stderr.
      process.exitCode = err.exitCode === 0 ? 0 : 2;
      return;
    }
    throw err;
  }
}

// --- dispatch helpers ----------------------------------------------------

interface Ctx {
  braveDir: string;
  channel: Channel;
  format: OutputFormat;
  force: boolean;
  profileInfo?: ReturnType<typeof resolveProfile>;
  prefsPath?: string;
}

function getOpts(program: Command): GlobalOpts {
  return program.opts<GlobalOpts & { format: string }>();
}

async function dispatch(
  program: Command,
  _opts: unknown,
  action: (ctx: Ctx) => void | Promise<void>,
): Promise<void> {
  try {
    const opts = getOpts(program);
    // --format validated in app code, not the parser (Rust parity).
    const format = outputFormatFromStr(opts.format);

    // Env var only affects channel default; --channel flag wins over
    // BRAVE_SHIELDS_CLI_CHANNEL.
    let channel = validateChannel(opts.channel, false);
    if (channel === undefined) {
      const envChannel = process.env['BRAVE_SHIELDS_CLI_CHANNEL'];
      if (envChannel !== undefined && envChannel.length > 0) {
        channel = validateChannel(envChannel, true) ?? DEFAULT_CHANNEL;
      }
    }
    const resolvedChannel: Channel = channel ?? DEFAULT_CHANNEL;

    // Precedence: --brave-dir > --channel > env > release-default.
    const resolved = resolveBraveDir(opts.braveDir, resolvedChannel);
    if (resolved.note !== undefined) logErr(resolved.note);

    const ctx: Ctx = {
      braveDir: resolved.path,
      channel: resolvedChannel,
      format,
      force: opts.force === true,
    };

    // profiles / filters / scriptlets skip profile resolution.
    const cmd = program.args[0];
    if (cmd !== 'profiles' && cmd !== 'filters' && cmd !== 'scriptlets') {
      ctx.profileInfo = resolveProfile(resolved.path, opts.profile);
      ctx.prefsPath = preferencesPath(resolved.path, ctx.profileInfo);
    }

    await action(ctx);
  } catch (err) {
    if (err instanceof ExitNow) return;
    if (err instanceof PortError) {
      process.stderr.write(`Error: ${err.message}\n`);
      process.exitCode = 1;
      return;
    }
    if (err instanceof Error) {
      process.stderr.write(`Error: ${err.message}\n`);
      process.exitCode = 1;
      return;
    }
    throw err;
  }
}

async function printResult(promise: Promise<RunResult>): Promise<void> {
  const result = await promise;
  if (result.warning !== undefined) logErr(result.warning);
  if (result.message !== undefined) logErr(result.message);
}

/** Interactive bisect: own streams + injectable Asker + cooperative SIGINT. */
async function runBisect(braveDir: string, opts: Parameters<typeof bisectRun>[1]): Promise<void> {
  const interrupted = { flag: false };
  const onSigint = () => {
    interrupted.flag = true;
  };
  process.on('SIGINT', onSigint);

  const rl = readline.createInterface({ input: process.stdin });

  // Queue every incoming line so a burst of piped input is not lost between
  // prompts (readline emits buffered lines synchronously).
  const lineQueue: (string | null)[] = [];
  let eofReached = false;
  let pendingResolve: ((value: string | null) => void) | null = null;
  const deliver = (value: string | null) => {
    const resolve = pendingResolve;
    pendingResolve = null;
    resolve?.(value);
  };
  rl.on('line', (line) => {
    if (pendingResolve !== null) deliver(line);
    else lineQueue.push(line);
  });
  rl.on('close', () => {
    if (pendingResolve !== null) deliver(null);
    else eofReached = true;
  });
  // The process-level SIGINT handler above sets the flag; keep the prompt
  // alive so it can observe it (matches the Rust ctrlc cooperative behavior).
  rl.on('SIGINT', () => {});

  const io: BisectIo = {
    err: logErr,
    errRaw: (text) => process.stderr.write(text),
    out: (text) => process.stdout.write(text),
    ask: (_message) =>
      new Promise<string | null>((resolve) => {
        if (lineQueue.length > 0) {
          resolve(lineQueue.shift() ?? null);
          return;
        }
        if (eofReached) {
          resolve(null);
          return;
        }
        pendingResolve = resolve;
      }),
    isInterrupted: () => interrupted.flag,
  };

  try {
    await bisectRun(braveDir, opts, io);
  } finally {
    rl.close();
    process.removeListener('SIGINT', onSigint);
  }
}

await main(process.argv);
// Success exits 0 naturally; error paths set process.exitCode above.