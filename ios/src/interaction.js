if (!window.speakInteractions) {
    window.speakInteractions = true;
    let compositionEnded = 0;
    document.addEventListener('compositionend', () => compositionEnded = performance.now());
    document.addEventListener('keydown', e => {
        const id = e.target.id;
        if ((id !== 'message' && id !== 'write-message') || e.key !== 'Enter') return;
        if (e.isComposing || e.keyCode === 229) return;
        if (performance.now() - compositionEnded < 50) { e.preventDefault(); return; }
        e.preventDefault();
        if (e.repeat || !e.target.value.trim()) return;
        const send = document.getElementById(id === 'message' ? 'speak-send' : 'write-send');
        send.disabled = false;
        send.click();
    }, true);
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
