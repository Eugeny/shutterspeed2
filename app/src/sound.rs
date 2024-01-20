use fugit::ExtU32;
use note_frequencies::note_frequencies_32;
use rtic_monotonics::systick::Systick;

note_frequencies_32!(440.0);

pub const NOTE_A0: usize = 69;

pub enum Chirp {
    Startup,
    Button,
    Measuring,
    Done,
}

pub trait BeeperExt {
    fn enable(&mut self, frequency: f32);

    fn disable(&mut self);

    fn set_duty_percent(&mut self, duty_percent: u8);

    fn note(&mut self, note: isize) {
        self.enable(NOTE_FREQUENCIES[(NOTE_A0 as isize + note) as usize])
    }

    async fn play(&mut self, note: isize, duration_millis: u32) {
        self.set_duty_percent(10);
        self.note(note - 12);
        Systick::delay(10.millis()).await;
        self.set_duty_percent(50);
        self.note(note);
        Systick::delay(duration_millis.millis()).await;
        self.disable();
    }
}
