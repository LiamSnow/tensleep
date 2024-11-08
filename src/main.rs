extern crate rustc_serialize;

mod frank;
mod settings;

use frank::types::*;

#[rocket::main]
async fn main() {
    env_logger::init();
    let s = frank::init();

    frank::set_temperature(BedSide::Both, -10, &s);
}
