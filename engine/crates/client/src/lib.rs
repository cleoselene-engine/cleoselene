mod render;
mod audio;
mod predictor;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, WebSocket, RtcPeerConnection, RtcDataChannel, RtcConfiguration, RtcIceServer, MessageEvent, BinaryType, RtcSdpType, RtcSessionDescriptionInit, RtcIceCandidateInit, RtcSessionDescription};
use std::rc::Rc;
use std::cell::RefCell;
use render::Renderer;
use audio::AudioManager;
use predictor::Predictor;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ruzstd::StreamingDecoder;
use std::io::Read;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum SignalMessage {
    WELCOME { session_id: String, server_instance_id: String },
    OFFER { sdp: String },
    ANSWER { sdp: String },
    CANDIDATE { candidate: String, sdp_mid: Option<String>, sdp_mline_index: Option<u16> },
}

struct ClientState {
    renderer: Renderer,
    audio: AudioManager,
    predictor: Option<Predictor>,
    ws: Option<WebSocket>,
    pc: Option<RtcPeerConnection>,
    dc: Option<RtcDataChannel>,
    session_id: Option<String>,
    game_started: bool,
    frame_count: u32,
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    console::log_1(&"Cleoselene Rust Client Starting... V3 (Debug)".into());

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    
    let canvas = document.get_element_by_id("gameCanvas")
        .expect("Canvas not found")
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    
    let dpr = window.device_pixel_ratio();
    canvas.set_width((800.0 * dpr) as u32);
    canvas.set_height((600.0 * dpr) as u32);
    let ctx = canvas.get_context("2d")?.unwrap().dyn_into::<web_sys::CanvasRenderingContext2d>()?;
    ctx.scale(dpr, dpr)?;

    let renderer = Renderer::new(canvas).map_err(|e| JsValue::from_str(&e))?;
    let audio = AudioManager::new().map_err(|e| JsValue::from_str(&e))?;

    let state = Rc::new(RefCell::new(ClientState {
        renderer,
        audio,
        predictor: None,
        ws: None,
        pc: None,
        dc: None,
        session_id: None,
        game_started: false,
        frame_count: 0,
    }));

    connect(state.clone())?;
    setup_input(state.clone())?;
    load_predictor(state.clone());

    Ok(())
}

fn connect(state: Rc<RefCell<ClientState>>) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let protocol = if location.protocol()? == "https:" { "wss:" } else { "ws:" };
    let host = location.host()?;
    let url = format!("{}//{}/ws", protocol, host);

    let ws = WebSocket::new(&url)?;
    ws.set_binary_type(BinaryType::Arraybuffer);

    let state_clone = state.clone();
    let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
        if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
            let txt: String = txt.into();
            handle_signal_message(state_clone.clone(), &txt);
        } else if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
            let data = js_sys::Uint8Array::new(&buf).to_vec();
            process_frame(state_clone.clone(), &data);
        }
    });
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    state.borrow_mut().ws = Some(ws);
    Ok(())
}

