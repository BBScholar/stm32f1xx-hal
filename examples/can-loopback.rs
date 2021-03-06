//! Showcases advanced CAN filter capabilities.
//! Does not require additional transceiver hardware.

#![no_main]
#![no_std]

use panic_halt as _;

use cortex_m_rt::entry;
use embedded_hal::digital::v2::OutputPin;
use nb::block;
use stm32f1xx_hal::{
    can::{Can, Filter, Frame},
    pac,
    prelude::*,
};

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    // To meet CAN clock accuracy requirements an external crystal or ceramic
    // resonator must be used.
    rcc.cfgr.use_hse(8.mhz()).freeze(&mut flash.acr);

    #[cfg(not(feature = "connectivity"))]
    let mut can = Can::new(dp.CAN1, &mut rcc.apb1, dp.USB);

    #[cfg(feature = "connectivity")]
    let mut can = Can::new(dp.CAN1, &mut rcc.apb1);

    // Use loopback mode: No pins need to be assigned to peripheral.
    can.configure(|config| {
        // APB1 (PCLK1): 8MHz, Bit rate: 125kBit/s, Sample Point 87.5%
        // Value was calculated with http://www.bittiming.can-wiki.info/
        config.set_bit_timing(0x001c_0003);
        config.set_loopback(true);
        config.set_silent(true);
    });

    // Use advanced configurations for the first three filter banks.
    // More details can be found in the reference manual of the device.
    #[cfg(not(feature = "connectivity"))]
    let mut filters = can
        .split_filters_advanced(0x0000_0006, 0xFFFF_FFFA, 0x0000_0007)
        .unwrap();
    #[cfg(feature = "connectivity")]
    let (mut filters, _) = can
        .split_filters_advanced(0x0000_0006, 0xFFFF_FFFA, 0x0000_0007, 3)
        .unwrap();

    assert_eq!(filters.num_available(), 8);

    // The order of the added filters is important: it must match configuration
    // of the `split_filters_advanced()` method.

    // 2x 11bit id + mask filter bank: Matches 0, 1, 2
    filters
        .add(&Filter::new_standard(0).with_mask(!0b1))
        .unwrap();
    filters
        .add(&Filter::new_standard(0).with_mask(!0b10))
        .unwrap();

    // 2x 29bit id filter bank: Matches 4, 5
    filters.add(&Filter::new_standard(4)).unwrap();
    filters.add(&Filter::new_standard(5)).unwrap();

    // 4x 11bit id filter bank: Matches 8, 9, 10, 11
    filters.add(&Filter::new_standard(8)).unwrap();
    filters.add(&Filter::new_standard(9)).unwrap();
    filters.add(&Filter::new_standard(10)).unwrap();
    filters.add(&Filter::new_standard(11)).unwrap();

    // Split the peripheral into transmitter and receiver parts.
    let mut rx = can.take_rx(filters).unwrap();
    let mut tx = can.take_tx().unwrap();

    // Sync to the bus and start normal operation.
    block!(can.enable()).ok();

    // Some messages shall pass the filters.
    for &id in &[0, 1, 2, 4, 5, 8, 9, 10, 11] {
        let frame_tx = Frame::new_standard(id, &[id as u8]);
        block!(tx.transmit(&frame_tx)).unwrap();
        let frame_rx = block!(rx.receive()).unwrap();
        assert_eq!(frame_tx, frame_rx);
    }

    // Others must be filtered out.
    for &id in &[3, 6, 7, 12] {
        let frame_tx = Frame::new_standard(id, &[id as u8]);
        block!(tx.transmit(&frame_tx)).unwrap();
        assert!(rx.receive().is_err());
    }

    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    let mut led = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);
    led.set_high().unwrap();

    loop {}
}
