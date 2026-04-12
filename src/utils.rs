use env_logger::{Builder, Env};

pub fn init_simple_logger() {
    use std::io::Write;
    let _ = Builder::from_env(Env::default().filter_or("RUST_LOG", "info"))
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .try_init();
}

pub fn init_test_logger() {
    use std::io::Write;
    let _ = Builder::from_env(Env::default().filter_or("RUST_LOG", "info"))
        .is_test(true)
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .try_init();
}
