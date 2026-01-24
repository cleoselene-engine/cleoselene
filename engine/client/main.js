import { decompress } from 'https://unpkg.com/fzstd@0.1.1/esm/index.mjs';

// Protocol Constants
const OP_CLEAR = 0x01;
const OP_SET_COLOR = 0x02;
const OP_FILL_RECT = 0x03;
const OP_DRAW_LINE = 0x04;
const OP_DRAW_TEXT = 0x05;
const OP_LOAD_SOUND = 0x06;
const OP_PLAY_SOUND = 0x07;
const OP_STOP_SOUND = 0x08;
const OP_SET_VOLUME = 0x09;
const OP_LOAD_IMAGE = 0x0A;
const OP_DRAW_IMAGE = 0x0B;

// Global State
let ctx = null;
let ws = null;
let pc = null;
let dc = null;
let audioCtx = null;
const sounds = {};
const images = {};
const activeSources = {};
let sessionId = null;
let gameStarted = false;
let reconnectAttempts = 0;
let reconnectTimer = null;
let initialServerInstanceId = null;

// Stats
let frameCount = 0;
let lastLogTime = 0;
let totalBytes = 0;

function getBasePath() {
    return (window.CLEOSELENE_CONFIG && window.CLEOSELENE_CONFIG.basePath) || "";
}

function updateLoadingStatus(text) {
    const el = document.getElementById('loading-text');
    if (el) el.textContent = text;
}

function showLoading(text) {
    updateLoadingStatus(text);
    const overlay = document.getElementById('loading-overlay');
    if (overlay) overlay.classList.remove('hidden');
}

function hideLoading() {
    const overlay = document.getElementById('loading-overlay');
    if (overlay) overlay.classList.add('hidden');
}

function setupTouchListeners(container) {
    container.querySelectorAll('.touch-btn').forEach(btn => {
        const key = parseInt(btn.dataset.key);
        const handleStart = (e) => { e.preventDefault(); sendInput(key, true); };
        const handleEnd = (e) => { e.preventDefault(); sendInput(key, false); };
        btn.addEventListener('touchstart', handleStart, {passive: false});
        btn.addEventListener('touchend', handleEnd, {passive: false});
        btn.addEventListener('mousedown', handleStart);
        btn.addEventListener('mouseup', handleEnd);
        btn.addEventListener('mouseleave', handleEnd);
    });
}

function buildMobileControls(layout) {
    if (!layout || !layout.length) return;
    const div = document.createElement('div');
    div.id = 'mobile-controls';
    div.className = 'touch-controls';
    // Display handled by CSS Media Queries
    
    let html = '';
    layout.forEach(row => {
        html += `<div class='control-row' style='display: grid; grid-template-columns: repeat(${row.length}, 1fr); gap: 10px;'>`;
        row.forEach(btn => {
            html += `<div class='touch-btn' data-key='${btn.key}'>${btn.label}</div>`;
        });
        html += `</div>`;
    });
    div.innerHTML = html;
    document.body.appendChild(div);
    setupTouchListeners(div);
}

function init() {
    console.log("App Mounted. Initializing Engine with WebRTC...");
    showLoading("INITIALIZING...");
    
    // Handle Mobile Controls (Responsive: Always Init, CSS hides on Desktop)
    const controls = document.getElementById('mobile-controls');
    if (controls) {
        setupTouchListeners(controls);
    } else {
        // Fallback: Fetch keys.json
        const bp = getBasePath();
        fetch(bp + '/assets/keys.json')
            .then(r => { if(r.ok) return r.json(); throw new Error(r.status); })
            .then(layout => buildMobileControls(layout))
            .catch(e => {
                console.warn("Mobile controls fetch failed, using default:", e);
                // Hardcoded Default Layout (Astro Maze)
                const defaultLayout = [
                    [{label: "🚀", key: 38}, {label: "⚡️", key: 90}],
                    [{label: "⬅️", key: 37}, {label: "➡️", key: 39}]
                ];
                buildMobileControls(defaultLayout);
            });
    }

    // Metadata Fetch
    const bp = getBasePath();
    fetch(bp + '/assets/metadata.json')
        .then(res => res.json())
        .then(meta => {
            if (meta.title) document.title = meta.title;
            if (meta.favicon) {
                let link = document.querySelector("link[rel~='icon']");
                if (!link) { link = document.createElement('link'); link.rel = 'icon'; document.head.appendChild(link); }
                link.href = meta.favicon;
            }
            if (meta.themeColor) {
                let metaColor = document.querySelector("meta[name='theme-color']");
                if (!metaColor) { metaColor = document.createElement('meta'); metaColor.name = 'theme-color'; document.head.appendChild(metaColor); }
                metaColor.content = meta.themeColor;
            }
            console.log("Metadata applied:", meta);
        })
        .catch(e => console.error("Failed to load metadata.json:", e));
    
    // Setup Canvas
    const canvas = document.getElementById('gameCanvas');
    const dpr = window.devicePixelRatio || 1;
    canvas.width = 800 * dpr;
    canvas.height = 600 * dpr;
    ctx = canvas.getContext('2d');
    ctx.scale(dpr, dpr);
    
    // Setup Audio
    try {
        audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    } catch (e) { console.error("AudioContext failed:", e); }

    const resumeAudio = () => {
        if (audioCtx && audioCtx.state === 'suspended') audioCtx.resume();
    };
    window.addEventListener('click', resumeAudio);
    window.addEventListener('keydown', resumeAudio);

    // Input Handling setup
    window.addEventListener('keydown', (e) => { if(!e.repeat) sendInput(e.keyCode, true); });
    window.addEventListener('keyup', (e) => { sendInput(e.keyCode, false); });

    // Connect
    connect();
}

