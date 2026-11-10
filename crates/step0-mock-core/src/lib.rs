use core_traits::{buttons, EmulatorCore, PixelFormat, VideoFrameInfo};

const WIDTH: usize = 256;
const HEIGHT: usize = 224;
const TOTAL_PIXELS: usize = WIDTH * HEIGHT;
const AUDIO_SAMPLE_RATE: u32 = 44100;
const SAMPLES_PER_FRAME: usize = 735; // 44100 / 60
const AUDIO_BUFFER_CAPACITY: usize = SAMPLES_PER_FRAME * 4; // stereo pairs * 2 frames headroom

pub struct MockCore {
    framebuffer: [u32; TOTAL_PIXELS], // RGBA8888 packed
    audio_buffer: [f32; AUDIO_BUFFER_CAPACITY * 2], // Stereo interleaved: L, R
    audio_samples_count: usize,
    frame_count: u32,
    audio_phase: f32,
    input_mask: u16,
    // Interactive bouncing sprite position
    sprite_x: i32,
    sprite_y: i32,
    sprite_vx: i32,
    sprite_vy: i32,
}

impl MockCore {
    pub fn new() -> Self {
        Self {
            framebuffer: [0xFF000000; TOTAL_PIXELS],
            audio_buffer: [0.0; AUDIO_BUFFER_CAPACITY * 2],
            audio_samples_count: 0,
            frame_count: 0,
            audio_phase: 0.0,
            input_mask: 0,
            sprite_x: (WIDTH / 2) as i32,
            sprite_y: (HEIGHT / 2) as i32,
            sprite_vx: 2,
            sprite_vy: 1,
        }
    }

    fn render_test_pattern(&mut self) {
        let t = self.frame_count;

        // 1. Draw animated retro background gradient and scanlines
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = y * WIDTH + x;

                if y < 160 {
                    // SMPTE-inspired animated vertical bars
                    let bar = (x + (t as usize / 2)) % WIDTH / 32;
                    let (r, g, b) = match bar {
                        0 => (220, 220, 220), // Light Grey
                        1 => (200, 200, 30),  // Yellow
                        2 => (30, 200, 200),  // Cyan
                        3 => (30, 200, 30),   // Green
                        4 => (200, 30, 200),  // Magenta
                        5 => (200, 30, 30),   // Red
                        6 => (30, 30, 200),   // Blue
                        _ => (15, 15, 15),    // Dark Blue/Black
                    };
                    // Slight vertical gradient
                    let dim = 255 - ((y * 80) / HEIGHT) as u8;
                    let final_r = ((r as u16 * dim as u16) >> 8) as u8;
                    let final_g = ((g as u16 * dim as u16) >> 8) as u8;
                    let final_b = ((b as u16 * dim as u16) >> 8) as u8;
                    self.framebuffer[idx] = (0xFF << 24) | ((final_b as u32) << 16) | ((final_g as u32) << 8) | (final_r as u32);
                } else {
                    // Lower third: Dark calibration area with moving rainbow bars
                    let wave = ((x as i32 + (t as i32 * 3)).rem_euclid(256)) as u8;
                    let r = wave;
                    let g = 255 - wave;
                    let b = ((y - 160) * 4) as u8;
                    self.framebuffer[idx] = (0xFF << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
                }
            }
        }

        // 2. Update interactive sprite based on input D-pad or auto-bounce
        let speed = if (self.input_mask & buttons::B) != 0 { 4 } else { 2 };
        if (self.input_mask & buttons::UP) != 0 { self.sprite_y -= speed; }
        if (self.input_mask & buttons::DOWN) != 0 { self.sprite_y += speed; }
        if (self.input_mask & buttons::LEFT) != 0 { self.sprite_x -= speed; }
        if (self.input_mask & buttons::RIGHT) != 0 { self.sprite_x += speed; }

        if self.input_mask == 0 {
            // Auto bounce if no buttons pressed
            self.sprite_x += self.sprite_vx;
            self.sprite_y += self.sprite_vy;
            if self.sprite_x <= 16 || self.sprite_x >= (WIDTH as i32 - 16) { self.sprite_vx = -self.sprite_vx; }
            if self.sprite_y <= 16 || self.sprite_y >= (HEIGHT as i32 - 16) { self.sprite_vy = -self.sprite_vy; }
        }

        // Clamp sprite in bounds
        self.sprite_x = self.sprite_x.clamp(16, WIDTH as i32 - 16);
        self.sprite_y = self.sprite_y.clamp(16, HEIGHT as i32 - 16);

        // 3. Draw a 16x16 pixel retro badge (Square with crosshair)
        let sprite_color: u32 = if (self.input_mask & buttons::A) != 0 {
            0xFF00FF00 // Green on A
        } else if (self.input_mask & buttons::B) != 0 {
            0xFF0000FF // Red on B
        } else if (self.input_mask & buttons::X) != 0 {
            0xFFFF00FF // Purple on X
        } else if (self.input_mask & buttons::Y) != 0 {
            0xFF00FFFF // Yellow on Y
        } else {
            0xFFFFFFFF // White default
        };

