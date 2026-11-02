if (!window.speakInteractions) {
    window.speakInteractions = true;
    let compositionEnded = 0;
    document.addEventListener('compositionend', () => compositionEnded = performance.now());
    document.addEventListener('keydown', e => {
        if (performance.now() - compositionEnded < 50 || e.key !== 'Tab' || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || e.isComposing || e.keyCode === 229) return;
        if (document.querySelector('main')?.dataset.desktop !== 'true' || document.getElementById('conversation').hidden || e.target.closest('#settings, #history-view')) return;
        e.preventDefault();
        if (!e.repeat) document.getElementById('mode-toggle').click();
    }, true);
    document.addEventListener('keydown', e => {
        const id = e.target.id;
        if (id !== 'message' || e.key !== 'Enter') return;
        if (e.isComposing || e.keyCode === 229) return;
        if (performance.now() - compositionEnded < 50) { e.preventDefault(); return; }
        e.preventDefault();
        if (e.repeat || !e.target.value.trim()) return;
        const send = document.getElementById('send');
        send.disabled = false;
        send.click();
    }, true);
    window.resizeComposer = () => {
        const field = document.getElementById('message');
        if (!field || !field.getClientRects().length) return;
        field.style.height = 'auto';
        field.style.height = `${field.scrollHeight + 2}px`;
    };
    let draftTimer;
    const prepare = () => {
        const field = document.getElementById('draft-prepare');
        field.value = document.getElementById('message').value;
        field.dispatchEvent(new Event('input', { bubbles: true }));
    };
    document.addEventListener('input', e => {
        if (e.target.id !== 'message') return;
        window.resizeComposer(); clearTimeout(draftTimer);
        if (!e.target.value.trim()) prepare();
        else if (!e.isComposing) draftTimer = setTimeout(prepare, 650);
    });
    document.addEventListener('compositionstart', () => clearTimeout(draftTimer));
    document.addEventListener('compositionend', () => { clearTimeout(draftTimer); draftTimer = setTimeout(prepare, 650); });
    window.addEventListener('resize', window.resizeComposer);
    let pressed, lastTap, suppressClick = false;
    const clearTap = () => { pressed = null; lastTap = null; };
    document.addEventListener('pointerdown', e => {
        pressed = null;
        suppressClick = false;
        // Controls keep their normal single-tap actions; text and empty space switch mode.
        if (e.button !== 0 || !e.isPrimary || !e.target.closest('main') || document.getElementById('conversation').hidden || e.target.closest('button, a, select, input, .spacing-handle, #settings, #history-view')) {
            lastTap = null;
            return;
        }
        pressed = { x: e.clientX, y: e.clientY, pointer: e.pointerId, at: performance.now() };
    });
    document.addEventListener('pointermove', e => {
        if (pressed && (Math.hypot(e.clientX - pressed.x, e.clientY - pressed.y) > 12 || e.pointerId !== pressed.pointer)) clearTap();
    });
    document.addEventListener('pointerup', e => {
        if (!pressed || e.pointerId !== pressed.pointer) return;
        const tap = pressed;
        pressed = null;
        const now = performance.now();
        if (now - tap.at > 300 || Math.hypot(e.clientX - tap.x, e.clientY - tap.y) > 12) { lastTap = null; return; }
        if (lastTap && now - lastTap.at <= 350 && Math.hypot(tap.x - lastTap.x, tap.y - lastTap.y) <= 24) {
            lastTap = null;
            document.getElementById('mode-toggle').click();
            suppressClick = true;
            e.preventDefault();
        } else lastTap = { ...tap, at: now };
    });
    document.addEventListener('pointercancel', () => { clearTap(); suppressClick = false; });
    document.addEventListener('scroll', clearTap, true);
    document.addEventListener('click', e => {
        if (suppressClick) { suppressClick = false; e.preventDefault(); e.stopImmediatePropagation(); }
    }, true);
    document.addEventListener('keydown', e => {
        if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === 'm') {
            e.preventDefault(); document.getElementById('mode-toggle').click();
        }
    });
    try {
        const saved = localStorage.getItem('speak-preferences');
        if (saved) {
            const field = document.getElementById('preferences'); field.value = saved;
            field.dispatchEvent(new Event('input', { bubbles: true }));
        }
    } catch (_) {}
    window.preferencesLoaded = true;
    requestAnimationFrame(window.resizeComposer);
    let drag;
    const update = (entry, gap) => {
        gap = Math.max(0, Math.min(100000, gap));
        entry.style.paddingTop = `${gap}px`;
        entry.dataset.gap = gap;
    };
    const save = entry => {
        const field = document.getElementById('write-gap-change');
        field.value = `${entry.dataset.id}:${entry.dataset.gap}`;
        field.dispatchEvent(new Event('input', { bubbles: true }));
    };
    document.addEventListener('pointerdown', e => {
        const handle = e.target.closest('.spacing-handle');
        if (!handle) return;
        e.preventDefault();
        handle.setPointerCapture(e.pointerId);
        const entry = handle.closest('.write-entry');
        drag = { entry, handle, pointer: e.pointerId, y: e.clientY + scrollY, gap: Number(entry.dataset.gap) };
    });
    document.addEventListener('pointermove', e => {
        if (!drag || e.pointerId !== drag.pointer) return;
        if (e.clientY > innerHeight - 60) window.scrollBy(0, 12);
        else if (e.clientY < 60) window.scrollBy(0, -12);
        update(drag.entry, drag.gap + e.clientY + scrollY - drag.y);
    });
    document.addEventListener('pointerup', e => {
        if (!drag || e.pointerId !== drag.pointer) return;
        save(drag.entry); drag = null;
    });
    document.addEventListener('pointercancel', () => {
        if (drag) update(drag.entry, drag.gap);
        drag = null;
    });
    document.addEventListener('keydown', e => {
        if (!e.target.matches('.spacing-handle') || !['ArrowUp', 'ArrowDown'].includes(e.key)) return;
        e.preventDefault();
        const entry = e.target.closest('.write-entry');
        update(entry, Number(entry.dataset.gap) + (e.key === 'ArrowDown' ? 40 : -40));
        save(entry);
    });
}