fn handle_signal_message(state: Rc<RefCell<ClientState>>, msg: &str) {
    let signal: SignalMessage = match serde_json::from_str(msg) {
        Ok(s) => s,
        Err(e) => { console::log_1(&format!("JSON Error: {}", e).into()); return; }
    };

    match signal {
        SignalMessage::WELCOME { session_id, .. } => {
            console::log_1(&format!("Joined Session: {}", session_id).into());
            
            // Hide overlay immediately on join
            {
                console::log_1(&"Attempting to hide overlay...".into());
                match state.try_borrow_mut() {
                    Ok(mut client) => {
                        client.session_id = Some(session_id);
                        if !client.game_started {
                            client.game_started = true;
                            if let Some(window) = web_sys::window() {
                                if let Some(document) = window.document() {
                                    if let Some(overlay) = document.get_element_by_id("loading-overlay") {
                                        let _ = overlay.class_list().add_1("hidden");
                                        console::log_1(&"Overlay hidden via classList".into());
                                    } else {
                                        console::warn_1(&"Overlay element not found".into());
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        console::error_1(&format!("CRITICAL: State Borrow Failed in WELCOME: {:?}", e).into());
                        // Try to force hide overlay anyway without state lock (unsafe logic but UI fix)
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(overlay) = document.get_element_by_id("loading-overlay") {
                                    let _ = overlay.class_list().add_1("hidden");
                                }
                            }
                        }
                    }
                }
            }
            
            setup_webrtc(state.clone());
        },
        SignalMessage::ANSWER { sdp } => {
            let state_rc = state.clone();
            let sdp_str = sdp.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let pc = state_rc.borrow().pc.clone();
                if let Some(pc) = pc {
                    let mut answer = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                    answer.set_sdp(&sdp_str);
                    let _ = JsFuture::from(pc.set_remote_description(&answer)).await;
                }
            });
        },
        SignalMessage::CANDIDATE { candidate, sdp_mid, sdp_mline_index } => {
            let pc = state.borrow().pc.clone();
            if let Some(pc) = pc {
                let mut init = RtcIceCandidateInit::new(&candidate);
                init.set_sdp_mid(sdp_mid.as_deref());
                if let Some(idx) = sdp_mline_index {
                    init.set_sdp_m_line_index(Some(idx));
                }
                let _ = pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init));
            }
        }
        _ => {}
    }
}

fn setup_webrtc(state: Rc<RefCell<ClientState>>) {
    let mut config = RtcConfiguration::new();
    let mut server = RtcIceServer::new();
    server.set_urls(&JsValue::from_str("stun:stun.l.google.com:19302"));
    config.set_ice_servers(&js_sys::Array::of1(&server));

    let pc = RtcPeerConnection::new_with_configuration(&config).unwrap();
    
    // Data Channel
    let dc = pc.create_data_channel("game_data");
    // dc.set_binary_type(BinaryType::Arraybuffer); // Commented out due to compilation error
    
    let state_clone = state.clone();
    let onmessage_dc = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
        if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
            let data = js_sys::Uint8Array::new(&buf).to_vec();
            process_frame(state_clone.clone(), &data);
        }
    });
    dc.set_onmessage(Some(onmessage_dc.as_ref().unchecked_ref()));
    onmessage_dc.forget();

    // ICE Candidates
    let state_ws = state.clone();
    let onicecandidate = Closure::<dyn FnMut(_)>::new(move |e: web_sys::RtcPeerConnectionIceEvent| {
        if let Some(candidate) = e.candidate() {
            let msg = json!({
                "type": "CANDIDATE",
                "candidate": candidate.candidate(),
                "sdp_mid": candidate.sdp_mid(),
                "sdp_mline_index": candidate.sdp_m_line_index()
            });
            if let Some(ws) = &state_ws.borrow().ws {
                let _ = ws.send_with_str(&msg.to_string());
            }
        }
    });
    pc.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
    onicecandidate.forget();

    state.borrow_mut().pc = Some(pc.clone());
    state.borrow_mut().dc = Some(dc);

    // Create Offer
    let state_offer = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let offer = JsFuture::from(pc.create_offer()).await.unwrap();
        // Convert JsValue to RtcSessionDescription (read-only interface)
        let offer_desc: RtcSessionDescription = offer.clone().unchecked_into();
        let sdp_str = offer_desc.sdp();
        
        // Use RtcSessionDescriptionInit for set_local_description
        let mut offer_init = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        offer_init.set_sdp(&sdp_str);
        
        JsFuture::from(pc.set_local_description(&offer_init)).await.unwrap();
        
        let msg = json!({ "type": "OFFER", "sdp": sdp_str });
        
        if let Some(ws) = &state_offer.borrow().ws {
            let _ = ws.send_with_str(&msg.to_string());
        }
    });
}

