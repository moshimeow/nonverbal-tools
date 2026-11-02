use dioxus::prelude::*;
use speak_pocket::{
    Voice,
    worker::{Snapshot, Speaker},
};
use std::{cell::RefCell, rc::Rc};

fn main() {
    if std::env::args().any(|a| a == "--speech-check") {
        std::thread::spawn(|| {
            let result = (|| -> anyhow::Result<()> {
                let engine = speak_pocket::SpeechEngine::load(&speak_pocket::model_dir())?;
                for text in [
                    "woof!",
                    "woof",
                    "Hello!",
                    "Hello world.",
                    "Henlo! This blorptastic floof wants to zorbulate.",
                    "zorbulate",
                    "floof",
                    "Hello! One moment please, I'm typing my reply.",
                ] {
                    let result = engine.generate(
                        text,
                        Voice::Emma,
                        1.0,
                        std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
                        1,
                    );
                    match result {
                        Ok(audio) => println!(
                            "DEVICE CHECK {text:?}: {} samples, {} Hz",
                            audio.samples.len(),
                            audio.sample_rate
                        ),
                        Err(e) => println!("DEVICE CHECK {text:?}: ERROR {e:#}"),
                    }
                }
                Ok(())
            })();
            println!("DEVICE CHECK COMPLETE: {result:?}");
        });
    }
    dioxus::launch(App);
}

fn send_draft(speaker: Speaker, voice: Voice, speed: f32, mut error: Signal<String>, write: bool) {
    spawn(async move {
        let mut script = document::eval(
            r#"
            const field = document.getElementById('message');
            if (field.dataset.sending === 'true') { dioxus.send(''); return; }
            field.dataset.sending = 'true';
            const text = field.value;
            if (text.trim()) {
                field.value = '';
                field.dispatchEvent(new Event('input', { bubbles: true }));
                field.focus(); window.scrollTo(0, 0);
            }
            dioxus.send(text);
            const accepted = await dioxus.recv();
            if (!accepted) {
                field.value = text + field.value;
                field.dispatchEvent(new Event('input', { bubbles: true }));
            }
            delete field.dataset.sending;
        "#,
        );
        match script.recv::<String>().await {
            Ok(text) => {
                let result = if text.trim().is_empty() {
                    Ok(())
                } else {
                    speaker.send(text, voice, speed, write).await
                };
                let _ = script.send(result.is_ok());
                error.set(result.err().map(|e| e.to_string()).unwrap_or_default());
            }
            Err(e) => error.set(format!("Couldn't send text: {e}")),
        }
    });
}

#[component]
fn ViewIcon(kind: u8) -> Element {
    rsx! {
        svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true", "focusable": "false",
            if kind == 0 {
                path { d: "M11 4 5 9H2v6h3l6 5V4Z" }
                path { d: "M15 8a6 6 0 0 1 0 8M18 5a10 10 0 0 1 0 14" }
            } else if kind == 1 {
                path { d: "m15 4 5 5M4 15 15 4a3.5 3.5 0 0 1 5 5L9 20l-6 1 1-6ZM4 15l5 5" }
            } else {
                circle { cx: "12", cy: "12", r: "9" }
                path { d: "M12 7v5l3 2" }
            }
        }
    }
}

