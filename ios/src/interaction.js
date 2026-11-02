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
    // Mirror chunks::split for immediate boundary detection and exact cache highlighting.
    // Offsets remain UTF-16 DOM offsets even for supplementary Unicode characters.
    window.draftChunks = (text, chinese) => {
        const chunks = [], limit = chinese ? 60 : 160;
        let start = 0;
        while (start < text.length) {
            while (/\s/u.test(text[start] || '') && start < text.length) start++;
            if (start >= text.length) break;
            const rest = text.slice(start).trimEnd(), chars = Array.from(rest);
            let end = rest.length, offset = 0, space = null, complete = false;
            for (let n = 0; n < Math.min(limit, chars.length); n++) {
                const c = chars[n], next = chars[n + 1];
                if (/\s/u.test(c)) space = offset;
                const sentence = /[?!。？！]/u.test(c) || (c === '.' && (next === undefined || /\s/u.test(next)) && n >= 12 && !rest.slice(0, offset).endsWith(' Mr') && !rest.slice(0, offset).endsWith(' Dr'));
                const clause = n >= limit / 2 && /[,;:，；：]/u.test(c);
                if (sentence || clause) { end = offset + c.length; complete = true; break; }
                if (n + 1 >= limit) { end = space > 0 ? space : offset + c.length; complete = true; break; }
                offset += c.length;
            }
            const phrase = rest.slice(0, end).trimEnd();
            if (/[\p{L}\p{N}]/u.test(phrase)) chunks.push({start, end: start + phrase.length, text: phrase, complete});
            start += end;
        }
        return chunks;
    };
    window.paintPrepared = () => {
        const overlay = document.getElementById('prepared-overlay');
        if (!overlay) return;
        const field = document.getElementById('message');
        const composer = document.getElementById('composer');
        overlay.replaceChildren();
        if (!composer.classList.contains('show-chunks')) return;
        const cached = new Set(Array.from(document.getElementById('prepared-cache').children, node => node.textContent));
        let cursor = 0;
        for (const chunk of window.draftChunks(field.value, composer.dataset.chinese === 'true')) {
            if (!cached.has(chunk.text)) continue;
            overlay.append(document.createTextNode(field.value.slice(cursor, chunk.start)));
            const mark = document.createElement('mark');
            mark.textContent = chunk.text;
            overlay.append(mark);
            cursor = chunk.end;
        }
        overlay.append(document.createTextNode(field.value.slice(cursor) + '\n'));
    };
    let draftTimer, hasText = false, lastBoundary = '';
    const prepare = () => {
        const field = document.getElementById('draft-prepare');
        field.value = document.getElementById('message').value;
        field.dispatchEvent(new Event('input', { bubbles: true }));
    };
    window.prepareDraft = prepare;
    const schedulePreparation = () => {
        clearTimeout(draftTimer);
        const text = document.getElementById('message').value;
        const chinese = document.getElementById('composer')?.dataset.chinese === 'true';
        const first = window.draftChunks(text, chinese)[0];
        const boundary = first?.complete ? first.text : '';
        if (!text.trim() || (boundary && boundary !== lastBoundary)) prepare();
        else draftTimer = setTimeout(prepare, 500);
        lastBoundary = boundary;
    };
    document.addEventListener('input', e => {
        if (e.target.id !== 'message') return;
        // Keep keystrokes local: only empty/nonempty transitions update Rust UI state.
        e.stopImmediatePropagation();
        const nextHasText = !!e.target.value.trim();
        if (nextHasText !== hasText) {
            hasText = nextHasText;
            const state = document.getElementById('composer-state');
            state.value = String(hasText);
            state.dispatchEvent(new Event('input', { bubbles: true }));
        }
        window.resizeComposer(); window.paintPrepared(); clearTimeout(draftTimer);
        if (!e.isComposing) schedulePreparation();
    }, true);
    document.addEventListener('compositionstart', () => clearTimeout(draftTimer));
    document.addEventListener('compositionend', schedulePreparation);
    window.addEventListener('resize', window.resizeComposer);
    let pressed, lastTap, suppressClick = false;
    const clearTap = () => { pressed = null; lastTap = null; };
    document.addEventListener('pointerdown', e => {
        pressed = null;
        suppressClick = false;
        // Controls and editors keep native tap actions; transcript text and empty space switch mode.
        if (e.button !== 0 || !e.isPrimary || !e.target.closest('main') || document.getElementById('conversation').hidden || e.target.closest('button, a, select, input, textarea, [contenteditable], .spacing-handle, #settings, #history-view')) {
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
