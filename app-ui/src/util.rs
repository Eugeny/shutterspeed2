#[cfg(target_os = "none")]
pub async fn delay_ms(ms: u32) {
    use fugit::ExtU32;
    use rtic_monotonics::systick::Systick;
    Systick::delay(ms.millis()).await;
}

#[cfg(not(target_os = "none"))]
pub async fn delay_ms(ms: u32) {
    #[cfg(feature = "std")]
    tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
}
