/**
 * In-process lock set. Used by the facilitator and the agent server so the
 * same receipt is never processed twice at the same time.
 *
 * Scope: one process. Running several replicas needs a shared lock (Redis,
 * database row). The on-chain status check is the final guard either way.
 */
export class KeyedLock {
  private readonly held = new Set<string>();

  tryAcquire(key: string): boolean {
    if (this.held.has(key)) return false;
    this.held.add(key);
    return true;
  }

  release(key: string): void {
    this.held.delete(key);
  }
}