#[component]
fn App() -> Element {
    let mut text = use_signal(String::new);
    let mut voice = use_signal(|| Voice::Emma);
    let mut speed = use_signal(|| 1.0f32);
    let mut menu = use_signal(|| false);
    let mut history_open = use_signal(|| false);
    let mut write_mode = use_signal(|| false);
    let mut speak_size = use_signal(|| 20u32);
    let mut write_size = use_signal(|| 36u32);
    let font_size = if write_mode() {
        write_size()
    } else {
        speak_size()
    };
    use_effect(move || {
        document::eval(&format!(
            r#"
            try {{ if (window.preferencesLoaded) localStorage.setItem('speak-preferences', '{}:{}:{}'); }} catch (_) {{}}
            if (window.resizeComposer) requestAnimationFrame(window.resizeComposer);
        "#,
            write_mode(),
            speak_size(),
            write_size()
        ));
    });
    use_effect(|| {
        document::eval(include_str!("interaction.js"));
    });
    let mut history_limit = use_signal(|| 100usize);
    let mut copied = use_signal(String::new);
    let error = use_signal(String::new);
    let mut state = use_signal(Snapshot::default);
    let (speaker, receiver) = use_hook(|| {
        let (speaker, receiver) = Speaker::start();
        (speaker, Rc::new(RefCell::new(Some(receiver))))
    });
    use_future(move || {
        let mut receiver = receiver.borrow_mut().take().expect("one event receiver");
        async move {
            while let Some(snapshot) = receiver.recv().await {
                state.set(snapshot);
            }
        }
    });
    let refresh_draft = speaker.clone();
    use_effect(move || {
        let writing = write_mode();
        let selected_voice = voice();
        let selected_speed = speed();
        let draft_text = text.peek();
        refresh_draft.prepare(
            if writing { "" } else { &draft_text },
            selected_voice,
            selected_speed,
        );
    });
    let send = speaker.clone();
    let prepare = speaker.clone();
    let spacing = speaker.clone();
    let chinese = voice() == Voice::Xiaoxiao;
    rsx! {
        document::Title { "Speak Pocket" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        style { {include_str!("style.css")} }
        main {
            nav {
                button { class: "menu-toggle", aria_label: "Menu", aria_expanded: menu().to_string(), aria_controls: "settings",
                    onclick: move |_| menu.toggle(), if menu() { "×" } else { "☰" } }
                if history_open() {
                    button { class: "copy", aria_label: "Back to conversation", onclick: move |_| history_open.set(false), "←" }
                }
                div { class: "write-toolbar",
                    button { class: "copy", aria_label: "Smaller text", disabled: font_size <= 16,
                        onclick: move |_| { if write_mode() { write_size.set((write_size() - 4).max(16)); } else { speak_size.set((speak_size() - 4).max(16)); } }, "−" }
                    button { class: "copy", aria_label: "Larger text", disabled: font_size >= 96,
                        onclick: move |_| { if write_mode() { write_size.set((write_size() + 4).min(96)); } else { speak_size.set((speak_size() + 4).min(96)); } }, "+" }
                }

            }
            if menu() {
                section { id: "settings", class: "settings", aria_label: "Speech settings",
                    button { class: "history-menu", aria_label: "History", title: "History", aria_controls: "history-view",
                        onclick: move |_| { history_open.set(true); menu.set(false); copied.set(String::new()); }, ViewIcon { kind: 2 } }
                    button { class: "copy", onclick: move |_| { write_mode.toggle(); menu.set(false); },
                        if write_mode() { "Switch to speaking" } else { "Switch to writing" }
                    }
                    small { "Hold anywhere in the conversation to switch. Control–Shift–M also works." }
                    label { r#for: "language", "Language" }
                    select { id: "language", value: if chinese { "zh" } else { "en" },
                        onchange: move |e| voice.set(if e.value() == "zh" { Voice::Xiaoxiao } else { Voice::Emma }),
                        option { value: "en", "English (British)" }
                        option { value: "zh", "普通话 · Mandarin" }
                    }
                    label { r#for: "voice", "Voice" }
                    select { id: "voice", value: if chinese { "xiaoxiao" } else { "emma" },
                        if chinese { option { value: "xiaoxiao", "晓晓 · Xiaoxiao" } }
                        else { option { value: "emma", "Emma" } }
                    }
                    div { class: "speed",
                        label { r#for: "speed", "Speed" }
                        input { id: "speed", r#type: "range", min: "0.5", max: "2", step: "0.1", value: "{speed}",
                            oninput: move |e| { if let Ok(value) = e.value().parse::<f32>() { speed.set(value.clamp(0.5, 2.0)); } } }
                        span { "{speed:.1}×" }
                    }
                }
            }
            if let Some(message) = state().audio_error { p { class: "error", role: "alert", "{message}" } }
            if let Some(message) = state().history_error { p { class: "error", role: "alert", "{message}" } }
            if history_open() {
                section { id: "history-view", aria_label: "History", class: "history",
                    div { class: "history-toolbar",
                        span { "{state().history.len()} sent messages" }
                        button { class: "copy", disabled: state().history.is_empty(), onclick: move |_| {
                            let all = state.peek().history.iter().map(|item| item.text.as_str()).collect::<Vec<_>>().join("\n\n");
                            copied.set(match speak_pocket::platform::copy_text(&all) { Ok(()) => "Copied all messages".into(), Err(e) => e.to_string() });
                        }, "Copy all" }
                    }
                    p { class: "copy-status", role: "status", "{copied}" }
                    if state().history.is_empty() { p { class: "history-empty", "Messages you send will appear here, even after restarting the app." } }
                    for item in state().history.iter().rev().take(history_limit()).cloned() {
                        article { key: "history-{item.id}", class: "history-item",
                            small { class: "history-meta", "{speak_pocket::platform::format_timestamp(item.sent_at)} · {item.kind.label()}" }
                            small { class: "history-meta", {match &item.location {
                                Some(l) => format!("{:.5}, {:.5} · ±{:.0} m", l.latitude, l.longitude, l.accuracy),
                                None if item.location_pending => "Locating…".into(),
                                None => "Location not recorded".into(),
                            }} }
                            p { "{item.text}" }
                            if let Some(status) = &item.playback { small { class: "history-meta", "{status}" } }
                            button { class: "copy", aria_label: "Copy message {item.id}", onclick: move |_| {
                                copied.set(match speak_pocket::platform::copy_text(&item.text) { Ok(()) => "Copied message".into(), Err(e) => e.to_string() });
                            }, "Copy" }
                        }
                    }
                    if state().history.len() > history_limit() { button { class: "copy", onclick: move |_| history_limit += 100, "Show older messages" } }
                }
            }
            button { id: "mode-toggle", hidden: true, onclick: move |_| write_mode.toggle() }
            input { id: "preferences", r#type: "hidden", oninput: move |e| {
                let value = e.value(); let parts: Vec<_> = value.split(':').collect();
                if parts.len() == 3 {
                    write_mode.set(parts[0] == "true");
                    if let Ok(n) = parts[1].parse::<u32>() { speak_size.set(n.clamp(16, 96)); }
                    if let Ok(n) = parts[2].parse::<u32>() { write_size.set(n.clamp(16, 96)); }
                }
            } }
            section { id: "conversation", aria_label: "Conversation", hidden: history_open(),
                "data-mode": if write_mode() { "write" } else { "speak" }, style: "font-size: {font_size}px",
                textarea { id: "message", aria_label: "Message", rows: 2, initial_value: "", placeholder: "Type here…",
                    oninput: move |e| text.set(e.value()) }
                button { id: "send", class: "send", aria_label: if write_mode() { "Send message" } else { "Speak message" },
                    title: if write_mode() { "Send message · hold to switch mode" } else { "Speak message · hold to switch mode" },
                    disabled: text().trim().is_empty(), onclick: move |_| send_draft(send.clone(), voice(), speed(), error, write_mode()),
                    if write_mode() {
                        svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true", path { d: "M12 20V4m-7 7 7-7 7 7" } }
                    } else { ViewIcon { kind: 0 } }
                }
                if !error().is_empty() { p { role: "alert", class: "error", "{error}" } }
                input { id: "draft-prepare", r#type: "hidden", oninput: move |e| {
                    let value = e.value();
                    prepare.prepare(if write_mode() { "" } else { &value }, voice(), speed());
                } }
                input { id: "write-gap-change", r#type: "hidden", oninput: move |e| {
                    if let Some((id, gap)) = e.value().split_once(':')
                        && let (Ok(id), Ok(gap)) = (id.parse::<i64>(), gap.parse::<f64>()) { spacing.set_gap(id, gap); }
                } }
                for item in state().history.iter().rev() {
                    { let replay = speaker.clone(); let stop = speaker.clone();
                      let id = item.id;
                      let active = state().queue.iter().find(|q| q.history_id == Some(id)).cloned();
                      rsx! {
                        article { key: "message-{id}", class: "write-entry", "data-id": "{id}", "data-gap": "{item.gap}", style: "padding-top: {item.gap}px",
                            button { class: "spacing-handle", aria_label: "Adjust space above message {id}", title: "Drag to add space above this message", "━" }
                            p { "{item.text}" }
                            div { class: "message-actions",
                                if let Some(active) = active {
                                    button { class: "message-speak", aria_label: "Stop message {id}", onclick: move |_| stop.remove(active.id), "■" }
                                    small { role: "status", "{active.phase.label()}" }
                                } else {
                                    button { class: "message-speak", aria_label: "Speak message {id}", title: "Speak", onclick: move |_| replay.replay(id, voice(), speed()), ViewIcon { kind: 0 } }
                                }
                            }
                            if let Some(status) = &item.playback {
                                if status.starts_with("Not ") { p { class: "error", "{status}" } }
                            }
                        }
                      }
                    }
                }
            }
        }
    }
}
