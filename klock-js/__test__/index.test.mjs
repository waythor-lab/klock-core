import test from 'node:test';
import assert from 'node:assert';
import { KlockClient } from '../index.js';

test('klock-js smoke test', async (t) => {
    const client = new KlockClient();

    await t.test('should register an agent', () => {
        client.registerAgent('agent-1', 100);
        // If it doesn't throw, we're good
    });

    await t.test('should acquire and release a lease', () => {
        const rawResult = client.acquireLease(
            'agent-1',
            'session-1',
            'FILE',
            '/app.ts',
            'MUTATES',
            60000
        );
        const result = JSON.parse(rawResult);

        assert.strictEqual(result.success, true, 'Lease acquisition should succeed');
        assert.ok(result.leaseId, 'Lease ID should be present');

        const releaseResult = client.releaseLease(result.leaseId);
        assert.strictEqual(releaseResult, true, 'Lease release should succeed');
    });

    await t.test('should detect conflicts', () => {
        // Acquire a lease for agent-1
        client.acquireLease('agent-1', 's1', 'FILE', '/shared.ts', 'MUTATES', 60000);

        // Attempt to acquire for agent-2 (junior)
        client.registerAgent('agent-2', 200);
        const rawJuniorResult = client.acquireLease('agent-2', 's2', 'FILE', '/shared.ts', 'MUTATES', 60000);
        const juniorResult = JSON.parse(rawJuniorResult);

        assert.strictEqual(juniorResult.success, false, 'Junior should be blocked by conflict');
        assert.strictEqual(juniorResult.reason, 'DIE', 'Junior should DIE per Wait-Die protocol');
    });
});
