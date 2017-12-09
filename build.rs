extern crate lalrpop;

fn main() {
    let mut config = lalrpop::Configuration::new();
    config.use_cargo_dir_conventions().process().unwrap();
}
