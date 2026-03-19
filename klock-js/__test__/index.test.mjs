import test from 'node:test';
import assert from 'node:assert';
import klockModule from '../index.js';

const { KlockClient, KlockHttpClient } = klockModule;

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

    await t.test('should map HTTP server responses', async () => {
        const originalFetch = global.fetch;
        const payloads = [
            { success: true, data: 'registered' },
            {
                success: true,
                data: {
                    lease_id: 'lease-http-1',
                    agent_id: 'agent-http',
                    resource: 'FILE:/shared.ts',
                    predicate: 'MUTATES',
                    expires_at: 1234,
                },
            },
            {
                success: true,
                data: [
                    {
                        id: 'lease-http-1',
                        agent_id: 'agent-http',
                        resource: 'FILE:/shared.ts',
                        predicate: 'MUTATES',
                        expires_at: 1234,
                    },
                ],
            },
            { success: true },
        ];

        global.fetch = async () => ({
            text: async () => JSON.stringify(payloads.shift()),
        });

        try {
            const httpClient = new KlockHttpClient({ baseUrl: 'https://klock.example.test', autoStart: false });
            await httpClient.registerAgent('agent-http', 100);

            const lease = await httpClient.acquireLease(
                'agent-http',
                'session-http',
                'FILE',
                '/shared.ts',
                'MUTATES',
                60000
            );

            assert.deepStrictEqual(lease, {
                success: true,
                leaseId: 'lease-http-1',
                agentId: 'agent-http',
                resource: 'FILE:/shared.ts',
                predicate: 'MUTATES',
                expiresAt: 1234,
            });

            const leases = await httpClient.listLeases();
            assert.deepStrictEqual(leases, [
                {
                    id: 'lease-http-1',
                    agentId: 'agent-http',
                    resource: 'FILE:/shared.ts',
                    predicate: 'MUTATES',
                    expiresAt: 1234,
                },
            ]);

            const released = await httpClient.releaseLease('lease-http-1');
            assert.strictEqual(released, true);
        } finally {
            global.fetch = originalFetch;
        }
    });

    await t.test('should disable auto-start from env', () => {
        const previous = process.env.KLOCK_DISABLE_AUTOSTART;
        process.env.KLOCK_DISABLE_AUTOSTART = '1';

        try {
            const httpClient = new KlockHttpClient({ baseUrl: 'http://localhost:3100' });
            assert.strictEqual(httpClient.autoStart, false);
            assert.strictEqual(httpClient.autoStartDisabledByEnv, true);
            assert.strictEqual(httpClient.autoStartedPid, null);
        } finally {
            if (previous === undefined) {
                delete process.env.KLOCK_DISABLE_AUTOSTART;
            } else {
                process.env.KLOCK_DISABLE_AUTOSTART = previous;
            }
        }
    });
});
