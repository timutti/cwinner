fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        if let Err(e) = cwinner_lib::daemon::run().await {
            eprintln!("cwinnerd fatal error: {e}");
            std::process::exit(1);
        }
    });
}