function scheduleReconnect() {
    if (reconnectTimer) clearTimeout(reconnectTimer);
    const delay = Math.min(1000 * Math.pow(1.5, reconnectAttempts), 10000); 
    reconnectAttempts++;
    console.log(`Reconnecting in ${delay}ms (Attempt ${reconnectAttempts})...`);
    showLoading(`RECONNECTING (${reconnectAttempts})...`);
    reconnectTimer = setTimeout(() => { connect(); }, delay);
}

async function connect() {
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
    updateLoadingStatus(reconnectAttempts > 0 ? "RECONNECTING..." : "CONNECTING TO SERVER...");
    
    const urlParams = new URLSearchParams(window.location.search);
    const urlSessionId = urlParams.get('session');
    const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
    let wsUrl = protocol + window.location.host + getBasePath() + "/ws";
    if (urlSessionId) { wsUrl += "?session=" + urlSessionId; sessionId = urlSessionId; }
    if (sessionId && reconnectAttempts > 0 && !wsUrl.includes("session=")) {
         wsUrl += (wsUrl.includes("?") ? "&" : "?") + "session=" + sessionId;
    }

    ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';

    ws.onopen = () => {
        console.log("WebSocket Open. Negotiating WebRTC...");
        updateLoadingStatus("NEGOTIATING...");
        reconnectAttempts = 0; 
        setupWebRTC();
    };

    ws.onclose = () => {
        console.log("WebSocket Closed");
        if (pc) pc.close();
        ws = null;
        gameStarted = false; // Reset to allow hiding on next first frame
        scheduleReconnect();
    };

    ws.onmessage = async (event) => {
        const data = event.data;
        if (typeof data === 'string') {
            const msg = JSON.parse(data);
            if (msg.type === 'WELCOME') {
                console.log("Session Joined:", msg.session_id);
                if (msg.server_instance_id) {
                    if (initialServerInstanceId === null) {
                        initialServerInstanceId = msg.server_instance_id;
                    } else if (initialServerInstanceId !== msg.server_instance_id) {
                        console.log("Server instance changed. Reloading...");
                        showLoading("UPDATING CLIENT...");
                        setTimeout(() => window.location.reload(), 500);
                        return;
                    }
                }
                updateLoadingStatus("ENTERING GAME...");
                sessionId = msg.session_id;
                const cleanUrl = window.location.protocol + "//" + window.location.host + window.location.pathname;
                window.history.replaceState({path: cleanUrl}, '', cleanUrl);
            } else if (msg.type === 'ANSWER') {
                await pc.setRemoteDescription(new RTCSessionDescription({ type: 'answer', sdp: msg.sdp }));
            } else if (msg.type === 'CANDIDATE') {
                try {
                    await pc.addIceCandidate(new RTCIceCandidate({
                        candidate: msg.candidate, sdpMid: msg.sdp_mid, sdpMLineIndex: msg.sdp_mline_index
                    }));
                } catch (e) { console.error("Error adding candidate:", e); }
            }
        } else {
            processCompressedFrame(data);
        }
    };
}

