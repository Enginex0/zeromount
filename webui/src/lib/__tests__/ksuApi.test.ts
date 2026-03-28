import { describe, it, expect, afterEach, vi } from 'vitest';

vi.mock('kernelsu-alt', () => ({
  exec: vi.fn(),
  listPackages: vi.fn(),
  getPackagesInfo: vi.fn(),
}));

describe('runShell', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
    vi.restoreAllMocks();
  });

  it('resolves with errno -1 when ksu is undefined', async () => {
    vi.stubGlobal('ksu', undefined);
    const { runShell } = await import('../ksuApi');
    const result = await runShell('echo hi');
    expect(result.errno).toBe(-1);
    expect(result.stderr).toBe('KSU not available');
  });

  it('delegates to exec from kernelsu-alt when ksu exists', async () => {
    vi.stubGlobal('ksu', {});
    const { exec } = await import('kernelsu-alt');
    const mockExec = vi.mocked(exec);
    mockExec.mockResolvedValue({ errno: 0, stdout: 'hello', stderr: '' });

    const { runShell } = await import('../ksuApi');
    const result = await runShell('echo hello');
    expect(result.errno).toBe(0);
    expect(result.stdout).toBe('hello');
    expect(mockExec).toHaveBeenCalledWith('echo hello');
  });
});

describe('listPackages', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
    vi.restoreAllMocks();
  });

  it('returns empty array when ksu is undefined', async () => {
    vi.stubGlobal('ksu', undefined);
    const { listPackages } = await import('../ksuApi');
    expect(await listPackages('all')).toEqual([]);
  });
});
