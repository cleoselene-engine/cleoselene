use web_sys::{AudioContext, AudioBuffer, AudioBufferSourceNode, GainNode, Response};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};

#[derive(Clone)]
pub struct AudioManager {
    ctx: Rc<AudioContext>,
    buffers: Rc<RefCell<HashMap<String, AudioBuffer>>>,
    active_sources: Rc<RefCell<HashMap<String, (AudioBufferSourceNode, GainNode)>>>,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let ctx = AudioContext::new().map_err(|e| format!("{:?}", e))?;
        Ok(Self {
            ctx: Rc::new(ctx),
            buffers: Rc::new(RefCell::new(HashMap::new())),
            active_sources: Rc::new(RefCell::new(HashMap::new())),
        })
    }

    pub fn resume(&self) {
        if self.ctx.state() == web_sys::AudioContextState::Suspended {
            let _ = self.ctx.resume();
        }
    }

    pub fn preload_sound(&self, name: String, url: String) {
        let manager = self.clone();
        spawn_local(async move {
            if let Err(e) = manager.load_sound_async(name, url).await {
                web_sys::console::warn_1(&format!("Failed to load sound: {}", e).into());
            }
        });
    }

    pub async fn load_sound_async(&self, name: String, url: String) -> Result<(), String> {
        if self.buffers.borrow().contains_key(&name) { return Ok(()); }

        let window = web_sys::window().ok_or("No window")?;
        
        let resp_value = JsFuture::from(window.fetch_with_str(&url)).await
            .map_err(|e| format!("Fetch failed: {:?}", e))?;
        let resp: Response = resp_value.dyn_into().map_err(|_| "Not a Response")?;
        
        let array_buffer_val = JsFuture::from(resp.array_buffer().map_err(|e| format!("{:?}", e))?).await
            .map_err(|e| format!("Buffer failed: {:?}", e))?;
            
        let promise = self.ctx.decode_audio_data(&array_buffer_val.dyn_into().unwrap())
            .map_err(|e| format!("Decode init failed: {:?}", e))?;
            
        let audio_buffer_val = JsFuture::from(promise).await
            .map_err(|e| format!("Decode failed: {:?}", e))?;
            
        let audio_buffer: AudioBuffer = audio_buffer_val.dyn_into().unwrap();

        self.buffers.borrow_mut().insert(name, audio_buffer);
        Ok(())
    }

    pub fn play_sound(&self, name: &str, loop_sound: bool, volume: f32) {
        if let Some(buffer) = self.buffers.borrow().get(name) {
            // Stop existing if any (simplistic logic)
            self.stop_sound(name);

            let source = match self.ctx.create_buffer_source() {
                Ok(s) => s,
                Err(_) => return,
            };
            source.set_buffer(Some(buffer));
            source.set_loop(loop_sound);

            let gain = match self.ctx.create_gain() {
                Ok(g) => g,
                Err(_) => return,
            };
            gain.gain().set_value(volume);

            let _ = source.connect_with_audio_node(&gain);
            let _ = gain.connect_with_audio_node(&self.ctx.destination());

            let _ = source.start();
            self.active_sources.borrow_mut().insert(name.to_string(), (source, gain));
        }
    }
    
    pub fn stop_sound(&self, name: &str) {
        if let Some((source, _)) = self.active_sources.borrow_mut().remove(name) {
            let _ = source.stop();
        }
    }

    pub fn set_volume(&self, name: &str, volume: f32) {
        if let Some((_, gain)) = self.active_sources.borrow().get(name) {
            gain.gain().set_value(volume);
        }
    }
}
