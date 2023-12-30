use core::cell::{RefCell, UnsafeCell};
use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{self, Ordering};

use cortex_m::interrupt::{CriticalSection, Mutex};

use crate::hardware_config::DisplayType;
use crate::ui::draw_panic_screen;

static PANIC_DISPLAY_REF: Mutex<RefCell<Option<&mut DisplayType>>> = Mutex::new(RefCell::new(None));

pub fn set_panic_display_ref(display: &UnsafeCell<DisplayType>) {
    cortex_m::interrupt::free(|cs| {
        *PANIC_DISPLAY_REF.borrow(cs).borrow_mut() = Some(unsafe { &mut *display.get() });
    });
}

#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // We're dying, go all out just this once
    let cs = unsafe { CriticalSection::new() };
    let display = PANIC_DISPLAY_REF.borrow(&cs).borrow_mut().take().unwrap();

    unsafe {
        cortex_m::interrupt::enable();
    }

    let mut message = heapless::String::<256>::default();

    if write!(message, "{info}").is_err() {
        let _ = write!(message, "Could not format panic message");
    }

    draw_panic_screen(&mut **display, message.as_ref());

    cortex_m::interrupt::disable();

    loop {
        // add some side effect to prevent this from turning into a UDF instruction
        // see rust-lang/rust#28728 for details
        atomic::compiler_fence(Ordering::SeqCst);
    }
}
