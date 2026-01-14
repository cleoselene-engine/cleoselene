use axum::{
    extract::{Query, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use engine::GameState;
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use notify::{Watcher, RecursiveMode, Event};
use std::sync::mpsc::channel;
use std::path::{Path, PathBuf};
use clap::Parser;
use rust_embed::RustEmbed;
use axum::http::{header, StatusCode, Uri};

// WebRTC Imports
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

// --- Architecture Types ---

// Embed Client Assets
#[derive(RustEmbed)]
#[folder = "../../client"]
struct ClientAssets;

const LUA_API_DOCS: &str = include_str!("../../../MANUAL.md");

#[derive(Parser)]
#[command(name = "Cleoselene", about = "A Multiplayer-First Server-Rendered Game Engine with Lua Scripting")]
#[command(version = env!("BUILD_TIMESTAMP"))]
#[command(after_help = LUA_API_DOCS)]
struct Cli {
    /// Path to the Lua game script
    script_path: PathBuf,

    /// Port to start the server on
    #[arg(long, default_value_t = 3425)]
    port: u16,

    /// Base path for the application (e.g. /game)
    #[arg(long, default_value = "/")]
    base_path: String,

    /// Export the embedded client assets to a directory (for static hosting)
    #[arg(long)]
    export_client: Option<PathBuf>,

    /// Enable the debug endpoint at /debug. 
    /// Accepts POST requests with Lua code and returns the evaluation result.
    #[arg(long)]
    debug: bool,

    /// Run the game script in test mode (headless). 
    /// Initializes the engine, runs init() and one update() cycle, then exits.
    #[arg(long)]
    test: bool,
}

struct ClientConnection {
    session_id: String,
    tx_render: mpsc::Sender<bytes::Bytes>,
    rx_input: mpsc::Receiver<(u8, bool)>,
}

// Global state used by Axum to push new clients to the game loop
struct AppState {
    // Queue of new clients waiting to join the game loop
    new_clients: Arc<Mutex<Vec<ClientConnection>>>,
    base_path: String,
    assets_dir: PathBuf,
    instance_id: String,
    tx_debug: Option<mpsc::Sender<(String, oneshot::Sender<String>)>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum SignalMessage {
    WELCOME { session_id: String, server_instance_id: String },
    OFFER { sdp: String },
    ANSWER { sdp: String },
    CANDIDATE { candidate: String, sdp_mid: Option<String>, sdp_mline_index: Option<u16> },
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    // Test Mode
    if args.test {
        println!("Running in TEST mode: {:?}", args.script_path);
        let script_path_str = args.script_path.to_string_lossy().to_string();
        
        match load_game(&script_path_str) {
            Some(game) => {
                println!("Script loaded successfully.");
                // Try running one update step
                if let Err(e) = game.update(0.1) {
                    eprintln!("Test Failed: Runtime error during update: {}", e);
                    std::process::exit(1);
                }
                println!("Test Passed: init() and update() executed without errors.");
                std::process::exit(0);
            }
            None => {
                eprintln!("Test Failed: Could not load script.");
                std::process::exit(1);
            }
        }
    }

    // Export Client Mode
    if let Some(target_dir) = args.export_client {
        println!("Exporting client assets to {:?}...", target_dir);
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            eprintln!("Failed to create directory: {}", e);
            std::process::exit(1);
        }

        for file_path in ClientAssets::iter() {
            if let Some(content) = ClientAssets::get(&file_path) {
                let dest_path = target_dir.join(file_path.as_ref());
                if let Some(parent) = dest_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&dest_path, content.data) {
                    eprintln!("Failed to write {}: {}", file_path, e);
                } else {
                    println!("  Extracted: {}", file_path);
                }
            }
        }
        println!("Export complete.");
        std::process::exit(0);
    }
    
    println!("Starting Cleoselene Server...");
    println!("Script: {:?}", args.script_path);
    println!("Port: {}", args.port);
    println!("Base Path: {}", args.base_path);

    let new_clients_queue = Arc::new(Mutex::new(Vec::new()));
    
    // Debug Channel
    let (tx_debug, rx_debug) = if args.debug {
        let (tx, rx) = mpsc::channel(10);
        println!("Debug endpoint enabled at /debug");
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Start the Global Game Loop
    let queue_clone = new_clients_queue.clone();
    let script_path = args.script_path.clone();
    
    thread::spawn(move || {
        game_loop(queue_clone, script_path, rx_debug);
    });

    // Determine assets dir (parent of script)
    let assets_dir = args.script_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    
    // Generate unique ID for this server process run
    let instance_id = Uuid::new_v4().to_string();
    println!("Server Instance ID: {}", instance_id);

    let app_state = Arc::new(AppState {
        new_clients: new_clients_queue,
        base_path: args.base_path.clone(),
        assets_dir: assets_dir.clone(),
        instance_id,
        tx_debug,
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/debug", post(debug_handler))
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Listening on http://localhost:{}", args.port);
    axum::serve(listener, app).await.unwrap();
}

async fn debug_handler(State(state): State<Arc<AppState>>, body: String) -> impl IntoResponse {
    if let Some(tx) = &state.tx_debug {
        let (reply_tx, reply_rx) = oneshot::channel();
        if tx.send((body, reply_tx)).await.is_ok() {
            if let Ok(result) = reply_rx.await {
                return result;
            }
        }
        "Error: Game loop unresponsive".to_string()
    } else {
        "Debug disabled".to_string()
    }
}

// Serve index.html with config injection from Embedded Assets
async fn serve_index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match ClientAssets::get("index.html") {
        Some(content) => {
            let body = std::str::from_utf8(content.data.as_ref()).unwrap();
            
            // Inject Config
            let config_script = format!(
                "<script>window.CLEOSELENE_CONFIG = {{ basePath: '{}' }};</script>",
                state.base_path.trim_end_matches('/')
            );
            
            // Inject Mobile Controls
            let controls_html = generate_controls_html(&state.assets_dir);

            let injected = body
                .replace("<!-- CLEOSELENE_CONFIG -->", &config_script)
                .replace("<!-- MOBILE_CONTROLS -->", &controls_html);

            (
                [(header::CONTENT_TYPE, "text/html")],
                injected
            ).into_response()
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct KeyDef {
    label: String,
    key: u32,
}

fn generate_controls_html(assets_dir: &Path) -> String {
    let keys_path = assets_dir.join("keys.json");
    if let Ok(file) = std::fs::File::open(keys_path) {
        let layout: Vec<Vec<KeyDef>> = serde_json::from_reader(file).unwrap_or_default();
        if layout.is_empty() { return String::new(); }

        let mut html = String::from("<div id='mobile-controls' class='touch-controls' style='display: none;'>");
        for row in layout {
            let cols = row.len();
            html.push_str(&format!("<div class='control-row' style='display: grid; grid-template-columns: repeat({}, 1fr); gap: 10px;'>", cols));
            for btn in row {
                html.push_str(&format!(
                    "<div class='touch-btn' data-key='{}'>{}</div>",
                    btn.key, btn.label
                ));
            }
            html.push_str("</div>");
        }
        html.push_str("</div>");
        html
    } else {
        String::new()
    }
}

// Serve other static files from Embedded Assets
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    
    match ClientAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data
            ).into_response()
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// --- Game Loop ---

struct ActiveClient {
    session_id: String,
    tx_render: mpsc::Sender<bytes::Bytes>,
    rx_input: mpsc::Receiver<(u8, bool)>,
}

fn game_loop(new_clients_queue: Arc<Mutex<Vec<ClientConnection>>>, script_path: PathBuf, mut rx_debug: Option<mpsc::Receiver<(String, oneshot::Sender<String>)>>) {
    println!("Global Game Loop Started");
    
    // Convert PathBuf to String for loading
    let script_path_str = script_path.to_string_lossy().to_string();
    
    // File Watcher
    let (tx_notify, rx_notify) = channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                let _ = tx_notify.send(());
            }
        }
    }).expect("Failed to create watcher");
    
    // Watch the parent directory of the script
    if let Some(parent) = script_path.parent() {
        if let Err(e) = watcher.watch(parent, RecursiveMode::Recursive) {
             eprintln!("Failed to watch directory {:?}: {}", parent, e);
        }
    } else {
         let _ = watcher.watch(Path::new("."), RecursiveMode::Recursive);
    }

    // Init Game
    let mut game = load_game(&script_path_str).expect("Failed to load initial game script");
    
    // Active Clients List
    let mut clients: Vec<ActiveClient> = Vec::new();

    let target_fps = 30;
    let frame_duration = Duration::from_micros(1_000_000 / target_fps);
    let mut last_time = Instant::now();

    loop {
        // 1. Hot Reload
        if rx_notify.try_recv().is_ok() {
            while rx_notify.try_recv().is_ok() {} // Drain
            thread::sleep(Duration::from_millis(50)); // Debounce
            println!("Hot Reload Triggered!");
            
            // Load new game without state preservation
            if let Some(new_game) = load_game(&script_path_str) {
                game = new_game;
                println!("Reload & Swap Successful!");
                
                // Re-register existing clients in the new Lua instance
                for client in &clients {
                    if let Ok(bytes) = game.on_connect(&client.session_id) {
                        let _ = client.tx_render.try_send(bytes);
                    }
                }
            }
        }

        let now = Instant::now();
        let dt = now.duration_since(last_time).as_secs_f32();
        last_time = now;

        // Reset frame state (events)
        game.begin_frame();

        // 2. Accept New Clients
        {
            let mut queue = new_clients_queue.lock().unwrap();
            while let Some(conn) = queue.pop() {
                println!("New player joined game: {}", conn.session_id);
                
                // Init player and get initialization commands (e.g. load_sound)
                match game.on_connect(&conn.session_id) {
                    Ok(bytes) => {
                        let _ = conn.tx_render.try_send(bytes);
                    },
                    Err(e) => {
                        eprintln!("Lua on_connect Error (Session {}): {}", conn.session_id, e);
                    }
                }
                
                clients.push(ActiveClient {
                    session_id: conn.session_id,
                    tx_render: conn.tx_render,
                    rx_input: conn.rx_input,
                });
            }
        }

        // Handle Debug
        if let Some(rx) = &mut rx_debug {
            if let Ok((code, tx)) = rx.try_recv() {
                let result = game.eval(&code);
                let _ = tx.send(result);
            }
        }

        // 3. Process Inputs & Prune Disconnected
        clients.retain_mut(|client| {
            // Read all pending inputs
            loop {
                match client.rx_input.try_recv() {
                    Ok((code, active)) => {
                        if let Err(e) = game.handle_input(&client.session_id, code, active) {
                            eprintln!("Input error {}: {}", client.session_id, e);
                        }
                    },
                    Err(mpsc::error::TryRecvError::Empty) => break, // No more inputs
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        println!("Player disconnected: {}", client.session_id);
                        let _ = game.on_disconnect(&client.session_id);
                        return false; // Remove from list
                    }
                }
            }
            true
        });

        // 4. Update World
        if let Err(e) = game.update(dt) {
            eprintln!("Update error: {}", e);
        }

        // 5. Render for Each Client
        clients.retain(|client| {
            match game.draw(&client.session_id) {
                Ok(bytes) => {
                    // Try to send. If receiver dropped (client closed connection), this fails.
                    // If channel full, we drop the frame (lag), but don't disconnect.
                    match client.tx_render.try_send(bytes) {
                        Ok(_) => true,
                        Err(mpsc::error::TrySendError::Full(_)) => true, // Lag
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                             println!("Render channel closed for {}", client.session_id);
                             let _ = game.on_disconnect(&client.session_id);
                             false // Remove
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Draw error {}: {}", client.session_id, e);
                    true
                }
            }
        });

        // Sleep
        let elapsed = now.elapsed();
        if elapsed < frame_duration {
            thread::sleep(frame_duration - elapsed);
        }
    }
}

fn load_game(path: &str) -> Option<GameState> {
    match std::fs::read_to_string(path) {
        Ok(script) => match GameState::new(&script, Some(std::path::Path::new(path))) {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("Lua Init Error: {}", e);
                None
            }
        },
        Err(e) => {
            eprintln!("File Read Error: {}", e);
            None
        }
    }
}

