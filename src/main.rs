#![no_std]
#![no_main]
#![allow(unused_imports)]

use ag_lcd::{Display, LcdDisplay};
use arduino_hal::{delay_ms, delay_us, prelude::*}; // prelude used for serial and printing to computer
use panic_halt as _;

#[arduino_hal::entry]
fn main() -> ! {
    arduino_hal::delay_ms(2000);
    // Setting up pins
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    let mut adc = arduino_hal::Adc::new(dp.ADC, Default::default());

    let a0 = pins.a0.into_analog_input(&mut adc); // Pin connected to water sensor
    let a1 = pins.a1.into_analog_input(&mut adc); // Pin connected to moisture sensor

    let mut d8 = pins.d8.into_output(); // Controls water sensor and moisture sensor
    let mut d7 = pins.d7.into_output(); // Controls pump

    // Setting up pins for LCD. Named by "[avr-pin]_[lcd-pin to be connected to]"
    let d12_rs = pins.d12.into_output().downgrade();
    let d10_en = pins.d10.into_output().downgrade();
    let d5_d4 = pins.d5.into_output().downgrade();
    let d4_d5 = pins.d4.into_output().downgrade();
    let d3_d6 = pins.d3.into_output().downgrade();
    let d2_d7 = pins.d2.into_output().downgrade();

    // Setting up LCD
    let delay = arduino_hal::Delay::new();
    let mut lcd: LcdDisplay<_, _> = LcdDisplay::new(d12_rs, d10_en, delay)
        .with_half_bus(d5_d4, d4_d5, d3_d6, d2_d7)
        .with_display(Display::On)
        .with_lines(ag_lcd::Lines::TwoLines)
        .build();

    // Setting sensor value limits
    let water_sensor_limit = 100; // Value above indicates sensor is in contact with water
    let moisture_sensor_lower_limit = 20; // Value below this indicates that moisture sensor is not placed in soil
    let moisture_sensor_dry_soil_limit = 500; // Value below this indicates that soil is dry enough to water

    // User defined parameters regarding pump and how much water which is plant is to be given
    let water_to_plant = 300.; // How much water that is to be pumped, in ml
    let ml_per_ms: f32 = 0.0475; // How much water in ml that is being pumped per ms. Based on measurement of 950 ml during 20 s

    // Calculations regarding time, i.e. time for pump to be turned on, and time until next measurement split into whole minutes and miliseconds
    let t_pump_on = (water_to_plant / ml_per_ms) as u16; // Amount of time that pump is to be running in ms
    let t_sensors_on = 3000; // Time to keep sensors on per measurement
    let t_next_check_ms: u32 = 24 * 60 * 60 * 1000 - (t_pump_on - t_sensors_on) as u32; // (A whole day in miliseconds) - (time for sensors and pump to finish)

    let t_next_check_mins = t_next_check_ms / 60_000;
    let t_next_check_remainder_ms = (t_next_check_ms % 60_000) as u16;

    // Used when Arduino is connected to computer. Used by ufmt::uwriteln!() to get outputs.
    // let mut serial = arduino_hal::default_serial!(dp, pins, 57600);
    // ufmt::uwriteln!(&mut serial, "t_pump_on: {} ", t_pump_on).void_unwrap();

    loop {
        d8.set_high(); // Turns on water sensor
        lcd.clear_print("Water & moisture", "sensors ON");
        delay_ms(t_sensors_on);
        let a0_value = a0.analog_read(&mut adc); // Value from water sensor
        let a1_value = a1.analog_read(&mut adc); // Value from moisture sensor
        d8.set_low(); // Turns off water sensor

        if a1_value < moisture_sensor_lower_limit {
            // Moisture sensor not in soil
            lcd.clear_print("Moisture sensor", "not in soil");
        } else if a0_value < water_sensor_limit && a1_value < moisture_sensor_dry_soil_limit {
            lcd.clear_print("Plant is dry:", "pump ON");
            d7.set_high(); // Turn on pump
        } else {
            lcd.clear_print("Plant has enough", "water: pump OFF");
        }
        delay_ms(t_pump_on); // So all scenarios take an equal amount of time
        d7.set_low(); // Turn off pump, if it was on

        for i in (1..=t_next_check_mins).rev() {
            // Inclusive, and goes from large value to small
            let count_down = CountDown::new(i);
            lcd.clear();
            delay_ms(1000);
            let mut buffer = [0u8; 9];
            let time_left_str = count_down.to_str(&mut buffer);
            lcd.clear_print("Measures in", time_left_str);
            delay_ms(60_000);
        }
        lcd.clear_print("Measures in", "less than 1 min");
        delay_ms(t_next_check_remainder_ms);
    }
}

trait ClearPrint {
    /// Clears the LCD before printing on both lines.
    fn clear_print(&mut self, _first_row: &str, _second_row: &str) {}
}

impl<T, D> ClearPrint for LcdDisplay<T, D>
where
    T: embedded_hal::digital::v2::OutputPin<Error = core::convert::Infallible> + Sized,
    D: embedded_hal::blocking::delay::DelayUs<u16> + Sized,
{
    fn clear_print(&mut self, first_row: &str, second_row: &str) {
        self.clear();
        self.set_position(0, 0);
        self.print(first_row);
        delay_us(100); // A delay, even a very small one, is needed between printing and setting a new position. A bit unreliable how large delay needs to be.
        self.set_position(0, 1);
        self.print(second_row);
        self.set_position(0, 0)
    }
}

struct CountDown {
    hours_left: u32,
    mins_left: u32,
}

impl CountDown {
    fn new(total_mins_left: u32) -> CountDown {
        CountDown {
            hours_left: (total_mins_left / 60),
            mins_left: (total_mins_left % 60),
        }
    }

    /// Converts a number to a UTF-8 slice, storing it in a supplied buffer, starting at index 'i-1'
    fn num_to_utf8slice(num: u32, buffer: &mut [u8; 9], mut i: usize) {
        let mut x = num;
        while x > 0 {
            i -= 1;
            let rem = x.rem_euclid(10);
            buffer[i] = rem as u8 + 48;
            x = x.div_euclid(10);
        }
    }

    /// Converts hours and minutes UTF-8 slice corresponding to "[hh]h:[mm]min"
    ///
    /// # Examples
    ///
    /// ```
    /// let count_down = Countdown{07, 23};
    /// let buffer = [0u8, 9];
    /// let count_down_str = count_down.to_str(&mut buffer);
    /// let expected_result = "07h:23min";
    /// assert_eq!(count_down_str, expected_result);
    /// ```
    fn to_str<'a>(self, buffer: &'a mut [u8; 9]) -> &'a str {
        if self.hours_left < 100 && self.mins_left < 100 {
            *buffer = [48, 48, 104, 58, 48, 48, 109, 105, 110]; // [0, 0, h, :, 0, 0, m, i, n]
            let mut i = 6; // Starts at 1 index after last '0'
            Self::num_to_utf8slice(self.mins_left, buffer, i);
            i = 2;
            Self::num_to_utf8slice(self.hours_left, buffer, i);
        }
        core::str::from_utf8(&buffer[0..]).unwrap()
    }
}
