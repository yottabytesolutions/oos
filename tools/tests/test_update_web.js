// SPDX-License-Identifier: GPL-2.0-or-later
// Exercise the actual embedded page script with in-memory browser and HTTP fakes.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../../ui/src/ui/update/upload.rs'), 'utf8');
const script = source.match(/<script>\n([\s\S]*?)\n<\/script>/)[1].replaceAll('{{', '{').replaceAll('}}', '}');

function browser(approve, mismatch = true) {
    const file = { size: 42 };
    const input = { files: [file] };
    const output = { textContent: '' };
    const button = { disabled: false };
    const requests = [], prompts = [], timers = [];
    const context = {
        document: {
            getElementById: id => id === 'f' ? input : output,
            querySelector: () => button,
        },
        fetch: async (url, options) => {
            requests.push({ url, body: options.body });
            const confirmation = mismatch && url === '/update';
            return {
                status: confirmation ? 409 : 200,
                ok: !confirmation,
                headers: { get: () => confirmation ? 'application/json' : 'text/plain' },
                json: async () => ({ refused: true, package_unit: 'other-unit', running_unit: 'this-unit' }),
                text: async () => 'accepted',
            };
        },
        confirm: text => { prompts.push(text); input.files = [{ size: 99 }]; return approve; },
        setTimeout: callback => timers.push(callback),
        location: { reload: () => assert.fail('reload must remain deferred') },
    };
    vm.createContext(context);
    vm.runInContext(script, context);
    return { context, file, input, output, button, requests, prompts, timers };
}

test('cancelling a mismatch names both units and sends no override', async () => {
    const b = browser(false);
    await b.context.up();
    assert.deepEqual(b.requests.map(r => r.url), ['/update']);
    assert.match(b.prompts[0], /Package unit 'other-unit', running image unit 'this-unit'/);
    assert.match(b.output.textContent, /Cancelled; nothing installed/);
    assert.equal(b.button.disabled, false);
    assert.equal(b.timers.length, 0);
});

test('confirmation resends the same file and consent is requested for every upload', async () => {
    const b = browser(true);
    await b.context.up();
    assert.deepEqual(b.requests.map(r => r.url), ['/update', '/update?any-unit=1']);
    assert.ok(b.requests.every(r => r.body === b.file));
    assert.match(b.prompts[0], /override will be logged/);
    await b.context.up();
    assert.equal(b.prompts.length, 2);
    assert.equal(b.requests[2].url, '/update');
    assert.equal(b.button.disabled, false);
});

test('matching packages install without an override or confirmation', async () => {
    const b = browser(false, false);
    await b.context.up();
    assert.deepEqual(b.requests.map(r => r.url), ['/update']);
    assert.equal(b.prompts.length, 0);
    assert.equal(b.output.textContent, 'accepted');
    assert.equal(b.timers.length, 1);
});

test('transport errors release the button without retrying', async () => {
    const b = browser(true);
    b.context.fetch = async () => { throw new Error('connection lost'); };
    await b.context.up();
    assert.match(b.output.textContent, /Upload failed: connection lost/);
    assert.equal(b.button.disabled, false);
    assert.equal(b.prompts.length, 0);
    assert.equal(b.timers.length, 0);
});

test('other HTTP refusals do not ask for an override or schedule a reload', async () => {
    const b = browser(true);
    b.context.fetch = async () => ({ status: 409, ok: false,
        headers: { get: () => 'text/plain' }, text: async () => 'REFUSED: invalid signature' });
    await b.context.up();
    assert.equal(b.output.textContent, 'REFUSED: invalid signature');
    assert.equal(b.prompts.length, 0);
    assert.equal(b.timers.length, 0);
    assert.equal(b.button.disabled, false);
});

test('JSON without an explicit refusal cannot authorize an override', async () => {
    const b = browser(true);
    b.context.fetch = async () => ({ status: 409, ok: false,
        headers: { get: () => 'application/json' }, json: async () => ({}) });
    await b.context.up();
    assert.match(b.output.textContent, /Invalid refusal response/);
    assert.equal(b.prompts.length, 0);
    assert.equal(b.timers.length, 0);
});
