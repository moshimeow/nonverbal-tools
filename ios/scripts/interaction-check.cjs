// Exercise the shipped handlers, including IME confirmation and pointer cancellation.
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const handlers = {}, fields = {}, sends = [];
let now = 1000;
const document = {
 addEventListener(name, handler) { (handlers[name] ??= []).push(handler); },
 getElementById(id) { return fields[id]; }
};
fields['speak-send'] = { click: () => sends.push('speak') };
fields['write-send'] = { click: () => sends.push('write') };
const saved = [];
fields['write-gap-change'] = { dispatchEvent() { saved.push(this.value); } };
vm.runInNewContext(readFileSync(`${__dirname}/../src/interaction.js`, 'utf8'), {
 document, window: { scrollBy() {} }, performance: { now: () => now }, scrollY: 0, innerHeight: 1000, Event: class {},
});
function fire(name, props = {}) {
 const e = { target: { matches: () => false }, preventDefault() { this.prevented = true; }, ...props };
 for (const f of handlers[name] ?? []) f(e);
 return e;
}
function key(id, props = {}) { return fire('keydown', { target: { id, value: 'hello', matches: () => false }, key: 'Enter', ...props }); }
assert.ok(key('message').prevented);
assert.ok(key('write-message', { shiftKey: true }).prevented);
assert.deepEqual(sends, ['speak','write']);
key('write-message', { isComposing: true });
key('write-message', { keyCode: 229 });
fire('compositionend');
assert.ok(key('write-message').prevented);
assert.equal(sends.length, 2);
now += 100;
key('write-message');
key('write-message', { repeat: true });
assert.equal(sends.length, 3);
const entry = { dataset: { id: '42', gap: '0' }, style: {} };
const handle = { setPointerCapture() {}, closest: () => entry };
fire('pointerdown', { target: { closest: () => handle }, pointerId: 1, clientY: 100 });
fire('pointermove', { pointerId: 1, clientY: 400 });
fire('pointerup', { pointerId: 1 });
assert.equal(entry.style.paddingTop, '300px');
assert.deepEqual(saved, ['42:300']);
fire('pointerdown', { target: { closest: () => handle }, pointerId: 2, clientY: 400 });
fire('pointermove', { pointerId: 2, clientY: 200 });
fire('pointercancel');
assert.equal(entry.style.paddingTop, '300px');
assert.deepEqual(saved, ['42:300']);
console.log('PASS: Enter, Shift-Enter, IME confirmation, repeat suppression, spacing drag and cancellation');
