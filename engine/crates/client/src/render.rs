use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use wasm_bindgen::JsCast;
use byteorder::{ByteOrder, LittleEndian};
use crate::audio::AudioManager;

// OpCodes
const OP_CLEAR: u8 = 0x01;
const OP_SET_COLOR: u8 = 0x02;
const OP_FILL_RECT: u8 = 0x03;
const OP_DRAW_LINE: u8 = 0x04;
const OP_DRAW_TEXT: u8 = 0x05;
const OP_LOAD_SOUND: u8 = 0x06;
const OP_PLAY_SOUND: u8 = 0x07;
const OP_STOP_SOUND: u8 = 0x08;
const OP_SET_VOLUME: u8 = 0x09;
const OP_LOAD_IMAGE: u8 = 0x0A;
const OP_DRAW_IMAGE: u8 = 0x0B;

pub struct Renderer {
    ctx: CanvasRenderingContext2d,
    width: f64,
    height: f64,
}

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|_| "Failed to get context")?
            .ok_or("Context not found")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Context cast failed")?;

        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        Ok(Self {
            ctx,
            width,
            height,
        })
    }

    pub fn render_frame(&mut self, data: &[u8], audio: &AudioManager) -> Result<(), String> {
        let mut offset = 0;
        while offset < data.len() {
            let op = data[offset];
            offset += 1;

            match op {
                OP_CLEAR => {
                    if offset + 3 > data.len() { break; }
                    let r = data[offset]; offset += 1;
                    let g = data[offset]; offset += 1;
                    let b = data[offset]; offset += 1;
                    let color = format!("rgb({},{},{})", r, g, b);
                    self.ctx.set_fill_style(&color.into());
                    self.ctx.fill_rect(0.0, 0.0, self.width, self.height);
                },
                OP_SET_COLOR => {
                    if offset + 4 > data.len() { break; }
                    let r = data[offset]; offset += 1;
                    let g = data[offset]; offset += 1;
                    let b = data[offset]; offset += 1;
                    let a = data[offset]; offset += 1;
                    let color = format!("rgba({},{},{},{})", r, g, b, a as f32 / 255.0);
                    self.ctx.set_fill_style(&color.clone().into());
                    self.ctx.set_stroke_style(&color.into());
                },
                OP_FILL_RECT => {
                    if offset + 16 > data.len() { break; }
                    let x = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let y = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let w = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let h = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    self.ctx.fill_rect(x, y, w, h);
                },
                OP_DRAW_LINE => {
                    if offset + 20 > data.len() { break; }
                    let x1 = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let y1 = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let x2 = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let y2 = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let w = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    
                    self.ctx.set_line_width(w);
                    self.ctx.begin_path();
                    self.ctx.move_to(x1, y1);
                    self.ctx.line_to(x2, y2);
                    self.ctx.stroke();
                    self.ctx.set_line_width(1.0); // Reset
                },
                OP_DRAW_TEXT => {
                    if offset + 2 > data.len() { break; }
                    let x = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let y = LittleEndian::read_f32(&data[offset..]) as f64; offset += 4;
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    
                    if offset + len > data.len() { break; }
                    let text_bytes = &data[offset..offset + len];
                    offset += len;
                    
                    if let Ok(text) = std::str::from_utf8(text_bytes) {
                        self.ctx.set_font("14px monospace");
                        self.ctx.set_text_baseline("middle");
                        let _ = self.ctx.fill_text(text, x, y);
                    }
                },
                OP_LOAD_SOUND => {
                    // Name
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    let name = String::from_utf8_lossy(&data[offset..offset+len]).to_string(); offset += len;

                    // URL
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    let url = String::from_utf8_lossy(&data[offset..offset+len]).to_string(); offset += len;

                    audio.preload_sound(name, url);
                },
                OP_PLAY_SOUND => {
                    // Name
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    let name = String::from_utf8_lossy(&data[offset..offset+len]).to_string(); offset += len;

                    if offset + 5 > data.len() { break; }
                    let loop_val = data[offset] != 0; offset += 1;
                    let vol = LittleEndian::read_f32(&data[offset..]); offset += 4;

                    audio.play_sound(&name, loop_val, vol);
                },
                OP_STOP_SOUND => {
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    let name = String::from_utf8_lossy(&data[offset..offset+len]).to_string(); offset += len;
                    
                    audio.stop_sound(&name);
                },
                OP_SET_VOLUME => {
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    let name = String::from_utf8_lossy(&data[offset..offset+len]).to_string(); offset += len;

                    if offset + 4 > data.len() { break; }
                    let vol = LittleEndian::read_f32(&data[offset..]); offset += 4;

                    audio.set_volume(&name, vol);
                },
                OP_LOAD_IMAGE => {
                    // Name
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    // let name = ...; 
                    offset += len;

                    // URL
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    // let url = ...;
                    offset += len;
                },
                OP_DRAW_IMAGE => {
                    // Name
                    if offset + 2 > data.len() { break; }
                    let len = LittleEndian::read_u16(&data[offset..]) as usize; offset += 2;
                    if offset + len > data.len() { break; }
                    // let name = ...;
                    offset += len;

                    // 11 floats: x, y, w, h, sx, sy, sw, sh, r, ox, oy
                    if offset + 44 > data.len() { break; }
                    offset += 44;
                },
                _ => {
                    // Unknown op, break to avoid desync
                    break;
                }
            }
        }
        Ok(())
    }
}
