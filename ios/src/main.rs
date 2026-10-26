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

fn send_draft(speaker: Speaker, voice: Voice, speed: f32, mut error: Signal<String>) {
    // Read and clear in one WebView operation, preserving any subsequent typing.
    spawn(async move {
        let result = document::eval(
            r#"
            const field = document.getElementById('message');
            const text = field.value;
            if (text.trim()) {
                field.value = '';
                field.dispatchEvent(new Event('input', { bubbles: true }));
                field.focus();
            }
            return text;
        "#,
        )
        .join::<String>()
        .await;
        match result {
            Ok(text) if !text.trim().is_empty() => {
                if let Err(e) = speaker.speak(text.clone(), voice, speed) {
                    error.set(e.to_string());
                    let restore = document::eval(
                        "const f = document.getElementById('message'); f.value = (await dioxus.recv()) + f.value; f.dispatchEvent(new Event('input', {bubbles:true}));",
                    );
                    let _ = restore.send(text);
                    let _ = restore.await;
                } else {
                    error.set(String::new());
                }
            }
            Ok(_) => {}
            Err(e) => error.set(format!("Couldn't send text: {e}")),
        }
    });
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
    let mut history_tab = use_signal(|| false);
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
    let chinese = voice() == Voice::Xiaoxiao;
    rsx! {
        document::Title { "Speak Pocket" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        style { {include_str!("style.css")} }
        main {
            nav {
                button { class: "menu-toggle", aria_label: "Language and voice settings", aria_expanded: menu().to_string(), aria_controls: "settings",
                    onclick: move |_| menu.toggle(), if menu() { "×" } else { "☰" } }
                div { class: "tabs", role: "tablist", aria_label: "View",
                    button { role: "tab", id: "speak-tab", aria_controls: "speech-view", aria_selected: (!history_tab()).to_string(),
                        class: if history_tab() { "tab" } else { "tab active" }, onclick: move |_| history_tab.set(false), "Speak" }
                    button { role: "tab", id: "history-tab", aria_controls: "history-view", aria_selected: history_tab().to_string(),
                        class: if history_tab() { "tab active" } else { "tab" }, onclick: move |_| { history_tab.set(true); copied.set(String::new()); }, "History" }
                }
            }
            if menu() {
                section { id: "settings", class: "settings", aria_label: "Speech settings",
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
            if history_tab() {
                section { id: "history-view", role: "tabpanel", aria_labelledby: "history-tab", class: "history",
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
                            p { "{item.text}" }
                            button { class: "copy", aria_label: "Copy message {item.id}", onclick: move |_| {
                                copied.set(match speak_pocket::platform::copy_text(&item.text) { Ok(()) => "Copied message".into(), Err(e) => e.to_string() });
                            }, "Copy" }
                        }
                    }
                    if state().history.len() > history_limit() { button { class: "copy", onclick: move |_| history_limit += 100, "Show older messages" } }
                }
            }
            div { id: "speech-view", class: "speech-view", role: "tabpanel", aria_labelledby: "speak-tab", hidden: history_tab(),
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
                button { class: "send", disabled: text().trim().is_empty(), onclick: move |_| send_draft(send.clone(), voice(), speed(), error), "▶ Play" }
                if !error().is_empty() { p { role: "alert", class: "error", "{error}" } }
            }
            }
        }
    }
}
