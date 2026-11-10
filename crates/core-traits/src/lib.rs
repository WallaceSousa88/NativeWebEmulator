#![no_std]

pub enum PixelFormat {
    Rgba8888,
    Rgb565,
}

pub struct VideoFrameInfo {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub aspect_ratio: f32,
    pub target_fps: f64,
}

pub mod buttons {
    pub const B: u16      = 1 << 0;
    pub const Y: u16      = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16  = 1 << 3;
    pub const UP: u16     = 1 << 4;
    pub const DOWN: u16   = 1 << 5;
    pub const LEFT: u16   = 1 << 6;
    pub const RIGHT: u16  = 1 << 7;
    pub const A: u16      = 1 << 8;
    pub const X: u16      = 1 << 9;
    pub const L: u16      = 1 << 10;
    pub const R: u16      = 1 << 11;
}

pub trait EmulatorCore {
    type Error;

    fn load_rom(&mut self, rom_data: &[u8]) -> Result<(), Self::Error>;
    fn reset(&mut self);
    fn step_frame(&mut self);

    // Video Output
    fn video_info(&self) -> VideoFrameInfo;
    fn video_buffer_ptr(&self) -> *const u8;
    fn video_buffer_len(&self) -> usize;

    // Audio Output (Stereo f32 samples: L, R, L, R...)
    fn audio_sample_rate(&self) -> u32;
    fn audio_buffer_ptr(&self) -> *const f32;
    fn audio_samples_available(&self) -> usize;
    fn consume_audio_samples(&mut self, count: usize);

    // Controller Input
    fn set_input_state(&mut self, port: u8, button_mask: u16);
}
