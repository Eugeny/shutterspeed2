#[rustfmt::skip]
macro_rules! pin_macro {
    ($d:tt $name:ident, $gpio:expr, $pin:ident) => {
        #[macro_export]
        macro_rules! $name {
            ($d gpio:expr) => {
                $d gpio. $gpio . $pin
            };
        }
    };
}
