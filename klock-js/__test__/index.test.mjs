import test from 'node:test';
import assert from 'node:assert';
import klockModule from '../index.js';

const { Klock, KlockClient, KlockHttpClient } = klockModule;

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

    await t.test('Klock.embedded should protect file callbacks and release on success', async () => {
        const klock = Klock.embedded({ agentId: 'facade-agent', sessionId: 'facade-session', priority: 100 });
        const result = await klock.withFile('/facade.ts', { mode: 'mutate' }, async () => 'done');
        assert.strictEqual(result, 'done');

        const second = await klock.withFile('/facade.ts', { mode: 'read' }, () => 'read-ok');
        assert.strictEqual(second, 'read-ok');
    });

    await t.test('Klock.embedded should release on callback error', async () => {
        const klock = Klock.embedded({ agentId: 'facade-error', sessionId: 'facade-error-session', priority: 100 });
        await assert.rejects(
            () => klock.withFile('/error.ts', { mode: 'mutate' }, () => {
                throw new Error('boom');
            }),
            /boom/
        );

        const result = await klock.withFile('/error.ts', { mode: 'read' }, () => 'released');
        assert.strictEqual(result, 'released');
    });

    await t.test('Klock.local should delegate to the HTTP client facade', async () => {
        const originalFetch = global.fetch;
        const seen = [];
        const payloads = [
            { success: true, data: 'registered' },
            {
                success: true,
                data: {
                    lease_id: 'lease-local-1',
                    agent_id: 'local-agent',
                    resource: 'FILE:/local.ts',
                    predicate: 'MUTATES',
                    expires_at: 1234,
                },
            },
            { success: true },
        ];

        global.fetch = async (_url, options = {}) => {
            if (options.body) {
                seen.push(JSON.parse(options.body));
            }
            return {
                ok: true,
                text: async () => JSON.stringify(payloads.shift()),
            };
        };

        try {
            const klock = Klock.local({
                agentId: 'local-agent',
                sessionId: 'local-session',
                priority: 100,
                baseUrl: 'https://klock.example.test',
                autoStart: false,
            });
            const result = await klock.withFile('/local.ts', { mode: 'mutate' }, () => 'ok');
            assert.strictEqual(result, 'ok');
            assert.strictEqual(seen[0].agent_id, 'local-agent');
            assert.strictEqual(seen[1].predicate, 'MUTATES');
            assert.strictEqual(seen[1].resource_path, '/local.ts');
        } finally {
            global.fetch = originalFetch;
        }
    });
});
