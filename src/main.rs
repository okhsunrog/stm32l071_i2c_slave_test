#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    gpio::{Level, Output, Speed},
    i2c::{self, Address, OwnAddresses, SlaveCommandKind},
    peripherals,
    time::{khz, mhz},
};
use embassy_time::{Duration, Timer};
#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

const DEV_ADDR: u8 = 0x22;

bind_interrupts!(struct Irqs {
    I2C1 => i2c::EventInterruptHandler<peripherals::I2C1>, i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::task]
async fn system_ticker(mut led: Output<'static>) {
    loop {
        //info!("Alive!");
        Timer::after(Duration::from_millis(500)).await;
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: mhz(8),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            source: PllSource::HSE,
            mul: PllMul::MUL4,
            div: PllDiv::DIV2,
        });

        config.rcc.sys = Sysclk::PLL1_R;
        config.rcc.ahb_pre = AHBPrescaler::DIV2;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
    }

    let p = embassy_stm32::init(config);

    let speed = khz(400);
    let mut config = i2c::Config::default();
    config.sda_pullup = true;
    config.scl_pullup = true;
    // what exatly is this?
    //config.timeout = Duration::from_millis(5);

    let d_addr_config = i2c::SlaveAddrConfig {
        addr: OwnAddresses::OA1(Address::SevenBit(DEV_ADDR)),
        general_call: false,
    };

    let led = Output::new(p.PB7, Level::High, Speed::Low);
    let d_sda = p.PA10;
    let d_scl = p.PA9;

    let mut dev = i2c::I2c::new(
        p.I2C1, d_scl, d_sda, Irqs, p.DMA1_CH2, p.DMA1_CH3, speed, config,
    )
    .into_slave_multimaster(d_addr_config);

    info!("Blinking LED");
    unwrap!(spawner.spawn(system_ticker(led)));

    info!("Device start");
    let state = 5;

    loop {
        let mut buf = [0u8; 16];
        match dev.listen().await {
            Ok(i2c::SlaveCommand {
                kind: SlaveCommandKind::Read,
                address: Address::SevenBit(DEV_ADDR),
            }) => {
                info!("Read command");
                match dev.respond_to_read(&[state]).await {
                    Ok(i2c::SendStatus::LeftoverBytes(x)) => {
                        info!("tried to write {} extra bytes", x)
                    }
                    Ok(i2c::SendStatus::Done) => {
                        info!("Successfully responded to read");
                    }
                    Err(e) => error!("error while responding {}", e),
                }
            }

            Ok(i2c::SlaveCommand {
                kind: SlaveCommandKind::Write,
                address: Address::SevenBit(DEV_ADDR),
            }) => {
                info!("Write command");
                match dev.respond_to_write(&mut buf).await {
                    Ok(len) => {
                        info!("Device received {} bytes: {}", len, buf[..len]);
                    }
                    Err(e) => error!("error while responding {}", e),
                }
            }
            Ok(i2c::SlaveCommand { address, .. }) => {
                defmt::unreachable!(
                    "The slave matched address: {}, which it was not configured for",
                    address
                );
            }
            Err(e) => {
                error!("Error when listening for slave command: {}", e);
            }
        }
    }
}
