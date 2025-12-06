use git_sensei::run;
use dotenvy::dotenv;
use std::process;

#[tokio::main]
async fn main() {
    dotenv().ok();
    
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
