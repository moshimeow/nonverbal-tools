use dioxus::prelude::*;
use speak_pocket::{
    Voice,
    worker::{Item, Phase, Snapshot, Speaker},
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
    // Keep the JS channel alive until persistence/enqueue acknowledges the send.
    // This also preserves the text in the WebView if writing the history fails.
    spawn(async move {
        let id = if write { "write-message" } else { "message" };
        let mut script = document::eval(&format!(
            r#"
            const field = document.getElementById('{id}');
            const text = field.value;
            if (text.trim()) {{
                field.value = '';
                field.dispatchEvent(new Event('input', {{ bubbles: true }}));
                field.focus();
                if (field.id === 'write-message') window.scrollTo(0, 0);
            }}
            dioxus.send(text);
            const accepted = await dioxus.recv();
            if (!accepted) {{
                field.value = text + field.value;
                field.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }}
        "#
        ));
        match script.recv::<String>().await {
            Ok(text) => {
                let result = if text.trim().is_empty() {
                    Ok(())
                } else if write {
                    speaker.write(text).await
                } else {
                    speaker.speak(text, voice, speed).map(|_| ())
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
fn QueueRow(item: Item, onremove: EventHandler<u64>) -> Element {
    let mut start = use_signal(|| None::<(f64, f64)>);
    let mut offset = use_signal(|| 0.0f64);
    let id = item.id;
    let playing = item.phase == Phase::Playing;
    let failed = matches!(item.phase, Phase::Failed(_));
    rsx! {
        div { class: "swipe-row",
            div { class: "swipe-under", aria_hidden: "true", if playing { "Stop" } else { "Remove" } }
            article {
                class: if playing { "queue-card playing" } else if failed { "queue-card failed" } else { "queue-card" },
                style: "transform: translateX({offset}px)",
                onpointerdown: move |e| { let p = e.client_coordinates(); start.set(Some((p.x, p.y))); },
                onpointermove: move |e| {
                    if let Some((x, y)) = start() {
                        let p = e.client_coordinates();
                        if (p.x - x).abs() > (p.y - y).abs() { offset.set((p.x - x).clamp(-120.0, 0.0)); }
                    }
                },
                onpointerup: move |e| {
                    if let Some((x, y)) = start() {
                        let p = e.client_coordinates();
                        if x - p.x > 65.0 && (p.y - y).abs() < (x - p.x) / 2.0 { onremove.call(id); }
                    }
                    start.set(None); offset.set(0.0);
                },
                onpointercancel: move |_| { start.set(None); offset.set(0.0); },
                div { class: "item-copy",
                    p { class: "item-text", "{item.text}" }
                    small { class: "item-status", "{item.phase.label()} · {item.voice.name()}" }
                }
                button { class: "remove", aria_label: if playing { "Stop this message" } else { "Remove this message" },
                    onclick: move |_| onremove.call(id), "×" }
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
    let mut tab = use_signal(|| 0u8);
    let mut write_text = use_signal(String::new);
    let mut font_size = use_signal(|| 36u32);
    use_effect(|| {
        document::eval(include_str!("interaction.js"));
    });
    let mut history_limit = use_signal(|| 100usize);
    let mut copied = use_signal(String::new);
    let mut error = use_signal(String::new);
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
    let send = speaker.clone();
    let write_send = speaker.clone();
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
                div { class: "tabs", role: "tablist", aria_label: "View",
                    button { role: "tab", id: "speak-tab", aria_label: "Speak", title: "Speak", aria_controls: "speech-view", aria_selected: (tab() == 0).to_string(),
                        class: if tab() == 0 { "tab active" } else { "tab" }, onclick: move |_| { tab.set(0); menu.set(false); }, ViewIcon { kind: 0 } }
                    button { role: "tab", id: "write-tab", aria_label: "Write", title: "Write", aria_controls: "write-view", aria_selected: (tab() == 1).to_string(),
                        class: if tab() == 1 { "tab active" } else { "tab" }, onclick: move |_| { tab.set(1); menu.set(false); }, ViewIcon { kind: 1 } }

                }
                if tab() == 1 {
                div { class: "write-toolbar",
                    button { class: "copy", aria_label: "Smaller text", disabled: font_size() <= 20, onclick: move |_| font_size.set((font_size() - 4).max(20)), "−" }
                    button { class: "copy", aria_label: "Larger text", disabled: font_size() >= 96, onclick: move |_| font_size.set((font_size() + 4).min(96)), "+" }
                }
                }
            }
            if menu() {
                section { id: "settings", class: "settings", aria_label: "Speech settings",
                    button { class: "history-menu", aria_label: "History", title: "History", aria_controls: "history-view",
                        onclick: move |_| { tab.set(2); menu.set(false); copied.set(String::new()); }, ViewIcon { kind: 2 } }
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
            if let Some(message) = state().history_error { p { class: "error", role: "alert", "{message}" } }
            if tab() == 2 {
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
                            button { class: "copy", aria_label: "Copy message {item.id}", onclick: move |_| {
                                copied.set(match speak_pocket::platform::copy_text(&item.text) { Ok(()) => "Copied message".into(), Err(e) => e.to_string() });
                            }, "Copy" }
                        }
                    }
                    if state().history.len() > history_limit() { button { class: "copy", onclick: move |_| history_limit += 100, "Show older messages" } }
                }
            }
            section { id: "write-view", role: "tabpanel", aria_labelledby: "write-tab", hidden: tab() != 1,
                textarea { id: "write-message", aria_label: "Write message", rows: 3, maxlength: 2000, initial_value: "", placeholder: "Type here…",
                    style: "font-size: {font_size}px", oninput: move |e| write_text.set(e.value()) }
                button { id: "write-send", class: "send write-send", aria_label: "Send message", title: "Send message", disabled: write_text().trim().is_empty(), onclick: move |_| send_draft(write_send.clone(), voice(), speed(), error, true),
                    svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true", "focusable": "false", path { d: "M12 20V4m-7 7 7-7 7 7" } }
                }
                if !error().is_empty() { p { role: "alert", class: "error", "{error}" } }
                input { id: "write-gap-change", r#type: "hidden", oninput: move |e| {
                    if let Some((id, gap)) = e.value().split_once(':')
                        && let (Ok(id), Ok(gap)) = (id.parse::<i64>(), gap.parse::<f64>()) { spacing.set_gap(id, gap); }
                } }
                for item in state().history.iter().rev().filter(|i| i.kind == speak_pocket::history::MessageKind::Write) {
                    article { key: "write-{item.id}", class: "write-entry", "data-id": "{item.id}", "data-gap": "{item.gap}", style: "padding-top: {item.gap}px; font-size: {font_size}px",
                        button { class: "spacing-handle", aria_label: "Adjust space above message {item.id}", title: "Drag to add space above this message", "━" }
                        p { "{item.text}" }
                    }
                }
            }
            div { id: "speech-view", class: "speech-view", role: "tabpanel", aria_labelledby: "speak-tab", hidden: tab() != 0,
            div { class: "stream",
                if !state().recent.is_empty() {
                    section { class: "recent", aria_label: "Recently spoken",
                        for item in state().recent {
                            { let replay = speaker.clone(); rsx! {
                                button { key: "recent-{item.id}", class: "recent-item", aria_label: "Say again: {item.text}",
                                    onclick: move |_| { if let Err(e) = replay.speak(item.text.clone(), item.voice, item.speed) { error.set(e.to_string()); } },
                                    span { "{item.text}" } small { "↻ Say again" }
                                }
                            } }
                        }
                    }
                }
                section { class: "queue", aria_label: "Speech queue",
                    for item in state().queue {
                        { let remove = speaker.clone(); rsx! { QueueRow { key: "{item.id}", item, onremove: move |id| remove.remove(id) } } }
                    }
                }
            }
            div { class: "composer",
                label { class: "sr-only", r#for: "message", "Message" }
                textarea { id: "message", rows: 3, maxlength: 2000, initial_value: "",
                    placeholder: if chinese { "输入你想说的话……" } else { "Type something…" },
                    oninput: move |e| text.set(e.value()),
                }
                button { id: "speak-send", class: "send", disabled: text().trim().is_empty(), onclick: move |_| send_draft(send.clone(), voice(), speed(), error, false), "▶ Play" }
                if !error().is_empty() { p { role: "alert", class: "error", "{error}" } }
            }
            }
        }
    }
}
