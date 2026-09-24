/**
 * Small shared fs/IO helpers used by the JSON persistence layer.
 */

/** Known errno codes mapped to Rust `std::io::Error` Display strings. */
const IO_CODE_MAP: Record<string, string> = {
  ENOENT: 'No such file or directory (os error 2)',
  EPERM: 'Operation not permitted (os error 1)',
  EACCES: 'Permission denied (os error 13)',
  EISDIR: 'Is a directory (os error 21)',
  ENOTDIR: 'Not a directory (os error 20)',
  EEXIST: 'File exists (os error 17)',
  EINVAL: 'Invalid argument (os error 22)',
  EMFILE: 'Too many open files (os error 24)',
  ENOSPC: 'No space left on device (os error 28)',
  ENAMETOOLONG: 'File name too long (os error 36)',
};

/**
 * Describe an error the way `std::io::Error`'s Display does, so context
 * chains like "failed to read Preferences at <path>: <cause>" match the
 * Rust tool's stderr output byte for byte on the common codes.
 */
export function describeIoError(err: unknown): string {
  if (err instanceof Error) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code !== undefined && IO_CODE_MAP[code] !== undefined) {
      return IO_CODE_MAP[code];
    }
    return err.message;
  }
  return String(err);
}