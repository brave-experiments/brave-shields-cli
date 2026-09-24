/**
 * Base error for all user-facing failures.
 *
 * Every message must exactly reproduce the corresponding Rust
 * `anyhow`/`bail!` message string; the CLI layer prints `Error: <message>`
 * to stderr and exits 1.
 */
export class PortError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'PortError';
  }
}