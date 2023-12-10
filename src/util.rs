use core::ops::{Deref, DerefMut};

use heapless::String;
use ufmt::uWrite;

#[derive(Default, Debug)]
pub struct EString<const L: usize>(String<L>);

impl<const L: usize> Deref for EString<L> {
    type Target = String<L>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const L: usize> DerefMut for EString<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const L: usize> uWrite for EString<L> {
    type Error = core::fmt::Error;

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.push_str(s).map_err(|_| core::fmt::Error)
    }

    fn write_char(&mut self, c: char) -> Result<(), Self::Error> {
        self.0.push(c).map_err(|_| core::fmt::Error)
    }
}
