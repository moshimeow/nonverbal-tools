// Run the shipped handlers with a deterministic clock and a minimal DOM.
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const handlers = {}, fields = {}, sends = [], saved = [], timers = new Map();
let now = 1000, timerID = 0, toggles = 0;
const document = {
 addEventListener(n, f) { (handlers[n] ??= []).push(f); },
 getElementById(id) { return fields[id]; },
 querySelector() { return fields.main; }
};
function fire(name, props = {}) {
 const e = { target: { matches: () => false, closest: () => null }, preventDefault() { this.prevented = true; }, stopImmediatePropagation() { this.stopped = true; }, ...props };
 for (const f of handlers[name] ?? []) { f(e); if (e.stopped) break; }
 return e;
}
fields.send = { click: () => sends.push('send') };
fields['mode-toggle'] = { click() { const e = fire('click'); if (!e.prevented) toggles++; } };
fields.conversation = { hidden: false };
fields.main = { dataset: { desktop: "true" } };
fields.message = { id:'message', value:'hello', dataset:{}, style:{}, scrollHeight:900, getClientRects:()=>[1] };
fields['write-gap-change'] = { dispatchEvent() { saved.push(this.value); } };
const preparations = [], composerStates = [];
fields['draft-prepare'] = { dispatchEvent() { preparations.push(this.value); } };
fields['composer-state'] = { dispatchEvent() { composerStates.push(this.value); } };
vm.runInNewContext(readFileSync(`${__dirname}/../src/interaction.js`, 'utf8'), {
 document, window: { scrollBy(){}, addEventListener(){} }, localStorage: { getItem:()=>null },
 performance:{now:()=>now}, scrollY:0, innerHeight:1000, Event:class{}, requestAnimationFrame:f=>f(),
 setTimeout(f,delay) { const id=++timerID; timers.set(id,{f,at:now+delay}); return id; }, clearTimeout:id=>timers.delete(id),
});
function advance(ms) { now+=ms; for(const [id,t] of [...timers]) if(t.at<=now){ timers.delete(id); t.f(); } }
function key(props={}) { return fire('keydown',{target:{...fields.message,matches:()=>false,closest:()=>null},key:'Enter',...props}); }
assert.ok(key().prevented); assert.ok(key({shiftKey:true}).prevented); assert.equal(sends.length,2);
key({isComposing:true}); key({keyCode:229}); fire('compositionend'); key(); assert.equal(sends.length,2);
advance(100); key(); key({repeat:true}); assert.equal(sends.length,3);
assert.equal(fields.message.style.height,'902px'); // No viewport cap.
const target={closest:s=>s==='main'?{}:null};
const down=()=>fire('pointerdown',{target,button:0,isPrimary:true,pointerId:1,clientX:10,clientY:10});
const up=()=>fire('pointerup',{pointerId:1,clientX:10,clientY:10});
const tap=()=>{down();advance(30);return up();};
tap(); assert.equal(toggles,0); advance(100); assert.ok(tap().prevented); assert.equal(toggles,1);
assert.ok(fire('click').prevented); assert.ok(!fire('click').prevented);
down(); advance(551); up(); assert.equal(toggles,1); // Holding no longer switches.
down(); fire('pointermove',{pointerId:1,clientX:10,clientY:40}); up(); tap(); assert.equal(toggles,1);
fire('pointercancel'); tap(); fire('scroll'); tap(); assert.equal(toggles,1);
fire('pointercancel'); tap(); advance(400); tap(); assert.equal(toggles,1);
fire('pointercancel'); tap(); fire('pointerdown',{target:{closest:s=>s==='.spacing-handle'?null:{}},button:0,isPrimary:true}); up(); tap(); assert.equal(toggles,1); // Controls break a double tap.
fire('pointercancel');
fire('keydown',{key:'M',metaKey:true,shiftKey:true}); assert.equal(toggles,2);
const entry={dataset:{id:'42',gap:'0'},style:{}};
const handle={setPointerCapture(){},closest:()=>entry};
fire('pointerdown',{target:{closest:s=>s==='.spacing-handle'?handle:null},pointerId:3,clientY:100});
fire('pointermove',{pointerId:3,clientY:400}); fire('pointerup',{pointerId:3});
assert.equal(entry.style.paddingTop,'300px'); assert.deepEqual(saved,['42:300']);
fire('pointerdown',{target:{closest:s=>s==='.spacing-handle'?handle:null},pointerId:4,clientY:400});
fire('pointermove',{pointerId:4,clientY:200}); fire('pointercancel');
assert.equal(entry.style.paddingTop,'300px'); assert.deepEqual(saved,['42:300']);
const beforeTab = toggles;
assert.ok(key({key:'Tab'}).prevented); assert.equal(toggles, beforeTab+1);
key({key:'Tab',repeat:true}); assert.equal(toggles,beforeTab+1);
assert.ok(!key({key:'Tab',shiftKey:true}).prevented);
assert.ok(!key({key:'Tab',isComposing:true}).prevented);
fields.main.dataset.desktop='false'; assert.ok(!key({key:'Tab'}).prevented);
fields.main.dataset.desktop='true';
const settingsTarget={...fields.message,matches:()=>false,closest:()=>({})};
assert.ok(!key({key:'Tab',target:settingsTarget}).prevented);
console.log('PASS: Enter/IME, growing composer, double taps, spacing, desktop Tab, Shift-Tab navigation, settings focus, iPhone exclusion');

advance(1000); preparations.length = 0;
for (const value of ['h', 'he', 'hel', 'hello']) {
 fields.message.value = value;
 assert.ok(fire('input', {target:fields.message}).stopped);
 advance(100);
}
assert.deepEqual(composerStates, ['true']);
assert.equal(preparations.length, 0);
advance(650); assert.deepEqual(preparations, ['hello']);
fields.message.value = '';
fire('input', {target:fields.message});
assert.deepEqual(composerStates, ['true', 'false']);
assert.equal(preparations.at(-1), '');
fields.message.value = '你好';
fire('compositionstart'); fire('input', {target:fields.message,isComposing:true});
const count = preparations.length;
advance(700); assert.equal(preparations.length, count);
fire('compositionend'); advance(650); assert.equal(preparations.at(-1), '你好');
console.log('PASS: keystrokes stay local, state changes only on empty/nonempty, preparation debounce and IME');