// --- Web Server Handlers ---

#[derive(Deserialize)]
struct WsParams {
    session: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, params.session))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>, requested_session: Option<String>) {
    let session_id = requested_session.unwrap_or_else(|| Uuid::new_v4().to_string());
    println!("Client {} connecting via WebSocket...", session_id);

    // 1. Send Handshake
    let handshake = SignalMessage::WELCOME { 
        session_id: session_id.clone(),
        server_instance_id: state.instance_id.clone() 
    };
    if let Err(e) = socket.send(Message::Text(serde_json::to_string(&handshake).unwrap().into())).await {
        eprintln!("Handshake failed: {}", e);
        return;
    }

    // 2. Prepare Game Loop Channels
    let (tx_render, mut rx_render) = mpsc::channel::<bytes::Bytes>(30); // From Game -> Network
    let (tx_input, rx_input) = mpsc::channel::<(u8, bool)>(100);       // From Network -> Game

    // Push to Game Loop
    {
        let mut queue = state.new_clients.lock().unwrap();
        queue.push(ClientConnection {
            session_id: session_id.clone(),
            tx_render,
            rx_input,
        });
    }

    // 3. Setup WebRTC API
    let mut m = MediaEngine::default();
    let registry = Registry::new();
    let registry = match register_default_interceptors(registry, &mut m) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("WebRTC Registry error: {}", e);
            return;
        }
    };
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // STUN Servers (Google is reliable)
    // Commented out to reduce FD usage on macOS dev environment and avoid DNS errors.
    // Localhost / Fallback mode works without this.
    let config = RTCConfiguration {
        // ice_servers: vec![RTCIceServer {
        //     urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        //     ..Default::default()
        // }],
        ..Default::default()
    };

    let peer_connection = match api.new_peer_connection(config).await {
        Ok(pc) => Arc::new(pc),
        Err(e) => {
             eprintln!("Failed to create PeerConnection: {}", e);
             return;
        }
    };

    // 4. Shared State for DataChannel
    // We need to pass the DataChannel from the callback to the sender task.
    let active_dc: Arc<tokio::sync::Mutex<Option<Arc<webrtc::data_channel::RTCDataChannel>>>> = Arc::new(tokio::sync::Mutex::new(None));
    let active_dc_clone = active_dc.clone();
    let session_id_rtc = session_id.clone();

    // 5. Handle Client-Initiated DataChannel
    // The client will create the DataChannel, ensuring the SDP Offer is valid.
    let tx_input_for_rtc = tx_input.clone();
    let session_id_for_dc = session_id_rtc.clone();
    peer_connection.on_data_channel(Box::new(move |dc: Arc<webrtc::data_channel::RTCDataChannel>| {
        let dc_label = dc.label().to_owned();
        let dc_id = dc.id();
        println!("New DataChannel {} Id: {} for session {}", dc_label, dc_id, session_id_for_dc);

        let active_dc_inner = active_dc_clone.clone();
        let tx_input_rtc = tx_input_for_rtc.clone();

        // Clone DC for use inside the on_open callback
        let dc_for_open = dc.clone();
        dc.on_open(Box::new(move || {
            println!("DataChannel '{}' open", dc_label);
            let dc_clone = dc_for_open.clone();
            let active_dc_inner = active_dc_inner.clone();
            Box::pin(async move {
                let mut lock = active_dc_inner.lock().await;
                *lock = Some(dc_clone);
            })
        }));

        dc.on_message(Box::new(move |msg: DataChannelMessage| {
            let tx = tx_input_rtc.clone();
            Box::pin(async move {
                let data = msg.data;
                if data.len() == 2 {
                    let code = data[0];
                    let active = data[1] != 0;
                    let _ = tx.send((code, active)).await;
                }
            })
        }));
        
        Box::pin(async {})
    }));

    // 6. WebSocket Signaling & Coordinator Loop
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let pc_clone = peer_connection.clone();

    // Spawn Coordinator Task (consumes rx_render)
    let active_dc_sender = active_dc.clone();
    
    let (tx_ws_frame, mut rx_ws_frame) = mpsc::channel::<Vec<u8>>(30);

    let coordinator_handle = tokio::spawn(async move {
        use std::io::Write;

        while let Some(bytes) = rx_render.recv().await {
            // println!("Sending frame: {} bytes", bytes.len());
            // Compress with Zstd (Standard, Level 0)
            let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 0).unwrap();
            
            if encoder.write_all(&bytes).is_ok() {
                if let Ok(compressed) = encoder.finish() {
                    let data = bytes::Bytes::from(compressed);
                    
                    // Check DC
                    let dc_opt = active_dc_sender.lock().await.clone();
                    let mut sent_via_udp = false;
                    
                    if let Some(dc) = dc_opt {
                         // Only try if actually Open
                         if dc.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open {
                             if let Err(_e) = dc.send(&data).await {
                                 // eprintln!("WebRTC Send Error: {}", _e);
                             } else {
                                 sent_via_udp = true;
                             }
                         }
                    } 
                    
                    if !sent_via_udp {
                         // Fallback TCP
                         let _ = tx_ws_frame.send(data.to_vec()).await;
                    }
                }
            }
        }
        println!("Coordinator task finished for session {}", session_id_rtc);
    });

    // Handle ICE Candidates from Local (Server) -> Remote (Client) via WebSocket
    let (tx_ws_sig, mut rx_ws_sig) = mpsc::channel::<Message>(100);
    
    peer_connection.on_ice_candidate(Box::new(move |c| {
        let tx = tx_ws_sig.clone();
        Box::pin(async move {
            if let Some(candidate) = c {
                if let Ok(json_cand) = candidate.to_json() {
                    let msg = SignalMessage::CANDIDATE {
                        candidate: json_cand.candidate,
                        sdp_mid: json_cand.sdp_mid,
                        sdp_mline_index: json_cand.sdp_mline_index,
                    };
                    let str_msg = serde_json::to_string(&msg).unwrap();
                    let _ = tx.send(Message::Text(str_msg.into())).await;
                }
            }
        })
    }));

    // Main Loop: Select between Incoming WS messages, Outgoing WS Frames (Fallback), Outgoing Signals
    loop {
        tokio::select! {
            // 1. Incoming WS Message
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                         // Handle Signaling
                         if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                            match signal {
                                SignalMessage::OFFER { sdp } => {
                                     let desc = RTCSessionDescription::offer(sdp).unwrap();
                                     if pc_clone.set_remote_description(desc).await.is_ok() {
                                         if let Ok(answer) = pc_clone.create_answer(None).await {
                                             if pc_clone.set_local_description(answer.clone()).await.is_ok() {
                                                 let resp = SignalMessage::ANSWER { sdp: answer.sdp };
                                                 let _ = ws_sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                                             }
                                         }
                                     }
                                },
                                SignalMessage::ANSWER { sdp } => {
                                    let desc = RTCSessionDescription::answer(sdp).unwrap();
                                    let _ = pc_clone.set_remote_description(desc).await;
                                },
                                SignalMessage::CANDIDATE { candidate, sdp_mid, sdp_mline_index } => {
                                    let cand = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                                        candidate,
                                        sdp_mid,
                                        sdp_mline_index,
                                        username_fragment: None,
                                    };
                                    let _ = pc_clone.add_ice_candidate(cand).await;
                                },
                                _ => {}
                            }
                        }
                    },
                    Some(Ok(Message::Binary(data))) => {
                        // Fallback Input
                        if data.len() == 2 {
                            let _ = tx_input.send((data[0], data[1] != 0)).await;
                        }
                    },
                    Some(Err(_)) | None => break, // Disconnected
                    _ => {}
                }
            },
            // 2. Outgoing WS Frame (Fallback)
            frame = rx_ws_frame.recv() => {
                if let Some(data) = frame {
                    if ws_sender.send(Message::Binary(data)).await.is_err() {
                        break;
                    }
                }
            },
            // 3. Outgoing Signaling
            sig = rx_ws_sig.recv() => {
                if let Some(msg) = sig {
                    if ws_sender.send(msg).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    
    println!("WS Handle Socket loop finished for {}", session_id);
    // Cleanup
    coordinator_handle.abort();
    let _ = peer_connection.close().await;
}
