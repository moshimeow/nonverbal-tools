use dioxus::prelude::*;
use speak_pocket::{Voice, worker::Speaker};
use std::{cell::RefCell, rc::Rc};

fn main() {
    dioxus::launch(App);
}

fn submit(
    speaker: &Speaker,
    text: String,
    voice: Voice,
    speed: f32,
    mut id: Signal<u64>,
    mut status: Signal<String>,
    mut last: Signal<String>,
) {
    match speaker.speak(text.clone(), voice, speed) {
        Ok(next) => {
            id.set(next);
            last.set(text);
            status.set("Making speech…".into());
        }
        Err(e) => status.set(e.to_string()),
    }
}

#[component]
fn App() -> Element {
    let mut text = use_signal(String::new);
    let mut voice = use_signal(|| Voice::Emma);
    let mut speed = use_signal(|| 1.0f32);
    let mut status = use_signal(|| "Ready when you are".to_string());
    let mut id = use_signal(|| 0u64);
    let last = use_signal(String::new);
    let (speaker, receiver) = use_hook(|| {
        let (speaker, receiver) = Speaker::start();
        (speaker, Rc::new(RefCell::new(Some(receiver))))
    });
    use_future(move || {
        let mut receiver = receiver
            .borrow_mut()
            .take()
            .expect("event receiver initialized once");
        async move {
            while let Some(event) = receiver.recv().await {
                if event.id == *id.peek() {
                    status.set(event.message);
                }
            }
        }
    });
    let speak = speaker.clone();
    let repeat = speaker.clone();
    let stop = speaker.clone();
    let chinese = voice() == Voice::Xiaoxiao;
    let phrases = if chinese {
        [
            "请稍等，我正在打字。",
            "谢谢！",
            "请再说一遍。",
            "我暂时不能说话。",
        ]
    } else {
        [
            "One moment, I'm typing.",
            "Thank you!",
            "Could you say that again?",
            "I'm using this to speak for now.",
        ]
    };
    rsx! {
        document::Title { "Speak Pocket" }
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        style { {include_str!("style.css")} }
        main {
            header {
                p { class: "eyebrow", "YOUR WORDS, OUT LOUD" }
                h1 { "Speak Pocket" }
                p { class: "subtitle", "A little voice, wherever you go." }
            }
            div { class: "voices", role: "group", aria_label: "Voice",
                button { class: if !chinese { "voice selected" } else { "voice" }, aria_pressed: (!chinese).to_string(), onclick: move |_| voice.set(Voice::Emma),
                    strong { "Emma" } span { "British English" }
                }
                button { class: if chinese { "voice selected" } else { "voice" }, aria_pressed: chinese.to_string(), onclick: move |_| voice.set(Voice::Xiaoxiao),
                    strong { "晓晓" } span { "普通话 · Mandarin" }
                }
            }
            label { r#for: "message", class: "label", "What would you like to say?" }
            textarea {
                id: "message", rows: 5, maxlength: 2000, value: "{text}",
                placeholder: if chinese { "在这里输入你想说的话……" } else { "Type a thought…" },
                oninput: move |event| text.set(event.value()),
            }
            div { class: "actions",
                button { class: "primary", disabled: text().trim().is_empty(), onclick: move |_| submit(&speak, text(), voice(), speed(), id, status, last), "Speak" }
                button { class: "secondary", onclick: move |_| { id.set(stop.stop()); status.set("Stopped".into()); }, "Stop" }
                button { class: "secondary", disabled: last().is_empty(), onclick: move |_| submit(&repeat, last(), voice(), speed(), id, status, last), "Repeat" }
            }
            p { class: "status", role: "status", aria_live: "polite", "{status}" }
            div { class: "speed",
                label { r#for: "speed", "Speed" }
                input { id: "speed", r#type: "range", min: "0.5", max: "2", step: "0.1", value: "{speed}",
                    oninput: move |event| { if let Ok(value) = event.value().parse::<f32>() { speed.set(value.clamp(0.5, 2.0)); } }
                }
                span { "{speed:.1}×" }
            }
            h2 { "A few useful words" }
            div { class: "phrases",
                for phrase in phrases {
                    { let speaker = speaker.clone(); rsx! {
                        button { class: "phrase", onclick: move |_| { text.set(phrase.into()); submit(&speaker, phrase.into(), voice(), speed(), id, status, last); }, "{phrase}" }
                    } }
                }
            }
            footer { "On your device. In your own time." }
        }
    }
}