fn process_frame(state: Rc<RefCell<ClientState>>, data: &[u8]) {
    // Attempt to decompress Zstd frame
    let cursor = std::io::Cursor::new(data);
    let mut decoder = match StreamingDecoder::new(cursor) {
        Ok(d) => d,
        Err(e) => {
            console::warn_1(&format!("Zstd Init Error: {:?}", e).into());
            return;
        }
    };

    let mut decompressed = Vec::new();
    if let Err(e) = decoder.read_to_end(&mut decompressed) {
        console::warn_1(&format!("Zstd Decompress Error: {:?}", e).into());
        return;
    }

    let mut client = state.borrow_mut();
    
    client.frame_count = client.frame_count.wrapping_add(1);
    if client.frame_count == 1 || client.frame_count % 120 == 0 {
        console::log_1(&format!("Rendered Frame #{} ({} bytes)", client.frame_count, decompressed.len()).into());
    }
    
    // Safety check: if overlay is still visible (maybe WELCOME didn't trigger it?), hide it now
    if !client.game_started {
        client.game_started = true;
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(overlay) = document.get_element_by_id("loading-overlay") {
                    let _ = overlay.class_list().add_1("hidden");
                }
            }
        }
    }

    let audio = client.audio.clone();
    if let Err(e) = client.renderer.render_frame(&decompressed, &audio) {
        console::warn_1(&format!("Render Error: {}", e).into());
    }
}

fn setup_input(state: Rc<RefCell<ClientState>>) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let state_down = state.clone();
    let onkeydown = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
        // Resume audio on first interaction
        state_down.borrow().audio.resume();
        
        if !e.repeat() {
            send_input(state_down.clone(), e.key_code(), true);
        }
    });
    document.add_event_listener_with_callback("keydown", onkeydown.as_ref().unchecked_ref())?;
    onkeydown.forget();

    let state_up = state.clone();
    let onkeyup = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
        send_input(state_up.clone(), e.key_code(), false);
    });
    document.add_event_listener_with_callback("keyup", onkeyup.as_ref().unchecked_ref())?;
    onkeyup.forget();
    
    // Also add click listener to resume audio (mobile/mouse)
    let state_click = state.clone();
    let onclick = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
        state_click.borrow().audio.resume();
    });
    document.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref())?;
    onclick.forget();

    Ok(())
}

fn send_input(state: Rc<RefCell<ClientState>>, code: u32, is_down: bool) {
    let mut buf = [0u8; 2];
    buf[0] = code as u8;
    buf[1] = if is_down { 1 } else { 0 };

    let client = state.borrow();
    if let Some(dc) = &client.dc {
        if dc.ready_state() == web_sys::RtcDataChannelState::Open {
            let _ = dc.send_with_u8_array(&buf);
            return;
        }
    }
    if let Some(ws) = &client.ws {
        if ws.ready_state() == WebSocket::OPEN {
            let _ = ws.send_with_u8_array(&buf);
        }
    }
}

fn load_predictor(state: Rc<RefCell<ClientState>>) {
    let window = web_sys::window().unwrap();
    let origin = window.location().origin().unwrap();
    let url = format!("{}/model.bin", origin); 

    wasm_bindgen_futures::spawn_local(async move {
        match fetch_bytes(&url).await {
            Ok(bytes) => {
                match Predictor::new(&bytes) {
                    Ok(p) => {
                        console::log_1(&"Predictive Model Loaded!".into());
                        state.borrow_mut().predictor = Some(p);
                    },
                    Err(e) => console::warn_1(&format!("Model init failed: {}", e).into()),
                }
            },
            Err(_) => console::warn_1(&"Model not found or fetch failed".into()),
        }
    });
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    let window = web_sys::window().unwrap();
    let resp = JsFuture::from(window.fetch_with_str(url)).await?;
    let resp: web_sys::Response = resp.dyn_into()?;
    if !resp.ok() { return Err("HTTP Error".into()); }
    let buf = JsFuture::from(resp.array_buffer()?).await?;
    let u8arr = js_sys::Uint8Array::new(&buf);
    Ok(u8arr.to_vec())
}