async function setupWebRTC() {
    const config = { iceServers: [{ urls: "stun:stun.l.google.com:19302" }] };
    pc = new RTCPeerConnection(config);
    dc = pc.createDataChannel("game_data", { ordered: false, maxRetransmits: 0 });
    dc.binaryType = "arraybuffer";
    dc.onopen = () => { 
        console.log("DataChannel OPEN! Switching to UDP."); 
        updateLoadingStatus("ENTERING GAME (UDP)...");
    };
    dc.onmessage = (e) => processCompressedFrame(e.data);
    pc.onicecandidate = (event) => {
        if (event.candidate) {
            ws.send(JSON.stringify({
                type: 'CANDIDATE', candidate: event.candidate.candidate,
                sdp_mid: event.candidate.sdpMid, sdp_mline_index: event.candidate.sdpMLineIndex
            }));
        }
    };
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    ws.send(JSON.stringify({ type: 'OFFER', sdp: offer.sdp }));
}

async function processCompressedFrame(data) {
    try {
        frameCount++;
        totalBytes += data.byteLength || data.size || 0;
        const now = performance.now();
        if (now - lastLogTime > 1000) {
            frameCount = 0; totalBytes = 0; lastLogTime = now;
        }
        let streamData = data;
        if (data instanceof Blob) { streamData = await data.arrayBuffer(); }
        
        // Zstd Decompression (Standard)
        const decompressed = decompress(new Uint8Array(streamData));
        renderFrame(new DataView(decompressed.buffer));
    } catch (e) {
        console.error("Frame Error:", e);
    }
}

function sendInput(code, isDown) {
    const buf = new Uint8Array(2);
    buf[0] = code; buf[1] = isDown ? 1 : 0;
    if (dc && dc.readyState === 'open') { dc.send(buf); } 
    else if (ws && ws.readyState === WebSocket.OPEN) { ws.send(buf); }
}

