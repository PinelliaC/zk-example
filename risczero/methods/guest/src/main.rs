#![no_main]
#![no_std]

use risc0_zkvm::guest::env;
risc0_zkvm::guest::entry!(main);

fn main() {
    let x: u64 = env::read();
    if x > 100 {
        panic!("x is too large");
    }
    let y = x
        .checked_mul(x)
        .and_then(|x2| x2.checked_mul(x))
        .and_then(|x2| x2.checked_add(x))
        .and_then(|x3| x3.checked_add(5))
        .expect("overflow");

    env::commit(&y);
}
