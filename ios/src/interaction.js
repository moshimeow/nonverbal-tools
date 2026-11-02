if (!window.speakInteractions) {
    window.speakInteractions = true;
    let compositionEnded = 0;
    document.addEventListener('compositionend', () => compositionEnded = performance.now());
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
    let held, holdTimer, suppressClick = false;
    const clearHold = () => { clearTimeout(holdTimer); held = null; };
    document.addEventListener('pointerdown', e => {
        clearHold();
        if (e.button !== 0 || !e.isPrimary || !e.target.closest('main') || document.getElementById('conversation').hidden || e.target.closest('.spacing-handle, #settings, #history-view')) return;
        suppressClick = false;
        held = { x: e.clientX, y: e.clientY, pointer: e.pointerId };
        holdTimer = setTimeout(() => {
            if (!held) return;
            document.getElementById('mode-toggle').click();
            suppressClick = true;
            clearHold();
        }, 550);
    });
    document.addEventListener('pointermove', e => {
        if (held && (Math.hypot(e.clientX - held.x, e.clientY - held.y) > 12 || e.pointerId !== held.pointer)) clearHold();
    });
    document.addEventListener('pointerup', clearHold);
    document.addEventListener('pointercancel', () => { clearHold(); suppressClick = false; });
    document.addEventListener('scroll', clearHold, true);
    document.addEventListener('click', e => {
        if (suppressClick) { suppressClick = false; e.preventDefault(); e.stopImmediatePropagation(); }
    }, true);
    document.addEventListener('contextmenu', e => {
        if (e.target.closest('#conversation')) e.preventDefault();
    });
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