function renderFrame(view) {
    if (!gameStarted) {
        console.log("First Frame Received! Hiding Overlay.");
        gameStarted = true;
        hideLoading();
    }
    let offset = 0;
    const len = view.byteLength;
    if (!ctx) return;
    while (offset < len) {
        const opcode = view.getUint8(offset);
        offset += 1;
        if (opcode === OP_CLEAR) {
            const r = view.getUint8(offset); const g = view.getUint8(offset + 1); const b = view.getUint8(offset + 2);
            offset += 3;
            ctx.fillStyle = `rgb(${r},${g},${b})`;
            ctx.fillRect(0, 0, 800, 600);
        } 
        else if (opcode === OP_SET_COLOR) {
            const r = view.getUint8(offset); const g = view.getUint8(offset + 1); const b = view.getUint8(offset + 2); const a = view.getUint8(offset + 3);
            offset += 4;
            const color = `rgba(${r},${g},${b},${a/255})`;
            ctx.fillStyle = color; ctx.strokeStyle = color;
        }
        else if (opcode === OP_FILL_RECT) {
            const x = view.getFloat32(offset, true); offset += 4;
            const y = view.getFloat32(offset, true); offset += 4;
            const w = view.getFloat32(offset, true); offset += 4;
            const h = view.getFloat32(offset, true); offset += 4;
            ctx.fillRect(x, y, w, h);
        }
        else if (opcode === OP_DRAW_LINE) {
            const x1 = view.getFloat32(offset, true); offset += 4;
            const y1 = view.getFloat32(offset, true); offset += 4;
            const x2 = view.getFloat32(offset, true); offset += 4;
            const y2 = view.getFloat32(offset, true); offset += 4;
            const w = view.getFloat32(offset, true); offset += 4;
            ctx.lineWidth = w; ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke(); ctx.lineWidth = 1;
        }
        else if (opcode === OP_DRAW_TEXT) {
            const x = view.getFloat32(offset, true); offset += 4;
            const y = view.getFloat32(offset, true); offset += 4;
            const textLen = view.getUint16(offset, true); offset += 2;
            const textBuffer = new Uint8Array(view.buffer, view.byteOffset + offset, textLen);
            offset += textLen;
            const text = new TextDecoder().decode(textBuffer);
            ctx.font = "14px monospace"; ctx.textBaseline = "middle"; ctx.fillText(text, x, y);
        }
        else if (opcode === OP_LOAD_SOUND) {
            const nameLen = view.getUint16(offset, true); offset += 2;
            const name = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, nameLen)); offset += nameLen;
            const urlLen = view.getUint16(offset, true); offset += 2;
            let url = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, urlLen)); offset += urlLen;
            
            // Fix Path for Subdirectory Deployment
            if (url.startsWith('/') && !url.startsWith('//')) {
                const bp = getBasePath();
                if (bp && !url.startsWith(bp)) {
                    url = bp + url;
                }
            }

            if (!sounds[name]) {
                sounds[name] = "loading"; 
                fetch(url)
                    .then(r => r.arrayBuffer())
                    .then(ab => audioCtx.decodeAudioData(ab))
                    .then(buf => { sounds[name] = buf; })
                    .catch(e => console.error("Sound load failed:", name, e));
            }
        }
        else if (opcode === OP_PLAY_SOUND) {
            const nameLen = view.getUint16(offset, true); offset += 2;
            const name = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, nameLen)); offset += nameLen;
            const shouldLoop = view.getUint8(offset) === 1; offset += 1;
            const volume = view.getFloat32(offset, true); offset += 4;
            if (sounds[name] && typeof sounds[name] !== "string" && audioCtx) {
                try {
                    if (activeSources[name] && shouldLoop) { try { activeSources[name].source.stop(); } catch(e){} }
                    const source = audioCtx.createBufferSource(); source.buffer = sounds[name]; source.loop = shouldLoop;
                    const gainNode = audioCtx.createGain(); gainNode.gain.value = volume; 
                    source.connect(gainNode); gainNode.connect(audioCtx.destination);
                    source.start(0);
                    source.onended = () => { if (activeSources[name] && activeSources[name].source === source) { delete activeSources[name]; } };
                    activeSources[name] = { source, gain: gainNode };
                } catch (e) { console.error(e); }
            }
        }
        else if (opcode === OP_STOP_SOUND) {
            const nameLen = view.getUint16(offset, true); offset += 2;
            const name = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, nameLen)); offset += nameLen;
            const active = activeSources[name];
            if (active && audioCtx) {
                try {
                    const now = audioCtx.currentTime;
                    active.gain.gain.setValueAtTime(active.gain.gain.value, now);
                    active.gain.gain.linearRampToValueAtTime(0, now + 0.5);
                    active.source.stop(now + 0.5); delete activeSources[name];
                } catch(e) {}
            }
        }
        else if (opcode === OP_SET_VOLUME) {
            const nameLen = view.getUint16(offset, true); offset += 2;
            const name = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, nameLen)); offset += nameLen;
            const volume = view.getFloat32(offset, true); offset += 4;
            const active = activeSources[name];
            if (active && audioCtx) {
                try { active.gain.gain.setTargetAtTime(volume, audioCtx.currentTime, 0.1); } catch(e) {}
            }
        }
        else if (opcode === OP_LOAD_IMAGE) {
            const nameLen = view.getUint16(offset, true); offset += 2;
            const name = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, nameLen)); offset += nameLen;
            const urlLen = view.getUint16(offset, true); offset += 2;
            let url = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, urlLen)); offset += urlLen;
            
            if (url.startsWith('/') && !url.startsWith('//')) {
                const bp = getBasePath();
                if (bp && !url.startsWith(bp)) {
                    url = bp + url;
                }
            }

            if (!images[name]) {
                images[name] = "loading";
                fetch(url)
                    .then(r => r.blob())
                    .then(blob => createImageBitmap(blob))
                    .then(bmp => { images[name] = bmp; })
                    .catch(e => console.error("Image load failed:", name, e));
            }
        }
        else if (opcode === OP_DRAW_IMAGE) {
            const nameLen = view.getUint16(offset, true); offset += 2;
            const name = new TextDecoder().decode(new Uint8Array(view.buffer, view.byteOffset + offset, nameLen)); offset += nameLen;
            
            const dx = view.getFloat32(offset, true); offset += 4;
            const dy = view.getFloat32(offset, true); offset += 4;
            const dw = view.getFloat32(offset, true); offset += 4;
            const dh = view.getFloat32(offset, true); offset += 4;
            
            const sx = view.getFloat32(offset, true); offset += 4;
            const sy = view.getFloat32(offset, true); offset += 4;
            const sw = view.getFloat32(offset, true); offset += 4;
            const sh = view.getFloat32(offset, true); offset += 4;
            
            const rot = view.getFloat32(offset, true); offset += 4;
            const ox = view.getFloat32(offset, true); offset += 4;
            const oy = view.getFloat32(offset, true); offset += 4;

            const img = images[name];
            if (img && typeof img !== "string") {
                let finalW = (dw !== -1) ? dw : img.width;
                const finalH = (dh !== -1) ? dh : img.height;
                const finalSW = (sw !== -1) ? sw : img.width;
                const finalSH = (sh !== -1) ? sh : img.height;
                
                ctx.save();
                ctx.translate(dx, dy);
                if (rot !== 0) ctx.rotate(rot);
                
                if (finalW < 0) {
                    ctx.scale(-1, 1);
                    finalW = -finalW; // Make positive for drawImage
                }
                
                ctx.drawImage(img, sx, sy, finalSW, finalSH, -ox, -oy, finalW, finalH);
                ctx.restore();
            }
        }
        else { break; }
    }
}
document.addEventListener('DOMContentLoaded', init);
