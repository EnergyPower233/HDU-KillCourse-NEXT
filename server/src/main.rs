#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tokio::main]
async fn main() {
    let port = std::env::var("HDU_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .or_else(|| std::env::args().nth(1).and_then(|v| v.parse::<u16>().ok()))
        .unwrap_or(6688);
    match hdu_course_server::serve(port).await {
        Ok(()) => {}
        Err(e) => {
            eprintln!("启动失败：{e}");
            eprintln!("提示：端口 {port} 可能已被占用，可用环境变量 HDU_PORT 指定其他端口。");
            std::process::exit(1);
        }
    }
}