        let sx = self.sprite_x;
        let sy = self.sprite_y;
        for dy in -8..=8 {
            for dx in -8..=8 {
                let px = sx + dx;
                let py = sy + dy;
                if px >= 0 && px < WIDTH as i32 && py >= 0 && py < HEIGHT as i32 {
                    let is_border = dx.abs() == 8 || dy.abs() == 8;
                    let is_center = dx == 0 || dy == 0;
                    if is_border || is_center {
                        self.framebuffer[(py as usize) * WIDTH + (px as usize)] = sprite_color;
                    }
                }
            }
        }
    }

    fn generate_audio(&mut self) {
        // Frequency shifts when buttons are pressed (Interactive audio feedback!)
        let freq = if (self.input_mask & buttons::A) != 0 {
            880.0 // A5
        } else if (self.input_mask & buttons::B) != 0 {
            587.33 // D5
        } else if (self.input_mask & buttons::X) != 0 {
            659.25 // E5
        } else if (self.input_mask & buttons::Y) != 0 {
            523.25 // C5
        } else {
            220.0 // A3 subtle drone
        };

        let volume = if self.input_mask != 0 { 0.15 } else { 0.04 };
        let phase_increment = (freq * 2.0 * core::f32::consts::PI) / AUDIO_SAMPLE_RATE as f32;

        for i in 0..SAMPLES_PER_FRAME {
            let sample = self.audio_phase.sin() * volume;
            self.audio_phase += phase_increment;
            if self.audio_phase > 2.0 * core::f32::consts::PI {
                self.audio_phase -= 2.0 * core::f32::consts::PI;
            }

            let out_idx = (self.audio_samples_count + i) * 2;
            if out_idx + 1 < self.audio_buffer.len() {
                self.audio_buffer[out_idx] = sample;     // Left
                self.audio_buffer[out_idx + 1] = sample; // Right
            }
        }
        self.audio_samples_count += SAMPLES_PER_FRAME;
    }
}

impl EmulatorCore for MockCore {
    type Error = ();

    fn load_rom(&mut self, _rom_data: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn reset(&mut self) {
        self.frame_count = 0;
        self.audio_samples_count = 0;
    }

    fn step_frame(&mut self) {
        self.render_test_pattern();
        self.generate_audio();
        self.frame_count = self.frame_count.wrapping_add(1);
    }

    fn video_info(&self) -> VideoFrameInfo {
        VideoFrameInfo {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            format: PixelFormat::Rgba8888,
            aspect_ratio: 4.0 / 3.0,
            target_fps: 60.0988,
        }
    }

    fn video_buffer_ptr(&self) -> *const u8 {
        self.framebuffer.as_ptr() as *const u8
    }

    fn video_buffer_len(&self) -> usize {
        self.framebuffer.len() * 4
    }

    fn audio_sample_rate(&self) -> u32 {
        AUDIO_SAMPLE_RATE
    }

    fn audio_buffer_ptr(&self) -> *const f32 {
        self.audio_buffer.as_ptr()
    }

    fn audio_samples_available(&self) -> usize {
        self.audio_samples_count
    }

    fn consume_audio_samples(&mut self, count: usize) {
        if count >= self.audio_samples_count {
            self.audio_samples_count = 0;
        } else {
            let remaining = self.audio_samples_count - count;
            // Shift remaining samples to front
            for i in 0..(remaining * 2) {
                self.audio_buffer[i] = self.audio_buffer[i + (count * 2)];
            }
            self.audio_samples_count = remaining;
        }
    }

    fn set_input_state(&mut self, _port: u8, button_mask: u16) {
        self.input_mask = button_mask;
    }
}

// ============================================================================
// WebAssembly C ABI Export Layer (Zero Overhead / No Emscripten)
// ============================================================================

use core::cell::UnsafeCell;

struct SyncCore(UnsafeCell<Option<MockCore>>);
unsafe impl Sync for SyncCore {}

static CORE: SyncCore = SyncCore(UnsafeCell::new(None));

#[inline(always)]
fn with_core_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut MockCore) -> R,
{
    unsafe {
        let ptr = CORE.0.get();
        (*ptr).as_mut().map(f)
    }
}

#[inline(always)]
fn with_core<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&MockCore) -> R,
{
    unsafe {
        let ptr = CORE.0.get();
        (*ptr).as_ref().map(f)
    }
}

#[no_mangle]
pub extern "C" fn core_init() {
    unsafe {
        let ptr = CORE.0.get();
        *ptr = Some(MockCore::new());
    }
}

#[no_mangle]
pub extern "C" fn core_step_frame() {
    with_core_mut(|c| c.step_frame());
}

#[no_mangle]
pub extern "C" fn core_get_fb_ptr() -> *const u8 {
    with_core(|c| c.video_buffer_ptr()).unwrap_or(core::ptr::null())
}

#[no_mangle]
pub extern "C" fn core_get_fb_len() -> usize {
    with_core(|c| c.video_buffer_len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn core_get_width() -> u32 {
    WIDTH as u32
}

#[no_mangle]
pub extern "C" fn core_get_height() -> u32 {
    HEIGHT as u32
}

#[no_mangle]
pub extern "C" fn core_get_audio_ptr() -> *const f32 {
    with_core(|c| c.audio_buffer_ptr()).unwrap_or(core::ptr::null())
}

#[no_mangle]
pub extern "C" fn core_get_audio_samples_available() -> usize {
    with_core(|c| c.audio_samples_available()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn core_consume_audio_samples(count: usize) {
    with_core_mut(|c| c.consume_audio_samples(count));
}

#[no_mangle]
pub extern "C" fn core_set_input(port: u8, mask: u16) {
    with_core_mut(|c| c.set_input_state(port, mask));
}
