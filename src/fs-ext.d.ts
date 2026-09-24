/** Minimal ambient types for the fs-ext native module (no shipped types). */
declare module 'fs-ext' {
  export const constants: {
    readonly SEEK_SET: 0;
    readonly SEEK_CUR: 1;
    readonly SEEK_END: 2;
    readonly LOCK_SH: 1;
    readonly LOCK_EX: 2;
    readonly LOCK_NB: 4;
    readonly LOCK_UN: 8;
    readonly F_GETFD: 1;
    readonly F_SETFD: 2;
    readonly FD_CLOEXEC: 1;
    readonly F_RDLCK: 0;
    readonly F_WRLCK: 1;
    readonly F_UNLCK: 2;
    readonly F_SETLK: 6;
    readonly F_GETLK: 5;
    readonly F_SETLKW: 7;
  };
  export function flockSync(fd: number, operation: number): void;
  export function flock(
    fd: number,
    operation: number,
    callback: (err: NodeJS.ErrnoException | null) => void,
  ): void;
  export function fcntlSync(fd: number, cmd: number, arg?: unknown): unknown;
  export function seekSync(fd: number, position: number, whence: number): void;
}