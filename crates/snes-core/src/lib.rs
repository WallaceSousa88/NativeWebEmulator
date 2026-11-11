pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod dma;
pub mod ppu;
pub mod snes;

use core::cell::UnsafeCell;
use core_traits::EmulatorCore;
use snes::Snes;

struct SyncSnes(UnsafeCell<Option<Snes>>);
unsafe impl Sync for SyncSnes {}

static SNES: SyncSnes = SyncSnes(UnsafeCell::new(None));

#[inline(always)]
fn with_snes_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Snes) -> R,
{
    unsafe {
        let ptr = SNES.0.get();
        (*ptr).as_mut().map(f)
    }
}

#[inline(always)]
fn with_snes<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Snes) -> R,
{
    unsafe {
        let ptr = SNES.0.get();
        (*ptr).as_ref().map(f)
    }
}

// ============================================================================
// WebAssembly C ABI Export Layer
// ============================================================================

#[no_mangle]
pub extern "C" fn snes_init() {
    unsafe {
        let ptr = SNES.0.get();
        *ptr = Some(Snes::new());
    }
}

#[no_mangle]
pub extern "C" fn snes_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    core::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn snes_dealloc(ptr: *mut u8, len: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, len);
    }
}

#[no_mangle]
pub extern "C" fn snes_load_rom(ptr: *const u8, len: usize) -> u32 {
    if ptr.is_null() || len == 0 {
        return 1;
    }
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    with_snes_mut(|s| match s.load_rom(slice) {
        Ok(_) => 0,
        Err(_) => 1,
    })
    .unwrap_or(1)
}

#[no_mangle]
pub extern "C" fn snes_step_frame() {
    with_snes_mut(|s| s.step_frame());
}

#[no_mangle]
pub extern "C" fn snes_get_fb_ptr() -> *const u8 {
    with_snes(|s| s.video_buffer_ptr()).unwrap_or(core::ptr::null())
}

#[no_mangle]
pub extern "C" fn snes_get_fb_len() -> usize {
    with_snes(|s| s.video_buffer_len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn snes_get_width() -> u32 {
    with_snes(|s| s.video_info().width).unwrap_or(256)
}

#[no_mangle]
pub extern "C" fn snes_get_height() -> u32 {
    with_snes(|s| s.video_info().height).unwrap_or(224)
}

#[no_mangle]
pub extern "C" fn snes_get_audio_ptr() -> *const f32 {
    with_snes(|s| s.audio_buffer_ptr()).unwrap_or(core::ptr::null())
}

#[no_mangle]
pub extern "C" fn snes_get_audio_samples_available() -> usize {
    with_snes(|s| s.audio_samples_available()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn snes_consume_audio_samples(count: usize) {
    with_snes_mut(|s| s.consume_audio_samples(count));
}

#[no_mangle]
pub extern "C" fn snes_set_input(port: u8, mask: u16) {
    with_snes_mut(|s| s.set_input_state(port, mask));
}

#[no_mangle]
pub extern "C" fn snes_get_rom_type() -> u32 {
    with_snes(|s| {
        s.rom_header()
            .map(|h| match h.rom_type {
                cartridge::RomType::LoRom => 0,
                cartridge::RomType::HiRom => 1,
            })
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn snes_get_rom_reset_vector() -> u16 {
    with_snes(|s| s.rom_header().map(|h| h.reset_vector).unwrap_or(0)).unwrap_or(0)
}
